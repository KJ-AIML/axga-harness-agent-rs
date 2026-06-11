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

use axga_core::{run_turn_streaming, StreamHandler, PermissionManager, PermissionMode};
use axga_core::goal::GoalManager;
use axga_shared::types::AgentMessage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn dirs_config() -> PathBuf {
    if let Ok(dir) = std::env::var("AXGA_CONFIG_DIR") { return PathBuf::from(dir).join("axga"); }
    #[cfg(target_os = "windows")] { PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("axga") }
    #[cfg(not(target_os = "windows"))] { PathBuf::from(format!("{}/.config/axga", std::env::var("HOME").unwrap_or_default())) }
}

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
        if dangerous { PermissionMode::Auto } else { PermissionMode::Manual },
    ));
    let goal_manager = Arc::new(Mutex::new(GoalManager::new()));

    // Per-channel persistent conversations (remember context across messages)

    // Per-channel last processed message ID to avoid reprocessing
    let state_path = {
        let dir = dirs_config();
        std::fs::create_dir_all(&dir)?;
        dir.join("discord_state.json")
    };
    let mut last_ids: HashMap<String, String> = if state_path.exists() {
        std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    if !last_ids.is_empty() {
        println!("   Resuming from saved state ({} channels)", last_ids.len());
    } else {
        // First run or state not found — seed with latest message IDs
        // so we don't re-process existing messages
        println!("   First run — skipping existing messages...");
        for (ch_id, _ch_name) in &channels {
            let resp = match client
                .get(format!("https://discord.com/api/v10/channels/{ch_id}/messages?limit=1"))
                .header("Authorization", format!("Bot {token}"))
                .send().await
            {
                Ok(r) => r,
                Err(_) => continue,
            };
            let latest = resp
                .json::<Vec<serde_json::Value>>().await
                .ok()
                .and_then(|msgs| msgs.first().and_then(|m| m["id"].as_str().map(|s| s.to_string())));
            if let Some(id) = latest {
                last_ids.insert(ch_id.clone(), id);
            }
        }
        println!("   Seeded {} channels with latest message IDs", last_ids.len());
        // Save seed state
        if let Ok(json) = serde_json::to_string(&last_ids) {
            let _ = std::fs::write(&state_path, json);
        }
    }

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
                // Persist state periodically
                if last_ids.len() % 5 == 0 {
                    if let Ok(json) = serde_json::to_string(&last_ids) {
                        let _ = std::fs::write(&state_path, json);
                    }
                }
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

                // Build thread context from recent history (so LLM can follow conversation)
                let user_input = strip_mention(&content, &bot_name);
                let thread_context = build_thread_context(&history, &bot_name, &author, &user_input);

                // ── Show typing indicator ──
                let typing_url = format!(
                    "https://discord.com/api/v10/channels/{ch_id}/typing"
                );
                let _ = client
                    .post(&typing_url)
                    .header("Authorization", format!("Bot {token}"))
                    .send()
                    .await;

                // ── Build tool registry ──
                let mut conversation = axga_core::Conversation::new();
                // Push thread context as system message so LLM knows what's happening
                if !thread_context.is_empty() {
                    conversation.push(AgentMessage::System {
                        content: format!("You are axga, a helpful AI agent. Below is the recent conversation in this Discord channel. Respond to the last message, keeping the thread context in mind.\n\n{thread_context}"),
                    });
                }
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
                    &user_input,
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
                            handler.text.clone()
                        } else {
                            response_text
                        };
                        // Split long messages into chunks (Discord 2000 char limit)
                        let chunks = chunk_message(&reply, 1950);
                        for (i, chunk) in chunks.iter().enumerate() {
                            let prefix = if chunks.len() > 1 {
                                format!("({}/{}) ", i + 1, chunks.len())
                            } else {
                                String::new()
                            };
                            let msg = format!("{prefix}{chunk}");
                            println!("🤖 → {msg}");

                            let _ = client
                                .post(format!(
                                    "https://discord.com/api/v10/channels/{ch_id}/messages"
                                ))
                                .header("Authorization", format!("Bot {token}"))
                                .json(&serde_json::json!({ "content": msg }))
                                .send()
                                .await;
                        }
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
                // Persist state periodically
                if last_ids.len() % 5 == 0 {
                    if let Ok(json) = serde_json::to_string(&last_ids) {
                        let _ = std::fs::write(&state_path, json);
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
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

/// Build a thread context from recent channel messages so the LLM
/// can follow the conversation flow across messages.
fn build_thread_context(
    history: &[serde_json::Value],
    bot_name: &str,
    current_author: &str,
    current_input: &str,
) -> String {
    let mut ctx = String::from("Recent chat history:\n");

    // History is newest-first from Discord — iterate oldest-first
    for msg in history.iter().rev() {
        let author = msg["author"]["username"].as_str().unwrap_or("unknown");
        let content = msg["content"].as_str().unwrap_or("").trim();
        if content.is_empty() { continue; }
        let label = if msg["author"]["bot"].as_bool().unwrap_or(false)
            && msg["author"]["username"].as_str() == Some(bot_name)
        {
            format!("{author} (you)")
        } else {
            author.to_string()
        };
        ctx.push_str(&format!("{label}: {content}\n"));
    }

    ctx.push_str(&format!("\nCurrent: {current_author}: {current_input}\n"));
    ctx
}

/// Split a long message into Discord-safe chunks (max 2000 chars per chunk).
/// Respects UTF-8 character boundaries.
fn chunk_message(s: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut remaining = s;
    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }
        // Find safe cut point at char boundary
        let mut end = max_chars;
        while end > 0 && !remaining.is_char_boundary(end) {
            end -= 1;
        }
        // Try to break at newline for cleaner cuts
        if let Some(nl) = remaining[..end].rfind('\n') {
            if nl > max_chars / 2 {
                end = nl + 1;
            }
        }
        chunks.push(remaining[..end].trim_end().to_string());
        remaining = remaining[end..].trim_start();
    }
    chunks
}
