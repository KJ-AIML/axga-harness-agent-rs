//! Background task manager — shared state for shell background tasks.
//!
//! Tracks spawned processes by handle ID, collects stdout/stderr,
//! and exposes query/cancel operations.

use axga_shared::error::{AxgaError, AxgaResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::process::Child;

/// Status of a background task.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed { exit_code: i32 },
    Cancelled,
    Failed { reason: String },
}

/// Handle to a managed background task.
pub struct TaskHandle {
    pub id: u64,
    pub command: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: TaskStatus,
    pub child: Option<Child>,
}

/// Thread-safe registry of background tasks.
pub struct TaskManager {
    tasks: Mutex<HashMap<u64, TaskHandle>>,
    next_id: AtomicU64,
}

impl TaskManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tasks: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        })
    }

    /// Allocate a new ID and insert a task handle.
    pub fn insert(&self, handle: TaskHandle) -> u64 {
        let id = handle.id;
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(id, handle);
        id
    }

    /// Look up a task by ID (clones data for read-only tools).
    pub fn get(&self, id: u64) -> AxgaResult<TaskInfo> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(&id).map(|h| TaskInfo {
            id: h.id,
            command: h.command.clone(),
            status: h.status.clone(),
        }).ok_or_else(|| AxgaError::ToolError {
            tool: "task".into(),
            message: format!("task {id} not found"),
        })
    }

    /// List all tasks.
    pub fn list(&self) -> Vec<TaskInfo> {
        let tasks = self.tasks.lock().unwrap();
        tasks.values().map(|h| TaskInfo {
            id: h.id,
            command: h.command.clone(),
            status: h.status.clone(),
        }).collect()
    }

    /// Read stdout from a task so far.
    pub fn read_output(&self, id: u64) -> AxgaResult<String> {
        let mut tasks = self.tasks.lock().unwrap();
        let h = tasks.get_mut(&id).ok_or_else(|| AxgaError::ToolError {
            tool: "task_output".into(),
            message: format!("task {id} not found"),
        })?;
        // Also try to read more from child if still running
        if let Some(ref mut child) = h.child {
            if let Ok(Some(status)) = child.try_wait() {
                h.status = TaskStatus::Completed {
                    exit_code: status.code().unwrap_or(-1),
                };
                h.child = None;
            }
        }
        let stdout = String::from_utf8_lossy(&h.stdout).to_string();
        let stderr = String::from_utf8_lossy(&h.stderr).to_string();
        let mut result = stdout;
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("[stderr]\n");
            result.push_str(&stderr);
        }
        match &h.status {
            TaskStatus::Completed { exit_code } => {
                result.push_str(&format!("\nExit code: {exit_code}"));
            }
            TaskStatus::Failed { reason } => {
                result.push_str(&format!("\nFailed: {reason}"));
            }
            TaskStatus::Cancelled => {
                result.push_str("\nCancelled");
            }
            TaskStatus::Running => {
                result.push_str("\n[still running]");
            }
        }
        Ok(result)
    }

    /// Cancel/kill a task.
    pub fn cancel(&self, id: u64) -> AxgaResult<()> {
        let mut tasks = self.tasks.lock().unwrap();
        let h = tasks.get_mut(&id).ok_or_else(|| AxgaError::ToolError {
            tool: "task_stop".into(),
            message: format!("task {id} not found"),
        })?;
        if let Some(ref mut child) = h.child.take() {
            // Best-effort kill; ignore errors
            let _ = child.start_kill();
            h.status = TaskStatus::Cancelled;
        }
        Ok(())
    }

    /// Reserve an ID for a new task.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Append a line to a task's stdout or stderr buffer.
    pub fn append_line(&self, id: u64, line: &[u8], is_stdout: bool) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(h) = tasks.get_mut(&id) {
            let buf = if is_stdout { &mut h.stdout } else { &mut h.stderr };
            if !buf.is_empty() {
                buf.extend_from_slice(b"\n");
            }
            buf.extend_from_slice(line);
        }
    }

    /// Mark a task as completed with the given exit code.
    pub fn mark_completed(&self, id: u64, exit_code: i32) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(h) = tasks.get_mut(&id) {
            h.status = TaskStatus::Completed { exit_code };
            h.child = None;
        }
    }
}

/// Read-only snapshot of a task for listing/inspection.
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: u64,
    pub command: String,
    pub status: TaskStatus,
}
