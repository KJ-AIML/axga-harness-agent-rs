//! Plan mode tools: EnterPlanMode and ExitPlanMode.
//!
//! Plan mode is a read-only exploration phase. When active, write/shell/network-mutating
//! tools are blocked. The agent can only use read-only tools (read_file, list_directory,
//! grep, glob, diff, web_search, fetch_url, memctrl, and task inspection).
//!
//! Use `EnterPlanModeTool` to activate plan mode and `ExitPlanModeTool` to deactivate.
//! A plan file should be written before exiting.

use super::Tool;
use super::PLAN_MODE;
use axga_shared::error::AxgaResult;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;

pub struct EnterPlanModeTool;

impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    fn description(&self) -> &str {
        "Enter plan mode — a read-only exploration phase. \
         Write/shell/network-mutating tools are blocked. \
         Use only read-only tools (read_file, list_directory, grep, glob, \
         diff, web_search, fetch_url, memctrl query) to explore the codebase. \
         Write a plan file when done, then call exit_plan_mode to execute."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn execute(
        &self,
        _input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let was_already = PLAN_MODE.swap(true, Ordering::SeqCst);
            if was_already {
                Ok("Already in plan mode. Write/shell tools are blocked.".into())
            } else {
                Ok(
                    "Entered plan mode. Write/shell/network-mutating tools are now blocked.\n\
                     You may use read-only tools (read_file, list_directory, grep, glob, \
                     diff, web_search, fetch_url, memctrl query, task_list, task_output) \
                     to explore and understand the codebase.\n\
                     Write a plan file and call exit_plan_mode when ready to execute."
                        .into(),
                )
            }
        })
    }
}

pub struct ExitPlanModeTool;

impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Exit plan mode and return to normal operation. \
         Optionally provide a plan_id for tracking. \
         All tools (write, shell, network) are re-enabled."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "Optional plan identifier for tracking"
                }
            },
            "required": []
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let was_active = PLAN_MODE.swap(false, Ordering::SeqCst);
            let plan_id = input["plan_id"].as_str().unwrap_or("(untitled)");
            if was_active {
                Ok(format!(
                    "Exited plan mode. All tools are now re-enabled. Plan ID: {plan_id}"
                ))
            } else {
                Ok("Was not in plan mode. All tools are available.".into())
            }
        })
    }
}

/// Returns true if the given tool name is allowed to execute in plan mode
/// (read-only tools).
pub fn is_plan_mode_safe(tool_name: &str) -> bool {
    matches!(
        tool_name,
        |"read_file"
        | "list_directory"
        | "grep"
        | "glob"
        | "diff"
        | "web_search"
        | "fetch_url"
        | "memctrl"
        | "task_list"
        | "task_output"
        | "task_stop"
        | "enter_plan_mode"
        | "exit_plan_mode"
        | "ask_user_question"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_plan_mode_tool_name() {
        let tool = EnterPlanModeTool;
        assert_eq!(tool.name(), "enter_plan_mode");
    }

    #[test]
    fn exit_plan_mode_tool_name() {
        let tool = ExitPlanModeTool;
        assert_eq!(tool.name(), "exit_plan_mode");
    }

    #[test]
    fn plan_mode_safe_tools() {
        assert!(is_plan_mode_safe("read_file"));
        assert!(is_plan_mode_safe("list_directory"));
        assert!(is_plan_mode_safe("grep"));
        assert!(is_plan_mode_safe("glob"));
        assert!(is_plan_mode_safe("diff"));
        assert!(is_plan_mode_safe("web_search"));
        assert!(is_plan_mode_safe("fetch_url"));
        assert!(is_plan_mode_safe("memctrl"));
        assert!(is_plan_mode_safe("task_list"));
        assert!(is_plan_mode_safe("enter_plan_mode"));
        assert!(is_plan_mode_safe("exit_plan_mode"));
        assert!(is_plan_mode_safe("ask_user_question"));
    }

    #[test]
    fn plan_mode_blocked_tools() {
        assert!(!is_plan_mode_safe("write_file"));
        assert!(!is_plan_mode_safe("shell"));
        assert!(!is_plan_mode_safe("agent"));
        assert!(!is_plan_mode_safe("agent_swarm"));
    }

    #[test]
    fn enter_and_exit_plan_mode_flag() {
        // Reset first
        PLAN_MODE.store(false, Ordering::SeqCst);

        // Simulate what EnterPlanMode does
        PLAN_MODE.swap(true, Ordering::SeqCst);
        assert!(PLAN_MODE.load(Ordering::SeqCst));

        // Simulate what ExitPlanMode does
        PLAN_MODE.swap(false, Ordering::SeqCst);
        assert!(!PLAN_MODE.load(Ordering::SeqCst));

        // Re-entering should work too
        PLAN_MODE.swap(true, Ordering::SeqCst);
        assert!(PLAN_MODE.load(Ordering::SeqCst));

        // Cleanup
        PLAN_MODE.store(false, Ordering::SeqCst);
    }

    #[test]
    fn enter_plan_mode_already_active_returns_info() {
        PLAN_MODE.store(false, Ordering::SeqCst);
        let tool = EnterPlanModeTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(serde_json::json!({}))).unwrap();
        assert!(result.contains("Entered plan mode"));
        // Now enter again while already in plan mode
        let result2 = rt.block_on(tool.execute(serde_json::json!({}))).unwrap();
        assert!(result2.contains("Already in plan mode"));
        // Cleanup
        PLAN_MODE.store(false, Ordering::SeqCst);
    }
}
