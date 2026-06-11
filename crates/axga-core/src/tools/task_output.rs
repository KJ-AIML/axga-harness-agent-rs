//! TaskOutput tool — reads stdout/stderr from a background task.

use super::{TaskManager, Tool};
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct TaskOutputTool {
    manager: Arc<TaskManager>,
}

impl TaskOutputTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

impl Tool for TaskOutputTool {
    fn name(&self) -> &str { "task_output" }
    fn description(&self) -> &str {
        "Read stdout/stderr output from a background task by its task_id. \
         Returns accumulated output so far; appends '[still running]' if not yet finished."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "ID of the background task." }
            },
            "required": ["task_id"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let mgr = Arc::clone(&self.manager);
        Box::pin(async move {
            let task_id = input["task_id"].as_u64().ok_or_else(|| AxgaError::ToolError {
                tool: "task_output".into(),
                message: "missing 'task_id'".into(),
            })?;
            mgr.read_output(task_id)
        })
    }
}
