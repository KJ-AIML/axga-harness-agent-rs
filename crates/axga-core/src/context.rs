//! Token budgeting and context window management.
//!
//! Uses the 4-char ≈ 1-token heuristic initially (ADR-006).
//! Can be swapped for `tiktoken-rs` later without changing the public API.

use axga_shared::limits::CHARS_PER_TOKEN;
use axga_shared::types::{AgentMessage, AssistantContent, TokenBudget};

/// Calculate the estimated token count of the full conversation.
pub fn estimate_conversation_tokens(messages: &[AgentMessage]) -> u32 {
    messages
        .iter()
        .map(|msg| match msg {
            AgentMessage::User { content } => estimate_text_tokens(content),
            AgentMessage::Assistant { content } => estimate_assistant_tokens(content),
            AgentMessage::System { content } => estimate_text_tokens(content),
            AgentMessage::Tool { content, .. } => estimate_text_tokens(content),
        })
        .sum()
}

fn estimate_assistant_tokens(content: &AssistantContent) -> u32 {
    let mut tokens: u32 = 0;
    if let Some(ref text) = content.text {
        tokens += estimate_text_tokens(text);
    }
    if let Some(ref calls) = content.tool_calls {
        for tc in calls {
            tokens += estimate_text_tokens(&tc.name);
            tokens += estimate_text_tokens(&tc.arguments.to_string());
        }
    }
    if let Some(ref thinking) = content.thinking {
        tokens += estimate_text_tokens(thinking);
    }
    tokens
}

fn estimate_text_tokens(text: &str) -> u32 {
    (text.len() / CHARS_PER_TOKEN).max(1) as u32
}

/// Check if adding `message` would exceed the token budget.
pub fn would_exceed_budget(
    budget: &TokenBudget,
    messages: &[AgentMessage],
    new_message: &AgentMessage,
) -> bool {
    let current = estimate_conversation_tokens(messages);
    let new = match new_message {
        AgentMessage::User { content } => estimate_text_tokens(content),
        AgentMessage::Assistant { content } => estimate_assistant_tokens(content),
        AgentMessage::System { content } => estimate_text_tokens(content),
        AgentMessage::Tool { content, .. } => estimate_text_tokens(content),
    };
    budget.used.saturating_add(current).saturating_add(new) > budget.max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_conversation() {
        assert_eq!(estimate_conversation_tokens(&[]), 0);
    }

    #[test]
    fn heuristic_is_stable() {
        let text = "this is a test sentence with some words";
        let tokens = estimate_text_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < 50);
    }
}
