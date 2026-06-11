//! Permission system for tool execution safety.
//!
//! Provides session-level approval memory so the user can approve/deny
//! tools once and have that decision remembered for the session.
//!
//! # Read-only tools
//! `read_file`, `list_directory`, `grep`, `glob`, `diff`, `memctrl` are
//! always auto-approved — they don't modify the filesystem or network.

use std::collections::HashSet;

/// Tool permission level.
#[derive(Debug, Clone, PartialEq)]
pub enum Permission {
    /// Auto-approve without asking
    Allow,
    /// Ask the user before executing
    Ask,
    /// Deny execution
    Deny,
}

/// Permission mode for the session.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionMode {
    /// Ask for every write/network/shell tool
    Manual,
    /// Auto-approve all tools (no questions)
    Auto,
}

/// Manages tool permissions with session-level approval memory.
///
/// Uses interior mutability (Mutex) so it can be shared via `Arc`.
pub struct PermissionManager {
    mode: std::sync::Mutex<PermissionMode>,
    /// Tool names that have been approved for this session
    approved: std::sync::Mutex<HashSet<String>>,
    /// Tool names that have been denied for this session
    denied: std::sync::Mutex<HashSet<String>>,
    /// Read-only tool names (always auto-approved)
    read_only: HashSet<&'static str>,
}

impl PermissionManager {
    /// Create a new permission manager with default safe settings.
    pub fn new(mode: PermissionMode) -> Self {
        let mut read_only = HashSet::new();
        read_only.insert("read_file");
        read_only.insert("list_directory");
        read_only.insert("grep");
        read_only.insert("glob");
        read_only.insert("diff");
        read_only.insert("memctrl");
        Self {
            mode: std::sync::Mutex::new(mode),
            approved: std::sync::Mutex::new(HashSet::new()),
            denied: std::sync::Mutex::new(HashSet::new()),
            read_only,
        }
    }

    /// Check if a tool needs approval. Returns the required action.
    pub fn check(&self, tool_name: &str) -> Permission {
        let mode = self.mode.lock().unwrap();
        match *mode {
            PermissionMode::Auto => Permission::Allow,
            PermissionMode::Manual => {
                // Read-only tools are always allowed
                if self.read_only.contains(tool_name) {
                    return Permission::Allow;
                }
                // Check session approvals
                if self.approved.lock().unwrap().contains(tool_name) {
                    return Permission::Allow;
                }
                if self.denied.lock().unwrap().contains(tool_name) {
                    return Permission::Deny;
                }
                Permission::Ask
            }
        }
    }

    /// Approve a tool for this session.
    pub fn approve(&self, tool_name: &str) {
        self.approved.lock().unwrap().insert(tool_name.to_string());
    }

    /// Approve all tools for the rest of this session (switch to Auto mode).
    pub fn approve_all(&self) {
        *self.mode.lock().unwrap() = PermissionMode::Auto;
    }

    /// Deny a tool for this session.
    pub fn deny(&self, tool_name: &str) {
        self.denied.lock().unwrap().insert(tool_name.to_string());
    }

    /// Get the current mode.
    pub fn mode(&self) -> PermissionMode {
        self.mode.lock().unwrap().clone()
    }

    /// Set the permission mode.
    pub fn set_mode(&self, mode: PermissionMode) {
        *self.mode.lock().unwrap() = mode;
    }

    /// Pre-screen a batch of tool calls, splitting them into approved and pending.
    /// Returns (approved_calls, pending_calls_that_need_asking).
    pub fn check_batch(
        &self,
        calls: &[axga_shared::types::ToolCall],
    ) -> (Vec<axga_shared::types::ToolCall>, Vec<axga_shared::types::ToolCall>) {
        let mut approved = Vec::new();
        let mut pending = Vec::new();
        for call in calls {
            match self.check(&call.name) {
                Permission::Allow => approved.push(call.clone()),
                Permission::Ask => pending.push(call.clone()),
                Permission::Deny => {} // silently skip denied tools
            }
        }
        (approved, pending)
    }
}
