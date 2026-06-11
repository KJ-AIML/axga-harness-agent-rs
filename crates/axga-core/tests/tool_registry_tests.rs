use axga_core::tools::registry::ToolRegistry;
use axga_core::tools::{fs, shell, code, memctrl_native, web_search, fetch_url, task_list, task_output, task_stop, TaskManager};
use axga_core::state::Conversation;

#[test]
fn tool_registry_register_and_lookup() {
    let tm = TaskManager::new();
    let mut registry = ToolRegistry::new();
    assert!(registry.is_empty());
    registry.register(fs::ReadFileTool).unwrap();
    registry.register(fs::WriteFileTool).unwrap();
    registry.register(shell::ShellTool::new(false, std::sync::Arc::clone(&tm))).unwrap();
    assert_eq!(registry.len(), 3);
}

#[test]
fn duplicate_tool_rejected() {
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool).unwrap();
    assert!(registry.register(fs::ReadFileTool).is_err());
}

#[test]
fn conversation_reset() {
    let mut conv = Conversation::new();
    conv.push(axga_shared::types::AgentMessage::User { content: "hello".into() });
    assert_eq!(conv.len(), 1);
    conv.reset();
    assert_eq!(conv.len(), 0);
}

#[test]
fn all_thirteen_tools_register() {
    let tm = TaskManager::new();
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool).unwrap();
    registry.register(fs::WriteFileTool).unwrap();
    registry.register(fs::ListDirectoryTool).unwrap();
    registry.register(shell::ShellTool::new(false, std::sync::Arc::clone(&tm))).unwrap();
    registry.register(code::GrepTool).unwrap();
    registry.register(code::GlobTool).unwrap();
    registry.register(code::DiffTool).unwrap();
    registry.register(memctrl_native::MemCtrlTool::new().unwrap()).unwrap();
    registry.register(web_search::WebSearchTool).unwrap();
    registry.register(fetch_url::FetchUrlTool).unwrap();
    registry.register(task_list::TaskListTool::new(std::sync::Arc::clone(&tm))).unwrap();
    registry.register(task_output::TaskOutputTool::new(std::sync::Arc::clone(&tm))).unwrap();
    registry.register(task_stop::TaskStopTool::new(std::sync::Arc::clone(&tm))).unwrap();
    assert_eq!(registry.len(), 13);
}
