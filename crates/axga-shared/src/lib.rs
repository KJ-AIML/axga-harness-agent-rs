//! `axga-shared` — Common types, errors, and constants used across all crates.
//!
//! This crate must be lightweight (no heavy dependencies).
//! Memory-critical structs like `Message` and `AgentState` live here
//! so they can be allocated with precise capacity controls.

pub mod error;
pub mod types;

/// Hard memory limits enforced across the codebase.
pub mod limits {
    /// Max file size readable by any tool (bytes).
    pub const MAX_FILE_READ_SIZE: u64 = 1_048_576; // 1 MB

    /// Max HTTP response body the agent will buffer (bytes).
    pub const MAX_HTTP_BUFFER_SIZE: usize = 1_048_576; // 1 MB

    /// Max conversation turns before summarization kicks in.
    pub const MAX_CONVERSATION_TURNS: usize = 20;

    /// Approximate token limit for active context window.
    pub const MAX_CONTEXT_TOKENS: u32 = 32_768;

    /// Heuristic: ~4 characters per token (zero-dependency estimate).
    pub const CHARS_PER_TOKEN: usize = 4;

    /// Max tool output length before truncation.
    pub const MAX_TOOL_OUTPUT_LEN: usize = 50_000;

    /// Bounded channel capacity for tool execution results.
    pub const TOOL_CHANNEL_CAP: usize = 100;
}
