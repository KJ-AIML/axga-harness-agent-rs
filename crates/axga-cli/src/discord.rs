//! Discord bot mode — connect axga to a Discord server.
//!
//! Usage: axga --discord --key <bot_token>
//!
//! Flow:
//! 1. Validates token via `/users/@me`, gets bot user ID + username
//! 2. Lists guilds via `/users/@me/guilds`, then channels per guild via `/guilds/{id}/channels`
//! 3. Polls each text channel every 3 seconds for the latest 5 messages
//! 4. Detects @mentions by checking `message["mentions"]` for the bot's user ID
//! 5. On mention: fetches 10-message history for context, shows typing, runs agent, replies
//! 6. Skips own bot messages and already-processed messages (per-channel tracking)

use axga_core::{Conversation, run_turn_streaming, StreamHandler, PermissionManager, PermissionMode};
use axga_core::goal::GoalManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub async fn run_discord_bot(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    token: &str,
    system_prompt: Option<&str>,
    dangerous: bool,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // ── Validate token + get bot info ──
    let resp = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {token}"))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Invalid Discord bot token. Create one at https://discord.com/developers/applications"
        );
    }

    let me: serde_json::Value = resp.json().await?;
    let bot_id = me["id"].as_str().unwrap_or("").to_string();
    let bot_name = me["username"].as_str().unwrap_or("unknown").to_string();
    tracing::info!(%bot_name, %bot_id, "Discord bot started");

    // ── List guilds + channels ──
    let guilds: Vec<serde_json::Value> = client
        .get("https://discord.com/api/v10/users/@me/guilds")
        .header("Authorization", format!("Bot {token}"))
        .send()
        .await?
        .json()
        .await?;

    let mut channels: Vec<(String, String)> = vec![]; // (channel_id, channel_name)

    for guild in &guilds {
        let guild_id = guild["id"].as_str().unwrap_or("");
        let guild_name = guild["name"].as_str().unwrap_or("unknown");

        let guild_channels: Vec<serde_json::Value> = match client
            .get(format!(
                "https://discord.com/api/v10/guilds/{guild_id}/channels"
            ))
            .header("Authorization", format!("Bot {token}"))
            .send()
            .await
        {
            Ok(resp) => resp.json().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!(%guild_id, %guild_name, "Failed to fetch channels: {e}");
                continue;
            }
        };

        for ch in guild_channels {
            // type 0 = GUILD_TEXT, type 5 = GUILD_ANNOUNCEMENT
            let ch_type = ch["type"].as_u64().unwrap_or(99);
            if ch_type == 0 || ch_type == 5 {
                let ch_id = ch["id"].as_str().unwrap_or("").to_string();
                let ch_name = ch["name"].as_str().unwrap_or("unknown").to_string();
                channels.push((ch_id, ch_name));
            }
        }
    }

    println!(
        "🤖 Discord bot @{bot_name} is running. Press Ctrl+C to stop."
    );
    println!("   Bot token: {}...", &token[..8.min(token.len())]);
    println!(
        "   Invite link: https://discord.com/oauth2/authorize?client_id={bot_id}&scope=bot&permissions=2048"
    );
    println!("   Listening on {} channels for @{bot_name}", channels.len());
    println!();

    let permissions = Arc::new(PermissionManager::new(
        if dangerous {
            PermissionMode::Auto
        } else {
            PermissionMode::Manual
        },
    ));
    let goal_manager = Arc::new(Mutex::new(GoalManager::new()));

    // Per-channel last processed message ID to avoid reprocessing
    let mut last_ids: HashMap<String, String> = HashMap::new();

    // ── Poll loop ──
    loop {
        for (ch_id, _ch_name) in &channels {
            // Fetch 5 most recent messages
            let msgs_url = format!(
                "https://discord.com/api/v10/channels/{ch_id}/messages?limit=5"
            );

            let messages: Vec<serde_json::Value> = match client
                .get(&msgs_url)
                .header("Authorization", format!("Bot {token}"))
                .send()
                .await
            {
                Ok(resp) => resp.json().await.unwrap_or_default(),
                Err(_) => continue,
            };

            // Process oldest-first so the latest message ID is stored last
            for msg in messages.iter().rev() {
                let msg_id = msg["id"].as_str().unwrap_or("").to_string();
                let author_bot = msg["author"]["bot"].as_bool().unwrap_or(false);

                // Skip own bot messages
                if author_bot {
                    continue;
                }

                // Skip already-processed messages (per channel)
                let last_id = last_ids.get(ch_id).cloned().unwrap_or_default();
                if msg_id <= last_id {
                    continue;
                }

                // Check for @mention of this bot
                let is_mentioned = msg["mentions"]
                    .as_array()
                    .map(|mentions| {
                        mentions.iter().any(|m| {
                            m["id"].as_str().unwrap_or("") == bot_id
                        })
                    })
                    .unwrap_or(false);

                if !is_mentioned {
                    // Not mentioned; still track this message ID so we don't
                    // re-check it, but don't process.
                    last_ids.insert(ch_id.clone(), msg_id);
                    continue;
                }

                // ── Bot is @mentioned ──
                let author = msg["author"]["username"]
                    .as_str()
                    .unwrap_or("user");
                let content = msg["content"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                println!("💬 @{author}: {content}");

                // ── Fetch 10-message history for context ──
                let history_url = format!(
                    "https://discord.com/api/v10/channels/{ch_id}/messages?limit=10&before={msg_id}"
                );

                let history: Vec<serde_json::Value> = match client
                    .get(&history_url)
                    .header("Authorization", format!("Bot {token}"))
                    .send()
                    .await
                {
                    Ok(resp) => resp.json().await.unwrap_or_default(),
                    Err(_) => vec![],
                };

                // Build context string from history + triggering message
                let context = build_context(&history, msg, &bot_name);

                // ── Show typing indicator ──
                let typing_url = format!(
                    "https://discord.com/api/v10/channels/{ch_id}/typing"
                );
                let _ = client
                    .post(&typing_url)
                    .header("Authorization", format!("Bot {token}"))
                    .send()
                    .await;

                // ── Build tool registry and conversation ──
                let mut conversation = Conversation::new();
                let registry = axga_core::build_default_registry(
                    dangerous,
                    Some(provider),
                    Some(model),
                    api_key,
                    base_url,
                    Some(goal_manager.clone()),
                )?;

                struct DiscordHandler {
                    text: String,
                }
                impl StreamHandler for DiscordHandler {
                    fn on_text_delta(&mut self, text: &str) { self.text.push_str(text); }
                    fn on_tool_call_delta(
                        &mut self,
                        _id: &str,
                        _name: &str,
                        _args: &str,
                    ) {
                    }
                    fn on_tool_call_result(
                        &mut self,
                        _name: &str,
                        _result: &str,
                    ) {
                    }
                    fn on_thinking(&mut self) {}
                    fn on_done(&mut self) {}
                }

                // ── Run agent ──
                let mut handler = DiscordHandler { text: String::new() };
                let result = run_turn_streaming(
                    provider,
                    api_key,
                    base_url,
                    model,
                    &mut conversation,
                    &context,
                    &registry,
                    system_prompt,
                    10,
                    &mut handler,
                    Some(permissions.clone()),
                )
                .await;

                match result {
                    Ok(turn) => {
                        // Use handler's accumulated text (includes post-tool response)
                        let response_text = if !handler.text.is_empty() {
                            handler.text.clone()
                        } else if !turn.final_text.is_empty() {
                            turn.final_text.clone()
                        } else {
                            "Done. No output.".to_string()
                        };
                        // If pending approvals, auto-approve and continue
                        if !turn.pending_approvals.is_empty() {
                            permissions.approve_all();
                            let _ = axga_core::continue_turn_streaming(
                                provider, api_key, base_url, model,
                                &mut conversation, &registry,
                                system_prompt, 10,
                                &mut handler, Some(permissions.clone()),
                                turn.pending_approvals,
                            ).await;
                        }
                        let reply = if !handler.text.is_empty() {
                            truncate_discord(&handler.text, 1900)
                        } else {
                            truncate_discord(&response_text, 1900)
                        };
                        println!("🤖 → {reply}");

                        let _ = client
                            .post(format!(
                                "https://discord.com/api/v10/channels/{ch_id}/messages"
                            ))
                            .header("Authorization", format!("Bot {token}"))
                            .json(&serde_json::json!({ "content": reply }))
                            .send()
                            .await;
                    }
                    Err(e) => {
                        let err_msg = format!("Error: {e}");
                        println!("🤖 ✗ {err_msg}");
                        let _ = client
                            .post(format!(
                                "https://discord.com/api/v10/channels/{ch_id}/messages"
                            ))
                            .header("Authorization", format!("Bot {token}"))
                            .json(&serde_json::json!({ "content": err_msg }))
                            .send()
                            .await;
                    }
                }

                // Track this message as processed
                last_ids.insert(ch_id.clone(), msg_id);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

/// Build a context string from channel history and the triggering message.
///
/// Formats the last N messages as a conversation log, then appends the
/// triggering @mention message so the agent sees full context.
fn build_context(
    history: &[serde_json::Value],
    trigger: &serde_json::Value,
    bot_name: &str,
) -> String {
    let mut ctx = String::from(
        "Recent channel conversation (oldest first):\n",
    );

    // Oldest first (Discord returns newest-first, so reverse)
    for msg in history.iter().rev() {
        let author = msg["author"]["username"]
            .as_str()
            .unwrap_or("unknown");
        let content = msg["content"].as_str().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let mut author_display = author.to_string();
        if msg["author"]["bot"].as_bool().unwrap_or(false)
            && msg["author"]["username"].as_str() == Some(bot_name)
        {
            author_display = format!("{author} (you)");
        }
        ctx.push_str(&format!("{author_display}: {content}\n"));
    }

    // Append the triggering @mention message
    let trigger_author = trigger["author"]["username"]
        .as_str()
        .unwrap_or("unknown");
    let trigger_content = trigger["content"].as_str().unwrap_or("").trim();
    // Strip the @mention from the content for cleaner context
    let cleaned = strip_mention(trigger_content, bot_name);
    ctx.push_str(&format!(
        "\n{trigger_author} (@mentioned you): {cleaned}\n"
    ));

    ctx
}

/// Remove the `@botname` prefix from a message so the agent sees a clean
/// question. Also handles `<@bot_id>` raw mention format.
fn strip_mention(content: &str, bot_name: &str) -> String {
    let at_name = format!("@{bot_name}");
    let s = content.strip_prefix(&at_name).unwrap_or(content);
    // Also strip <@12345...> raw mention if present at the start
    let s = if let Some(rest) = s.trim_start().strip_prefix('<')
        .and_then(|after_lt| after_lt.split_once('>'))
        .map(|(_, rest)| rest)
    {
        rest
    } else {
        s
    };
    s.trim().to_string()
}

fn truncate_discord(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... [truncated]", &s[..max_len])
    }
}
