use axga_core::tools::registry::ToolRegistry;
use axga_core::tools::{fs, shell, code};
use axga_core::state::Conversation;

#[test]
fn tool_registry_register_and_lookup() {
    let mut registry = ToolRegistry::new();
    assert!(registry.is_empty());
    registry.register(fs::ReadFileTool).unwrap();
    registry.register(fs::WriteFileTool).unwrap();
    registry.register(shell::ShellTool).unwrap();
    assert_eq!(registry.len(), 3);
    assert!(registry.get("read_file").is_some());
    assert!(registry.get("write_file").is_some());
    assert!(registry.get("execute_shell").is_some());
    assert!(registry.get("nonexistent").is_none());
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
fn all_seven_tools_register() {
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool).unwrap();
    registry.register(fs::WriteFileTool).unwrap();
    registry.register(fs::ListDirectoryTool).unwrap();
    registry.register(shell::ShellTool).unwrap();
    registry.register(code::GrepTool).unwrap();
    registry.register(code::GlobTool).unwrap();
    registry.register(code::DiffTool).unwrap();
    assert_eq!(registry.len(), 7);
}
