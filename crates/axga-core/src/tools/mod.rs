//! Tool registry and trait system.
//!
//! Tools are registered at startup and looked up by name at runtime.
//! Each tool is a `Box<dyn Tool>` for runtime polymorphism.

pub mod registry;
pub mod fs;
pub mod shell;
pub mod code;
pub mod memctrl_native;
pub mod web_search;
pub mod fetch_url;
pub mod task_manager;
pub mod task_list;
pub mod task_output;
pub mod task_stop;
pub mod agent;
pub mod agent_swarm;
pub mod plan;
pub mod ask_user;
pub mod cron;
pub mod goal;

use axga_shared::error::AxgaResult;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;

/// Global plan mode flag. When true, write/shell/network-mutating tools are
/// blocked. Read-only tools (read_file, grep, glob, etc.) still work.
pub static PLAN_MODE: AtomicBool = AtomicBool::new(false);

// Re-export task manager types for use by shell and task tools.
pub use task_manager::{TaskManager, TaskHandle, TaskStatus, TaskInfo};

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
