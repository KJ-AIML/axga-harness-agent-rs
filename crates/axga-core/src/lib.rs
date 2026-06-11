//! `axga-core` — Agent runtime.

pub mod state;
pub mod context;
pub mod agent_loop;
pub mod executor;
pub mod tools;
pub mod session;
pub mod retry;
pub mod config;
pub mod orchestrator;
pub mod permission;

pub use state::Conversation;
pub use agent_loop::{run_turn, run_turn_streaming, continue_turn_streaming, simple_chat, StreamHandler};
pub use tools::registry::ToolRegistry;
pub use tools::Tool;
pub use config::{Config, load_config, save_config};
pub use session::{save_session, load_session, list_sessions};
pub use orchestrator::Orchestrator;
pub use permission::{PermissionManager, PermissionMode, Permission};

use axga_shared::error::AxgaResult;

/// Build the default tool registry with all built-in tools.
/// `dangerous` bypasses shell denylist when true.
pub fn build_default_registry(dangerous: bool) -> AxgaResult<ToolRegistry> {
    use tools::*;
    let mut registry = ToolRegistry::new();

    let task_manager = task_manager::TaskManager::new();

    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool::new(dangerous, std::sync::Arc::clone(&task_manager)))?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl_native::MemCtrlTool::new()?)?;
    registry.register(web_search::WebSearchTool)?;
    registry.register(fetch_url::FetchUrlTool)?;
    registry.register(task_list::TaskListTool::new(std::sync::Arc::clone(&task_manager)))?;
    registry.register(task_output::TaskOutputTool::new(std::sync::Arc::clone(&task_manager)))?;
    registry.register(task_stop::TaskStopTool::new(std::sync::Arc::clone(&task_manager)))?;
    Ok(registry)
}
