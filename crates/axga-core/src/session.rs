//! Session persistence — save/load conversations as JSONL files.
//!
//! Format: one JSON object per line, each line is an AgentMessage.

use axga_shared::types::AgentMessage;
use std::path::PathBuf;

/// Save a conversation to a JSONL file.
pub fn save_session(messages: &[AgentMessage], path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    for msg in messages {
        let json = serde_json::to_string(msg).unwrap_or_default();
        use std::io::Write;
        writeln!(file, "{json}")?;
    }
    Ok(())
}

/// Load a conversation from a JSONL file.
pub fn load_session(path: &PathBuf) -> std::io::Result<Vec<AgentMessage>> {
    let content = std::fs::read_to_string(path)?;
    let messages: Vec<AgentMessage> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Ok(messages)
}

/// List available sessions.
pub fn list_sessions(sessions_dir: &PathBuf) -> Vec<String> {
    if let Ok(entries) = std::fs::read_dir(sessions_dir) {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}
