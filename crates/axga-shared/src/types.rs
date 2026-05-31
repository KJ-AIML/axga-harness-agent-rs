//! Core types shared across all crates.
//!
//! Memory layout notes:
//! - `AgentMessage` is allocated once per turn → keep fields small.
//! - `ToolResult` content is truncated at `MAX_TOOL_OUTPUT_LEN` before storage.
//! - Use `Box<str>` instead of `String` for message content after streaming complete.

use serde::{Deserialize, Serialize};

// ─── Message Types ────────────────────────────────────────────────

/// A single message in the conversation transcript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum AgentMessage {
    #[serde(rename = "user")]
    User { content: String },
    #[serde(rename = "assistant")]
    Assistant { content: AssistantContent },
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "tool")]
    Tool { tool_call_id: String, content: String },
}

/// Content blocks within an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantContent {
    pub text: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

// ─── Agent State Machine ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Running,
    WaitingForTools,
    Finished,
    Error,
    Aborted,
}

// ─── Tool Definitions ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

// ─── Streaming Events ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta { text: String },
    ToolCallDelta { id: String, name: String, args_fragment: String },
    ThinkingDelta { text: String },
    Usage { input_tokens: u32, output_tokens: u32 },
    Done,
    Stop { reason: String },
    Error { message: String },
}

// ─── Configuration ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider: ProviderConfig,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub headers: Option<Vec<(String, String)>>,
}

// ─── Token Budgeting ──────────────────────────────────────────────

/// Tracks token usage across a conversation to enforce limits.
#[derive(Debug, Clone, Default)]
pub struct TokenBudget {
    pub used: u32,
    pub max: u32,
}

impl TokenBudget {
    pub fn new(max: u32) -> Self {
        Self { used: 0, max }
    }

    /// Returns `true` if adding `n` tokens would exceed the budget.
    pub fn would_exceed(&self, n: u32) -> bool {
        self.used.saturating_add(n) > self.max
    }

    /// Reserve `n` tokens. Returns `Err` if budget exceeded.
    pub fn reserve(&mut self, n: u32) -> Result<(), ()> {
        if self.would_exceed(n) {
            return Err(());
        }
        self.used += n;
        Ok(())
    }

    /// Estimate tokens from character count using 4-char ≈ 1-token heuristic.
    pub fn estimate_tokens(text: &str) -> u32 {
        (text.len() / super::limits::CHARS_PER_TOKEN).max(1) as u32
    }
}
