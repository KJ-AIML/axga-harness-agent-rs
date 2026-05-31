//! Tool registry and trait system.
//!
//! Tools are registered at startup and looked up by name at runtime.
//! Each tool is a `Box<dyn Tool>` for runtime polymorphism.

pub mod registry;
pub mod fs;
pub mod shell;
pub mod code;
pub mod memctrl;
pub mod web_search;
pub mod fetch_url;

use axga_shared::error::AxgaResult;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

/// A registered tool that the agent can invoke.
///
/// # Design (ADR-nnn)
/// Uses `async fn` in trait (Rust 1.75+) rather than `fn` pointers
/// or macro-generated dispatch. This keeps the tool API ergonomic
/// while still allowing closures/state in tool implementations.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>>;
}
