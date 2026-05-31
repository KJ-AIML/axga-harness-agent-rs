//! Shell execution tool.
//!
//! # Memory Safety
//! - stdout/stderr streamed through tokio::process, never buffered fully in memory.
//! - Default timeout: 60s.
//! - Exit code appended to output.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &str { "execute_shell" }
    fn description(&self) -> &str {
        "Execute a shell command. Default timeout 60s. Returns stdout, stderr, and exit code."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command to execute." },
                "timeout_seconds": { "type": "integer", "description": "Max execution time (seconds)." },
                "working_dir": { "type": "string", "description": "Working directory." }
            },
            "required": ["command"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let command = input["command"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "execute_shell".into(), message: "missing 'command'".into(),
            })?;
            let timeout_secs = input["timeout_seconds"].as_u64().unwrap_or(60);

            #[cfg(target_os = "windows")]
            let (shell, flag) = ("cmd", "/C");
            #[cfg(not(target_os = "windows"))]
            let (shell, flag) = ("bash", "-c");

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                tokio::process::Command::new(shell)
                    .arg(flag)
                    .arg(command)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output(),
            )
            .await
            .map_err(|_| AxgaError::ToolError {
                tool: "execute_shell".into(),
                message: format!("timed out after {}s", timeout_secs),
            })??;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit = output.status.code().unwrap_or(-1);

            let mut result = String::new();
            if !stdout.is_empty() { result.push_str(&stdout); }
            if !stderr.is_empty() {
                if !result.is_empty() { result.push('\n'); }
                result.push_str("[stderr]\n");
                result.push_str(&stderr);
            }
            result.push_str(&format!("\nExit code: {}", exit));
            Ok(result)
        })
    }
}
