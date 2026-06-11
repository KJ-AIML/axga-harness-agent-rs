//! `axga-core` — Agent runtime.

pub mod state;
pub mod context;
pub mod agent_loop;
pub mod executor;
pub mod tools;
pub mod session;
pub mod retry;
pub mod config;

pub use state::Conversation;
pub use agent_loop::run_turn;
pub use tools::registry::ToolRegistry;
pub use tools::Tool;
pub use config::{Config, load_config, save_config};
pub use session::{save_session, load_session, list_sessions};

use axga_shared::error::AxgaResult;

/// Build the default tool registry with all 10 built-in tools.
/// `dangerous` bypasses shell denylist when true.
pub fn build_default_registry(dangerous: bool) -> AxgaResult<ToolRegistry> {
    use tools::*;
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool::new(dangerous))?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl_native::MemCtrlTool::new()?)?;
    registry.register(web_search::WebSearchTool)?;
    registry.register(fetch_url::FetchUrlTool)?;
    Ok(registry)
}
