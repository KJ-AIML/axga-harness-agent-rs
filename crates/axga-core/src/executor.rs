//! Tool execution loop — the core of the agent runtime.
//!
//! # Flow
//! 1. LLM returns tool calls → executor runs them (parallel by default)
//! 2. Each tool output is truncated at `MAX_TOOL_OUTPUT_LEN`
//! 3. Results flow through a bounded `mpsc` channel (TOOL_CHANNEL_CAP = 100)
//! 4. After all tools complete, conversation state is updated
//!
//! # Dedup Detection
//! Tracks consecutive identical tool calls (name + args prefix) and intervenes:
//! - streak ≥ 3 → appends a warning to the result
//! - streak ≥ 5 → "You have called this same tool 5 times"
//! - streak ≥ 8 → "Dead end: this tool has been called 8 times with same args"
//! - streak ≥ 12 → force-stops the turn

use axga_shared::error::AxgaResult;
use axga_shared::limits;
use axga_shared::types::ToolResult;
use crate::tools::registry::ToolRegistry;
use crate::permission::{Permission, PermissionManager};
use axga_shared::types::ToolCall;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Tracks consecutive identical tool calls to detect stuck loops.
pub struct DedupTracker {
    last_key: Option<String>,
    consecutive_count: u32,
}

impl DedupTracker {
    pub fn new() -> Self {
        Self {
            last_key: None,
            consecutive_count: 0,
        }
    }

    /// Check a tool call against the dedup tracker.
    ///
    /// Returns `(warning, force_stop)`. If the same key matches the previous
    /// call, the streak increments; otherwise it resets to 1.
    pub fn check(&mut self, tool_name: &str, args: &serde_json::Value) -> (Option<String>, bool) {
        let args_str = serde_json::to_string(args).unwrap_or_default();
        let key = if args_str.len() <= 100 {
            format!("{tool_name}|{args_str}")
        } else {
            format!("{}|{}", tool_name, &args_str[..100])
        };

        match &self.last_key {
            Some(last) if *last == key => {
                self.consecutive_count += 1;
            }
            _ => {
                self.last_key = Some(key);
                self.consecutive_count = 1;
            }
        }

        debug!(
            tool = %tool_name,
            streak = self.consecutive_count,
            "dedup check"
        );

        match self.consecutive_count {
            0..=2 => (None, false),
            3..=4 => (
                Some(format!(
                    "[dedup] This same tool call has been repeated {} times consecutively",
                    self.consecutive_count
                )),
                false,
            ),
            5..=7 => (
                Some(format!(
                    "You have called this same tool {} times",
                    self.consecutive_count
                )),
                false,
            ),
            8..=11 => (
                Some(format!(
                    "Dead end: this tool has been called {} times with same args",
                    self.consecutive_count
                )),
                false,
            ),
            _ => (
                Some(format!(
                    "Force-stop: tool called {} times with same args",
                    self.consecutive_count
                )),
                true,
            ),
        }
    }
}

impl Default for DedupTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a batch of tool calls and return results.
///
/// # Memory
/// Uses a bounded channel (`TOOL_CHANNEL_CAP = 100`) to prevent
/// unbounded buffering of tool outputs.
///
/// # Dedup
/// Checks each tool call against the `DedupTracker`. If a call has been
/// repeated too many times, a warning is appended or the turn is force-stopped.
pub async fn execute_tool_calls(
    registry: &ToolRegistry,
    calls: &[ToolCall],
    permissions: Option<Arc<PermissionManager>>,
    dedup: &mut DedupTracker,
) -> AxgaResult<Vec<ToolResult>> {
    // First pass: check dedup for each call, collecting warnings and detecting force-stop
    let mut dedup_warnings: Vec<Option<String>> = Vec::with_capacity(calls.len());

    for call in calls {
        let (warning, force_stop) = dedup.check(&call.name, &call.arguments);
        if force_stop {
            warn!(
                tool = %call.name,
                streak = dedup.consecutive_count,
                "dedup force-stop triggered"
            );
            return Ok(vec![ToolResult {
                tool_call_id: call.id.clone(),
                content: warning.unwrap_or_else(|| {
                    "Force-stop: repeated tool call detected".into()
                }),
                is_error: true,
                force_stop: true,
            }]);
        }
        dedup_warnings.push(warning);
    }

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
            Some(Ok(result)) => {
                results.push(result);
            }
            Some(Err(e)) => {
                results.push(ToolResult {
                    tool_call_id: String::new(),
                    content: format!("Error: {e}"),
                    is_error: true,
                    force_stop: false,
                });
            }
            None => break,
        }
    }

    // Apply dedup warnings to results by matching tool_call_id against calls
    for result in &mut results {
        if let Some(idx) = calls.iter().position(|tc| tc.id == result.tool_call_id) {
            if let Some(Some(warning)) = dedup_warnings.get(idx) {
                result.content = format!("{}\n{}", warning, result.content);
            }
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
                    force_stop: false,
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
        force_stop: false,
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
