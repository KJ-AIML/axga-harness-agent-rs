//! Telegram bot mode - long-polling agent accessible via Telegram.
//!
//! Usage: axga --onboard --telegram --key <bot_token>
//!
//! Flow:
//! 1. Validates the token via getMe
//! 2. Starts long-polling getUpdates
//! 3. Each incoming message runs an isolated per-chat agent conversation

use axga_core::tools::{code, fetch_url, fs, memctrl, shell, web_search};
use axga_core::{Conversation, ToolRegistry, run_turn};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

const MAX_TELEGRAM_CHATS: usize = 64;
const TELEGRAM_MESSAGE_LIMIT: usize = 4000;

pub struct TelegramBotConfig<'a> {
    pub provider: &'a str,
    pub api_key: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model: &'a str,
    pub token: &'a str,
    pub allowed_users: &'a [i64],
    pub system_prompt: Option<&'a str>,
    pub max_turns: usize,
}

pub async fn run_telegram_bot(config: TelegramBotConfig<'_>) -> anyhow::Result<()> {
    let TelegramBotConfig {
        provider,
        api_key,
        base_url,
        model,
        token,
        allowed_users,
        system_prompt,
        max_turns,
    } = config;
    let client = reqwest::Client::new();

    let me_url = format!("https://api.telegram.org/bot{token}/getMe");
    let me: Value = client.get(&me_url).send().await?.json().await?;

    if !me["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!("Invalid Telegram bot token. Create one at @BotFather.");
    }

    let bot_name = me["result"]["username"].as_str().unwrap_or("unknown");
    tracing::info!(%bot_name, "Telegram bot started");

    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool)?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl::MemCtrlTool)?;
    registry.register(web_search::WebSearchTool)?;
    registry.register(fetch_url::FetchUrlTool)?;

    let mut conversations: HashMap<i64, Conversation> = HashMap::new();
    let mut chat_order: VecDeque<i64> = VecDeque::new();
    let mut last_update_id: i64 = 0;

    println!("Telegram bot @{bot_name} is running. Press Ctrl+C to stop.");
    println!("   Send a message to @{bot_name} on Telegram.");
    println!(
        "   Tokens checked: {}",
        token.chars().take(8).collect::<String>()
    );
    if !allowed_users.is_empty() {
        println!("   Access control: {} allowed user(s)", allowed_users.len());
    }
    println!();

    loop {
        let updates_url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            token,
            last_update_id + 1
        );

        match client
            .get(&updates_url)
            .timeout(Duration::from_secs(35))
            .send()
            .await
        {
            Ok(resp) => {
                let updates: Value = match resp.json().await {
                    Ok(u) => u,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                if let Some(results) = updates["result"].as_array() {
                    for update in results {
                        let update_id = update["update_id"].as_i64().unwrap_or(0);
                        if update_id > last_update_id {
                            last_update_id = update_id;
                        }

                        if let Some(msg) = update.get("message") {
                            let chat_id = msg["chat"]["id"].as_i64().unwrap_or(0);
                            let user_id = msg["from"]["id"].as_i64().unwrap_or(0);
                            let text = msg["text"].as_str().unwrap_or("");
                            let username = msg["from"]["first_name"].as_str().unwrap_or("User");

                            if text.is_empty() || chat_id == 0 || user_id == 0 {
                                continue;
                            }

                            if !is_allowed_user(user_id, allowed_users) {
                                tracing::warn!(%username, %user_id, %chat_id, "telegram user denied");
                                let _ = send_text_chunks(&client, token, chat_id, "Unauthorized.")
                                    .await;
                                continue;
                            }

                            tracing::info!(%username, %chat_id, %text, "message received");

                            if let Some(reply) = handle_command(
                                text,
                                chat_id,
                                &mut conversations,
                                provider,
                                model,
                                max_turns,
                            ) {
                                let _ = send_text_chunks(&client, token, chat_id, &reply).await;
                                continue;
                            }

                            let _ = client
                                .post(format!(
                                    "https://api.telegram.org/bot{token}/sendChatAction"
                                ))
                                .json(&serde_json::json!({
                                    "chat_id": chat_id,
                                    "action": "typing"
                                }))
                                .send()
                                .await;

                            let conversation =
                                conversation_for_chat(&mut conversations, &mut chat_order, chat_id);

                            match run_turn(
                                provider,
                                api_key,
                                base_url,
                                model,
                                conversation,
                                text,
                                &registry,
                                system_prompt,
                                max_turns,
                            )
                            .await
                            {
                                Ok(turn) => {
                                    let reply = if turn.final_text.trim().is_empty() {
                                        "Done."
                                    } else {
                                        &turn.final_text
                                    };

                                    let _ = send_text_chunks(&client, token, chat_id, reply).await;

                                    tracing::info!(%username, "reply sent ({} chars)", reply.len());
                                }
                                Err(e) => {
                                    let _ = send_text_chunks(
                                        &client,
                                        token,
                                        chat_id,
                                        &format!("Error: {e}"),
                                    )
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

fn conversation_for_chat<'a>(
    conversations: &'a mut HashMap<i64, Conversation>,
    chat_order: &mut VecDeque<i64>,
    chat_id: i64,
) -> &'a mut Conversation {
    if !conversations.contains_key(&chat_id) {
        while conversations.len() >= MAX_TELEGRAM_CHATS {
            let Some(oldest) = chat_order.pop_front() else {
                break;
            };
            conversations.remove(&oldest);
        }
        conversations.insert(chat_id, Conversation::new());
        chat_order.push_back(chat_id);
    }

    conversations
        .get_mut(&chat_id)
        .expect("conversation inserted")
}

fn handle_command(
    text: &str,
    chat_id: i64,
    conversations: &mut HashMap<i64, Conversation>,
    provider: &str,
    model: &str,
    max_turns: usize,
) -> Option<String> {
    match text.trim() {
        "/start" | "/help" => Some(
            "Commands:\n/reset - clear this chat context\n/status - show runtime status\n/help - show this help"
                .to_string(),
        ),
        "/reset" => {
            conversations.remove(&chat_id);
            Some("Context reset for this chat.".to_string())
        }
        "/status" => {
            let messages = conversations
                .get(&chat_id)
                .map(Conversation::len)
                .unwrap_or_default();
            Some(format!(
                "Provider: {provider}\nModel: {model}\nMax turns: {max_turns}\nMessages in this chat: {messages}"
            ))
        }
        _ => None,
    }
}

fn is_allowed_user(user_id: i64, allowed_users: &[i64]) -> bool {
    allowed_users.is_empty() || allowed_users.contains(&user_id)
}

async fn send_text_chunks(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
) -> anyhow::Result<()> {
    for chunk in split_telegram_message(text) {
        client
            .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": chunk
            }))
            .send()
            .await?;
    }
    Ok(())
}

fn split_telegram_message(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for ch in text.chars() {
        if current_len >= TELEGRAM_MESSAGE_LIMIT {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        current.push(ch);
        current_len += 1;
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_allows_everyone_when_list_is_empty() {
        assert!(is_allowed_user(42, &[]));
    }

    #[test]
    fn access_enforces_allowed_users_when_configured() {
        assert!(is_allowed_user(42, &[42, 99]));
        assert!(!is_allowed_user(7, &[42, 99]));
    }

    #[test]
    fn message_split_keeps_chunks_under_limit() {
        let text = "a".repeat(TELEGRAM_MESSAGE_LIMIT + 12);
        let chunks = split_telegram_message(&text);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), TELEGRAM_MESSAGE_LIMIT);
        assert_eq!(chunks[1].chars().count(), 12);
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn message_split_preserves_multibyte_chars() {
        let text = "界".repeat(TELEGRAM_MESSAGE_LIMIT + 1);
        let chunks = split_telegram_message(&text);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), TELEGRAM_MESSAGE_LIMIT);
        assert_eq!(chunks[1], "界");
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn conversations_are_isolated_per_chat() {
        let mut conversations = HashMap::new();
        let mut chat_order = VecDeque::new();

        conversation_for_chat(&mut conversations, &mut chat_order, 1).push(
            axga_shared::types::AgentMessage::User {
                content: "chat one".into(),
            },
        );
        conversation_for_chat(&mut conversations, &mut chat_order, 2).push(
            axga_shared::types::AgentMessage::User {
                content: "chat two".into(),
            },
        );

        assert_eq!(conversations.get(&1).unwrap().len(), 1);
        assert_eq!(conversations.get(&2).unwrap().len(), 1);
        assert_ne!(
            conversations
                .get(&1)
                .unwrap()
                .messages()
                .next()
                .map(|message| format!("{message:?}")),
            conversations
                .get(&2)
                .unwrap()
                .messages()
                .next()
                .map(|message| format!("{message:?}"))
        );
    }

    #[test]
    fn conversations_are_capped() {
        let mut conversations = HashMap::new();
        let mut chat_order = VecDeque::new();

        for chat_id in 0..=MAX_TELEGRAM_CHATS as i64 {
            let _ = conversation_for_chat(&mut conversations, &mut chat_order, chat_id);
        }

        assert_eq!(conversations.len(), MAX_TELEGRAM_CHATS);
        assert!(!conversations.contains_key(&0));
        assert!(conversations.contains_key(&(MAX_TELEGRAM_CHATS as i64)));
    }
}
