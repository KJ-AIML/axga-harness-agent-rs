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
pub mod goal;

pub use state::Conversation;
pub use agent_loop::{run_turn, run_turn_streaming, continue_turn_streaming, simple_chat, StreamHandler};
pub use tools::registry::ToolRegistry;
pub use tools::Tool;
pub use config::{Config, load_config, save_config};
pub use session::{save_session, load_session, list_sessions};
pub use orchestrator::Orchestrator;
pub use permission::{PermissionManager, PermissionMode, Permission};
pub use tools::cron::{CronScheduler, CronEvent};

use axga_shared::error::AxgaResult;

/// Build the default tool registry with all built-in tools.
/// `dangerous` bypasses shell denylist when true.
///
/// When `provider` and `model` are provided, also registers the `agent` and
/// `agent_swarm` tools so the LLM can spawn single/parallel sub-agents.
///
/// When `goal_manager` is provided, registers the goal tools:
/// `create_goal`, `get_goal`, `update_goal`, `set_goal_budget`.
pub fn build_default_registry(
    dangerous: bool,
    provider: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    base_url: Option<&str>,
    goal_manager: Option<std::sync::Arc<std::sync::Mutex<goal::GoalManager>>>,
) -> AxgaResult<ToolRegistry> {
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
    registry.register(plan::EnterPlanModeTool)?;
    registry.register(plan::ExitPlanModeTool)?;
    registry.register(ask_user::AskUserQuestionTool)?;
    registry.register(cron::CronCreateTool)?;
    registry.register(cron::CronListTool)?;
    registry.register(cron::CronDeleteTool)?;

    // Register agent and agent_swarm tools if provider/model info is available.
    if let (Some(provider), Some(model)) = (provider, model) {
        let orch = Orchestrator::new(registry.clone());
        registry.register(agent_swarm::AgentSwarmTool::new(
            orch,
            provider.to_string(),
            model.to_string(),
            api_key.map(|s| s.to_string()),
            base_url.map(|s| s.to_string()),
        ))?;

        let agent_orch = Orchestrator::new(registry.clone());
        registry.register(agent::AgentTool::new(
            agent_orch,
            provider.to_string(),
            model.to_string(),
            api_key.map(|s| s.to_string()),
            base_url.map(|s| s.to_string()),
        ))?;
    }

    // Register goal tools if a GoalManager is provided
    if let Some(gm) = goal_manager {
        registry.register(tools::goal::CreateGoalTool::new(std::sync::Arc::clone(&gm)))?;
        registry.register(tools::goal::GetGoalTool::new(std::sync::Arc::clone(&gm)))?;
        registry.register(tools::goal::UpdateGoalTool::new(std::sync::Arc::clone(&gm)))?;
        registry.register(tools::goal::SetGoalBudgetTool::new(std::sync::Arc::clone(&gm)))?;
    }

    Ok(registry)
}
