//! `axga-ai` — LLM provider abstraction.
//!
//! # Design (ADR-002)
//! - Thin HTTP wrappers around OpenAI/Anthropic APIs.
//! - SSE parsing yields `StreamEvent` chunks without buffering full response.
//! - Uses `axga-shared` for common types and error definitions.

pub mod providers;
pub mod stream;
pub mod request;

pub use providers::Provider;
