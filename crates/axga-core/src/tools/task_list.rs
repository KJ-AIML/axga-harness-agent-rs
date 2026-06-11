//! TaskList tool — lists all background tasks and their statuses.

use super::{TaskManager, TaskStatus, Tool};
use axga_shared::error::AxgaResult;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct TaskListTool {
    manager: Arc<TaskManager>,
}

impl TaskListTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

impl Tool for TaskListTool {
    fn name(&self) -> &str { "task_list" }
    fn description(&self) -> &str {
        "List all background tasks with their IDs, commands, and statuses (Running, Completed, Cancelled, Failed)."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
    fn execute(&self, _input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let mgr = Arc::clone(&self.manager);
        Box::pin(async move {
            let tasks = mgr.list();
            if tasks.is_empty() {
                return Ok("No background tasks.".to_string());
            }
            let mut out = String::new();
            for t in tasks {
                let status_str = match t.status {
                    TaskStatus::Running => "Running".to_string(),
                    TaskStatus::Completed { exit_code } => format!("Completed({exit_code})"),
                    TaskStatus::Cancelled => "Cancelled".to_string(),
                    TaskStatus::Failed { reason } => format!("Failed: {reason}"),
                };
                out.push_str(&format!("[{}] {} — {}\n", t.id, t.command, status_str));
            }
            Ok(out)
        })
    }
}
