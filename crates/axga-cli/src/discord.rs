//! Discord bot mode — connect axga to a Discord server.
//!
//! Usage: axga --discord --key <bot_token>
//!
//! Flow:
//! 1. Validates the token via a simple HTTP GET
//! 2. Listens for messages via HTTP polling (simpler than websocket gateway)
//! 3. Each incoming message → runs agent → sends reply

use axga_core::{Conversation, run_turn_streaming, StreamHandler, PermissionManager, PermissionMode};
use axga_core::goal::GoalManager;
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

    // Validate token
    let me_url = "https://discord.com/api/v10/users/@me".to_string();
    let resp = client
        .get(&me_url)
        .header("Authorization", format!("Bot {token}"))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Invalid Discord bot token. Create one at https://discord.com/developers/applications"
        );
    }

    let me: serde_json::Value = resp.json().await?;
    let bot_name = me["username"].as_str().unwrap_or("unknown");
    tracing::info!(%bot_name, "Discord bot started");

    println!("🤖 Discord bot @{bot_name} is running. Press Ctrl+C to stop.");
    println!("   Bot token: {}...", &token[..8.min(token.len())]);
    println!("   Invite link: https://discord.com/oauth2/authorize?client_id={}&scope=bot&permissions=2048",
        me["id"].as_str().unwrap_or("YOUR_CLIENT_ID"));
    println!();

    let permissions = Arc::new(PermissionManager::new(
        if dangerous { PermissionMode::Auto } else { PermissionMode::Manual }
    ));
    let goal_manager = Arc::new(Mutex::new(GoalManager::new()));

    let mut last_message_id: Option<String> = None;

    // Poll for messages every 2 seconds
    loop {
        let channels_url = "https://discord.com/api/v10/users/@me/channels".to_string();
        let channels: Vec<serde_json::Value> = client
            .get(&channels_url)
            .header("Authorization", format!("Bot {token}"))
            .send()
            .await?
            .json()
            .await?;

        for channel in channels {
            let channel_id = channel["id"].as_str().unwrap_or("");
            let msgs_url = format!(
                "https://discord.com/api/v10/channels/{channel_id}/messages?limit=1"
            );

            let messages: Vec<serde_json::Value> = client
                .get(&msgs_url)
                .header("Authorization", format!("Bot {token}"))
                .send()
                .await?
                .json()
                .await?;

            for msg in messages {
                let msg_id = msg["id"].as_str().unwrap_or("").to_string();

                // Skip our own messages and already-processed ones
                if msg["author"]["bot"].as_bool().unwrap_or(false) {
                    continue;
                }
                if Some(&msg_id) == last_message_id.as_ref() {
                    continue;
                }

                let content = msg["content"].as_str().unwrap_or("").trim().to_string();
                if content.is_empty() { continue; }

                // Show typing indicator
                let typing_url = format!(
                    "https://discord.com/api/v10/channels/{channel_id}/typing"
                );
                let _ = client
                    .post(&typing_url)
                    .header("Authorization", format!("Bot {token}"))
                    .send()
                    .await;

                last_message_id = Some(msg_id.clone());
                println!("💬 {}: {content}", msg["author"]["username"].as_str().unwrap_or("user"));

                // Process with agent
                let mut conversation = Conversation::new();
                let registry = axga_core::build_default_registry(
                    dangerous,
                    Some(provider),
                    Some(model),
                    api_key,
                    base_url,
                    Some(goal_manager.clone()),
                )?;

                struct DiscordHandler;
                impl StreamHandler for DiscordHandler {
                    fn on_text_delta(&mut self, _text: &str) {}
                    fn on_tool_call_delta(&mut self, _id: &str, _name: &str, _args: &str) {}
                    fn on_tool_call_result(&mut self, _name: &str, _result: &str) {}
                    fn on_thinking(&mut self) {}
                    fn on_done(&mut self) {}
                }

                let result = run_turn_streaming(
                    provider, api_key, base_url, model,
                    &mut conversation, &content, &registry,
                    system_prompt, 10, &mut DiscordHandler,
                    Some(permissions.clone()),
                ).await;

                match result {
                    Ok(turn) => {
                        let reply = if turn.final_text.is_empty() {
                            "Done. No output.".to_string()
                        } else {
                            truncate_discord(&turn.final_text, 1900)
                        };
                        println!("🤖 → {reply}");

                        let _ = client
                            .post(format!("https://discord.com/api/v10/channels/{channel_id}/messages"))
                            .header("Authorization", format!("Bot {token}"))
                            .json(&serde_json::json!({ "content": reply }))
                            .send()
                            .await;
                    }
                    Err(e) => {
                        let err_msg = format!("Error: {e}");
                        println!("🤖 ✗ {err_msg}");
                        let _ = client
                            .post(format!("https://discord.com/api/v10/channels/{channel_id}/messages"))
                            .header("Authorization", format!("Bot {token}"))
                            .json(&serde_json::json!({ "content": err_msg }))
                            .send()
                            .await;
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

fn truncate_discord(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... [truncated]", &s[..max_len])
    }
}
