//! Error types for the axga ecosystem.
//!
//! Every fallible operation must map to a typed error here.
//! No `anyhow` in library crates — only in `axga-cli` main.

use thiserror::Error;

/// Top-level error enum for the entire axga codebase.
#[derive(Error, Debug)]
pub enum AxgaError {
    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },

    #[error("HTTP response too large: {size} bytes (max {limit})")]
    HttpResponseTooLarge { size: u64, limit: u64 },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Tool execution failed: {tool} — {message}")]
    ToolError { tool: String, message: String },

    #[error("File too large: {path} is {size} bytes (max {limit})")]
    FileTooLarge { path: String, size: u64, limit: u64 },

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Token limit exceeded: {used}/{max}")]
    TokenLimitExceeded { used: u32, max: u32 },

    #[error("Operation aborted")]
    Aborted,

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias used throughout the codebase.
pub type AxgaResult<T> = Result<T, AxgaError>;
