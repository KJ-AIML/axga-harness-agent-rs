//! MemCtrl memory layer tool.
//!
//! Integrates the memctrl CLI (https://github.com/KJ-AIML/memctrl)
//! as a built-in axga tool. Stores and retrieves project knowledge
//! with provenance and confidence tracking.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

pub struct MemCtrlTool;

impl Tool for MemCtrlTool {
    fn name(&self) -> &str {
        "memctrl"
    }
    fn description(&self) -> &str {
        "Store and query project memory. Subcommands: add (store a fact), \
         query (search memories), list (show all), tree (show tree), \
         doctor (health check). Use --layer project for permanent facts, \
         --layer session for current session facts."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "query", "list", "tree", "doctor", "forget", "clear"],
                    "description": "Memory action to perform."
                },
                "content": {
                    "type": "string",
                    "description": "Text to store (for add) or question to search (for query)."
                },
                "layer": {
                    "type": "string",
                    "enum": ["project", "session", "user"],
                    "description": "Memory layer. project=permanent, session=7 days, user=90 days. Default: session."
                },
                "id": {
                    "type": "string",
                    "description": "Memory ID (for forget action)."
                }
            },
            "required": ["action"]
        })
    }
    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let action = input["action"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "memctrl".into(),
                    message: "missing 'action'".into(),
                })?;

            let content = input["content"].as_str().unwrap_or("");
            let layer = input["layer"].as_str().unwrap_or("session");

            // Build memctrl command
            let mut cmd = std::process::Command::new("memctrl");
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            match action {
                "add" => {
                    if content.is_empty() {
                        return Err(AxgaError::ToolError {
                            tool: "memctrl".into(),
                            message: "missing 'content' to add".into(),
                        });
                    }
                    cmd.arg("add").arg(content);
                    if layer != "session" {
                        cmd.arg("--layer").arg(layer);
                    }
                }
                "query" => {
                    if content.is_empty() {
                        return Err(AxgaError::ToolError {
                            tool: "memctrl".into(),
                            message: "missing 'content' to query".into(),
                        });
                    }
                    cmd.arg("query").arg(content);
                }
                "list" => {
                    cmd.arg("list");
                    if layer != "session" {
                        cmd.arg("--layer").arg(layer);
                    }
                }
                "tree" => {
                    cmd.arg("tree");
                }
                "doctor" => {
                    cmd.arg("doctor");
                }
                "forget" => {
                    let id = input["id"].as_str().unwrap_or("");
                    if id.is_empty() {
                        return Err(AxgaError::ToolError {
                            tool: "memctrl".into(),
                            message: "missing 'id' to forget".into(),
                        });
                    }
                    cmd.arg("forget").arg(id);
                }
                "clear" => {
                    cmd.arg("clear");
                    if layer != "session" {
                        cmd.arg("--layer").arg(layer);
                    }
                }
                _ => {
                    return Err(AxgaError::ToolError {
                        tool: "memctrl".into(),
                        message: format!("unknown action: {action}"),
                    });
                }
            }

            // Set PYTHONIOENCODING for Windows compatibility
            cmd.env("PYTHONIOENCODING", "utf-8");

            let output = tokio::task::spawn_blocking(move || cmd.output())
                .await
                .map_err(|e| AxgaError::ToolError {
                    tool: "memctrl".into(),
                    message: e.to_string(),
                })?
                .map_err(|e| AxgaError::ToolError {
                    tool: "memctrl".into(),
                    message: e.to_string(),
                })?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if stdout.trim().is_empty() {
                    Ok("Memory operation completed.".into())
                } else {
                    Ok(stdout)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                // memctrl sometimes outputs to stderr for info
                if !stdout.trim().is_empty() {
                    Ok(stdout)
                } else if stderr.contains("not found") || stderr.contains("No such file") {
                    Ok("memctrl is not installed. Install with: pip install memctrl && memctrl init".into())
                } else {
                    Ok(format!("{stdout} {stderr}"))
                }
            }
        })
    }
}
