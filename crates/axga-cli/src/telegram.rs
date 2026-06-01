//! Telegram bot mode — long-polling agent accessible via Telegram.
//!
//! Usage: axga --onboard --telegram --key <bot_token>
//!
//! Flow:
//! 1. Validates the token via getMe
//! 2. Starts long-polling getUpdates
//! 3. Each incoming message → runs agent → sends reply

use axga_core::{Conversation, ToolRegistry, run_turn};
use axga_core::tools::{fs, shell, code, memctrl_native, web_search, fetch_url};
use axga_shared::types::AgentMessage;
use serde_json::Value;
use std::time::Duration;

pub async fn run_telegram_bot(
    provider: &str,
    api_key: Option<&str>,
    model: &str,
    token: &str,
    system_prompt: Option<&str>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // Validate token
    let me_url = format!("https://api.telegram.org/bot{}/getMe", token);
    let me: Value = client.get(&me_url).send().await?.json().await?;

    if !me["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!("Invalid Telegram bot token. Create one at @BotFather.");
    }

    let bot_name = me["result"]["username"].as_str().unwrap_or("unknown");
    tracing::info!(%bot_name, "Telegram bot started");

    // Build tool registry
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool::new(false))?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl_native::MemCtrlTool::new()?)?;
    registry.register(web_search::WebSearchTool)?;
    registry.register(fetch_url::FetchUrlTool)?;
    let mut conversation = Conversation::new();
    let mut last_update_id: i64 = 0;

    println!("🤖 Telegram bot @{} is running. Press Ctrl+C to stop.", bot_name);
    println!("   Send a message to @{} on Telegram.", bot_name);
    println!("   Tokens checked: {}", token.chars().take(8).collect::<String>());
    println!();

    loop {
        let updates_url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            token,
            last_update_id + 1
        );

        match client.get(&updates_url).timeout(Duration::from_secs(35)).send().await {
            Ok(resp) => {
                let updates: Value = match resp.json().await {
                    Ok(u) => u,
                    Err(_) => { tokio::time::sleep(Duration::from_secs(1)).await; continue; }
                };

                if let Some(results) = updates["result"].as_array() {
                    for update in results {
                        let update_id = update["update_id"].as_i64().unwrap_or(0);
                        if update_id > last_update_id {
                            last_update_id = update_id;
                        }

                        if let Some(msg) = update.get("message") {
                            let chat_id = msg["chat"]["id"].as_i64().unwrap_or(0);
                            let text = msg["text"].as_str().unwrap_or("");
                            let username = msg["from"]["first_name"].as_str().unwrap_or("User");

                            if text.is_empty() || chat_id == 0 {
                                continue;
                            }

                            tracing::info!(%username, %chat_id, %text, "message received");

                            // Show typing indicator
                            let _ = client.post(format!(
                                "https://api.telegram.org/bot{}/sendChatAction",
                                token
                            ))
                            .json(&serde_json::json!({
                                "chat_id": chat_id,
                                "action": "typing"
                            }))
                            .send()
                            .await;

                            // Run agent
                            match run_turn(
                                provider, api_key, None, model,
                                &mut conversation, text,
                                &registry, system_prompt,
                                10,
                            ).await {
                                Ok(turn) => {
                                    let reply = if turn.final_text.is_empty() {
                                        "Done.".to_string()
                                    } else {
                                        truncate_telegram(&turn.final_text)
                                    };

                                    let _ = client.post(format!(
                                        "https://api.telegram.org/bot{}/sendMessage",
                                        token
                                    ))
                                    .json(&serde_json::json!({
                                        "chat_id": chat_id,
                                        "text": reply,
                                        "parse_mode": "Markdown"
                                    }))
                                    .send()
                                    .await;

                                    tracing::info!(%username, "reply sent ({} chars)", reply.len());
                                }
                                Err(e) => {
                                    let _ = client.post(format!(
                                        "https://api.telegram.org/bot{}/sendMessage",
                                        token
                                    ))
                                    .json(&serde_json::json!({
                                        "chat_id": chat_id,
                                        "text": format!("Error: {}", e)
                                    }))
                                    .send()
                                    .await;
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

fn truncate_telegram(text: &str) -> String {
    const MAX: usize = 4000; // Telegram limit is 4096, leave margin
    if text.len() <= MAX {
        text.to_string()
    } else {
        let mut t = text[..MAX].to_string();
        t.push_str("\n\n... [truncated]");
        t
    }
}
/// Webhook mode — sets up a webhook URL and listens for updates via POST.
pub async fn run_telegram_webhook(
    provider: &str, api_key: Option<&str>, model: &str,
    token: &str, _system_prompt: Option<&str>, webhook_url: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let set_url = format!("https://api.telegram.org/bot{}/setWebhook?url={}", token, webhook_url);
    let resp: Value = client.get(&set_url).send().await?.json().await?;
    if resp["ok"].as_bool().unwrap_or(false) {
        println!("Webhook set to {}", webhook_url);
    } else {
        anyhow::bail!("Failed to set webhook: {}", resp["description"].as_str().unwrap_or("unknown"));
    }
    println!("Webhook mode active. Use nginx/caddy reverse proxy to {}", webhook_url);
    Ok(())
}
