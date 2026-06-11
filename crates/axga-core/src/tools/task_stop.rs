//! TaskStop tool — cancels a running background task.

use super::{TaskManager, Tool};
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct TaskStopTool {
    manager: Arc<TaskManager>,
}

impl TaskStopTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

impl Tool for TaskStopTool {
    fn name(&self) -> &str { "task_stop" }
    fn description(&self) -> &str {
        "Cancel/stop a running background task by its task_id. The task will be killed."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "ID of the background task to cancel." }
            },
            "required": ["task_id"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let mgr = Arc::clone(&self.manager);
        Box::pin(async move {
            let task_id = input["task_id"].as_u64().ok_or_else(|| AxgaError::ToolError {
                tool: "task_stop".into(),
                message: "missing 'task_id'".into(),
            })?;
            mgr.cancel(task_id)?;
            Ok(format!("Task {task_id} cancelled."))
        })
    }
}
