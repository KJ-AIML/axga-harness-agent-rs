//! Tool execution loop — the core of the agent runtime.
//!
//! # Flow
//! 1. LLM returns tool calls → executor runs them (parallel by default)
//! 2. Each tool output is truncated at `MAX_TOOL_OUTPUT_LEN`
//! 3. Results flow through a bounded `mpsc` channel (TOOL_CHANNEL_CAP = 100)
//! 4. After all tools complete, conversation state is updated

use axga_shared::error::AxgaResult;
use axga_shared::limits;
use axga_shared::types::ToolResult;
use crate::tools::registry::ToolRegistry;
use crate::permission::{Permission, PermissionManager};
use axga_shared::types::ToolCall;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Execute a batch of tool calls and return results.
///
/// # Memory
/// Uses a bounded channel (`TOOL_CHANNEL_CAP = 100`) to prevent
/// unbounded buffering of tool outputs.
pub async fn execute_tool_calls(
    registry: &ToolRegistry,
    calls: &[ToolCall],
    permissions: Option<Arc<PermissionManager>>,
) -> AxgaResult<Vec<ToolResult>> {
    let (tx, mut rx) = mpsc::channel::<Result<ToolResult, axga_shared::error::AxgaError>>(
        limits::TOOL_CHANNEL_CAP,
    );

    // Spawn each tool call concurrently
    for call in calls {
        let registry = registry.clone();
        let call = call.clone();
        let tx = tx.clone();
        let permissions = permissions.clone();

        tokio::spawn(async move {
            let result = execute_single_tool(&registry, &call, permissions).await;
            let _ = tx.send(result).await;
        });
    }

    // Collect results
    let mut results = Vec::with_capacity(calls.len());
    for _ in 0..calls.len() {
        match rx.recv().await {
            Some(Ok(result)) => results.push(result),
            Some(Err(e)) => {
                results.push(ToolResult {
                    tool_call_id: String::new(),
                    content: format!("Error: {e}"),
                    is_error: true,
                });
            }
            None => break,
        }
    }

    Ok(results)
}

async fn execute_single_tool(
    registry: &ToolRegistry,
    call: &ToolCall,
    permissions: Option<Arc<PermissionManager>>,
) -> AxgaResult<ToolResult> {
    debug!(tool = %call.name, id = %call.id, "executing tool");

    // Check permissions before executing
    if let Some(ref perms) = permissions {
        match perms.check(&call.name) {
            Permission::Allow => {
                // Proceed normally
            }
            Permission::Ask => {
                // No TUI approval dialog available yet — treat as Allow for now
                warn!(tool = %call.name, "permission check returned Ask but no handler available; allowing");
            }
            Permission::Deny => {
                warn!(tool = %call.name, "tool denied by permission manager");
                return Ok(ToolResult {
                    tool_call_id: call.id.clone(),
                    content: format!("Permission denied: tool '{}' is blocked by session policy", call.name),
                    is_error: true,
                });
            }
        }
    }

    let tool = registry
        .get(&call.name)
        .ok_or_else(|| axga_shared::error::AxgaError::ToolError {
            tool: call.name.clone(),
            message: "tool not found in registry".into(),
        })?;

    let output = tool.execute(call.arguments.clone()).await?;

    // Truncate output to limit
    let content = truncate_output(&output, limits::MAX_TOOL_OUTPUT_LEN);

    Ok(ToolResult {
        tool_call_id: call.id.clone(),
        content,
        is_error: false,
    })
}

fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        let mut truncated = output[..max_len].to_string();
        truncated.push_str(&format!(
            "\n... [truncated {} bytes, original: {} bytes]",
            output.len() - max_len,
            output.len()
        ));
        truncated
    }
}
