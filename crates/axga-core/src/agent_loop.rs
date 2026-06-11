//! Agent loop — the main runtime tying LLM, tools, and conversation state.
//!
//! # Flow
//! 1. User prompt → push to Conversation
//! 2. Build OpenAI/Anthropic request from conversation history
//! 3. Stream LLM response → collect text + tool calls
//! 4. If tool calls → execute via ToolRegistry → push results → loop
//! 5. If no tool calls → done, return final text
//!
//! # Memory
//! - Each loop iteration yields a Collector that drains the stream without buffering.
//! - Tool outputs truncated at MAX_TOOL_OUTPUT_LEN before pushing to conversation.

use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::limits;
use axga_shared::types::{
    AgentMessage, AssistantContent, StreamEvent, ToolCall, ToolDefinition,
};
use axga_ai::request::RequestBuilder;
use axga_ai::Provider;
use crate::state::Conversation;
use crate::executor::{execute_tool_calls, DedupTracker};
use crate::permission::PermissionManager;
use crate::ToolRegistry;
use futures::{Stream, StreamExt};
use std::sync::Arc;
use tracing::{debug, warn};

/// Result of one full turn (may include multiple LLM calls + tool executions).
pub struct TurnResult {
    pub final_text: String,
    pub tool_calls_made: Vec<String>,
    pub total_tokens: u32,
    /// Tool calls that need user approval before execution.
    pub pending_approvals: Vec<ToolCall>,
}

/// Handler for streaming events during an agent turn.
/// Implement this to provide real-time UI updates.
pub trait StreamHandler: Send {
    /// Called for each text delta as it arrives from the LLM stream.
    fn on_text_delta(&mut self, text: &str);
    /// Called for each tool-call delta fragment during streaming.
    fn on_tool_call_delta(&mut self, id: &str, name: &str, args: &str);
    /// Called after a tool has finished executing, with the truncated result.
    fn on_tool_call_result(&mut self, name: &str, result: &str);
    /// Called when the LLM starts thinking (before the first token).
    fn on_thinking(&mut self);
    /// Called when the entire turn is complete.
    fn on_done(&mut self);
}

/// Run a single turn of the agent: user input → LLM → tools → loop → final response.
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    provider_type: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    conversation: &mut Conversation,
    user_input: &str,
    tools: &ToolRegistry,
    system_prompt: Option<&str>,
    max_turns: usize,
    permissions: Option<Arc<PermissionManager>>,
) -> AxgaResult<TurnResult> {
    // Push user message
    conversation.push(AgentMessage::User {
        content: user_input.to_string(),
    });

    let mut final_text = String::new();
    let mut tool_calls_made: Vec<String> = Vec::new();
    let mut total_tokens: u32 = 0;
    let mut dedup = DedupTracker::new();

    for turn in 0..max_turns {
        debug!(turn, "agent loop iteration");

        // Build tool definitions from registry
        let tool_defs: Vec<ToolDefinition> = tools
            .names()
            .filter_map(|name| tools.get(name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect();

        // Stream LLM response
        let messages: Vec<AgentMessage> = conversation.messages().cloned().collect();
        let request = RequestBuilder::new(model, &messages)
            .with_max_tokens(4096);

        let request = if let Some(sys) = system_prompt {
            request.with_system_prompt(sys)
        } else {
            request
        };

        let request = if !tool_defs.is_empty() {
            request.with_tools(&tool_defs)
        } else {
            request
        };

        let provider: Box<dyn Provider> = match provider_type {
            "openai" => {
                Box::new(axga_ai::providers::openai::OpenAiProvider::new(
                    api_key.map(|s| s.to_string()),
                    base_url.map(|s| s.to_string()),
                )?)
            }
            "deepseek" => {
                Box::new(axga_ai::providers::deepseek::DeepSeekProvider::new(
                    api_key.map(|s| s.to_string()),
                    base_url.map(|s| s.to_string()),
                )?)
            }
            "anthropic" => {
                Box::new(axga_ai::providers::anthropic::AnthropicProvider::new(
                    api_key.map(|s| s.to_string()),
                )?)
            }
            _ => return Err(AxgaError::Config(format!("unknown provider: {provider_type}"))),
        };

        let stream = provider.stream_chat(&request).await?;

        // Collect stream events
        let (text, tool_calls, token_count) = collect_stream(stream).await?;
        total_tokens += token_count;

        if !text.is_empty() {
            final_text = text.clone();
        }

        // No tool calls → turn is done
        if tool_calls.is_empty() {
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent {
                    text: Some(text),
                    tool_calls: None,
                    thinking: None,
                },
            });
            break;
        }

        // Execute tool calls — filter empty IDs
        let valid_tool_calls: Vec<ToolCall> = tool_calls
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .collect();

        if valid_tool_calls.is_empty() {
            // No valid tool calls — treat as text-only response
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent { text: Some(text), tool_calls: None, thinking: None },
            });
            break;
        }

        // Pre-screen tool calls for permissions
        let (approved_tc, pending_tc) = if let Some(ref perms) = permissions {
            perms.check_batch(&valid_tool_calls)
        } else {
            (valid_tool_calls.clone(), vec![])
        };

        // Execute approved tool calls
        let results = if !approved_tc.is_empty() {
            execute_tool_calls(tools, &approved_tc, permissions.clone(), &mut dedup).await?
        } else {
            vec![]
        };

        let should_stop = results.iter().any(|r| r.force_stop);

        for tc in &approved_tc {
            tool_calls_made.push(tc.name.clone());
        }

        // Build combined tool calls for the assistant message (approved + pending)
        let mut all_tool_calls = approved_tc.clone();
        all_tool_calls.extend(pending_tc.clone());

        conversation.push(AgentMessage::Assistant {
            content: AssistantContent {
                text: Some(text),
                tool_calls: Some(all_tool_calls),
                thinking: None,
            },
        });

        for result in results {
            if result.tool_call_id.is_empty() { continue; }
            conversation.push(AgentMessage::Tool {
                tool_call_id: result.tool_call_id,
                content: truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN),
            });
        }

        if should_stop {
            warn!("dedup force-stop: breaking agent loop");
            break;
        }

        // Return pending approvals if any
        if !pending_tc.is_empty() {
            conversation.set_state(axga_shared::types::AgentState::WaitingForTools);
            return Ok(TurnResult {
                final_text,
                tool_calls_made,
                total_tokens,
                pending_approvals: pending_tc,
            });
        }
    }

    conversation.set_state(axga_shared::types::AgentState::Finished);

    Ok(TurnResult {
        final_text,
        tool_calls_made,
        total_tokens,
        pending_approvals: vec![],
    })
}

/// Drain a stream of StreamEvents into collected text, tool calls, and token count.
async fn collect_stream<S>(mut stream: S) -> AxgaResult<(String, Vec<ToolCall>, u32)>
where
    S: Stream<Item = AxgaResult<StreamEvent>> + Unpin,
{
    let mut text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut current_tc: Option<ToolCall> = None;
    let mut tokens: u32 = 0;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta { text: t } => {
                text.push_str(&t);
            }
            StreamEvent::ToolCallDelta { id, name, args_fragment } => {
                if id.is_empty() {
                    // ID hasn't arrived yet — merge args into current
                    if let Some(ref mut tc) = current_tc {
                        if let serde_json::Value::String(ref mut existing) = tc.arguments {
                            existing.push_str(&name);
                            existing.push_str(&args_fragment);
                        }
                    }
                } else if current_tc.as_ref().is_none_or(|tc| tc.id != id) {
                    if let Some(tc) = current_tc.take() {
                        tool_calls.push(tc);
                    }
                    current_tc = Some(ToolCall {
                        id,
                        name,
                        arguments: serde_json::Value::String(args_fragment),
                    });
                } else if let Some(ref mut tc) = current_tc {
                    if let serde_json::Value::String(ref mut existing) = tc.arguments {
                        existing.push_str(&args_fragment);
                    }
                }
            }
            StreamEvent::ThinkingDelta { .. } => {}
            StreamEvent::Usage { input_tokens, output_tokens } => {
                tokens += input_tokens + output_tokens;
            }
            StreamEvent::Done => break,
            StreamEvent::Stop { reason } => {
                debug!(%reason, "stream stopped");
                break;
            }
            StreamEvent::Error { message } => {
                return Err(AxgaError::LlmProvider(message));
            }
        }
    }

    // Finalize last tool call — validate before using
    if let Some(tc) = current_tc.take() {
        if !tc.id.is_empty() && !tc.name.is_empty() {
            if let serde_json::Value::String(ref args_str) = tc.arguments {
                let parsed: serde_json::Value = serde_json::from_str(args_str).unwrap_or(tc.arguments.clone());
                tool_calls.push(ToolCall { id: tc.id, name: tc.name, arguments: parsed });
            } else {
                tool_calls.push(tc);
            }
        }
    }

    // Filter: only keep tool calls with valid ids and names
    let valid_count = tool_calls.iter().filter(|tc| !tc.id.is_empty() && !tc.name.is_empty()).count();
    if valid_count < tool_calls.len() {
        tracing::warn!(total = tool_calls.len(), valid = valid_count, "some tool calls had empty ids, filtering");
        tool_calls.retain(|tc| !tc.id.is_empty() && !tc.name.is_empty());
    }

    Ok((text, tool_calls, tokens))
}

/// Run a single turn with streaming event callbacks.
/// Same as `run_turn()` but calls the handler for real-time UI updates.
#[allow(clippy::too_many_arguments)]
pub async fn run_turn_streaming(
    provider_type: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    conversation: &mut Conversation,
    user_input: &str,
    tools: &ToolRegistry,
    system_prompt: Option<&str>,
    max_turns: usize,
    handler: &mut dyn StreamHandler,
    permissions: Option<Arc<PermissionManager>>,
) -> AxgaResult<TurnResult> {
    // Push user message
    conversation.push(AgentMessage::User {
        content: user_input.to_string(),
    });

    let mut final_text = String::new();
    let mut tool_calls_made: Vec<String> = Vec::new();
    let mut total_tokens: u32 = 0;
    let mut dedup = DedupTracker::new();

    handler.on_thinking();

    for turn in 0..max_turns {
        debug!(turn, "agent loop iteration (streaming)");

        // Build tool definitions from registry
        let tool_defs: Vec<ToolDefinition> = tools
            .names()
            .filter_map(|name| tools.get(name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect();

        // Build request
        let messages: Vec<AgentMessage> = conversation.messages().cloned().collect();
        let request = RequestBuilder::new(model, &messages)
            .with_max_tokens(4096);

        let request = if let Some(sys) = system_prompt {
            request.with_system_prompt(sys)
        } else {
            request
        };

        let request = if !tool_defs.is_empty() {
            request.with_tools(&tool_defs)
        } else {
            request
        };

        let provider: Box<dyn Provider> = match provider_type {
            "openai" => {
                Box::new(axga_ai::providers::openai::OpenAiProvider::new(
                    api_key.map(|s| s.to_string()),
                    base_url.map(|s| s.to_string()),
                )?)
            }
            "deepseek" => {
                Box::new(axga_ai::providers::deepseek::DeepSeekProvider::new(
                    api_key.map(|s| s.to_string()),
                    base_url.map(|s| s.to_string()),
                )?)
            }
            "anthropic" => {
                Box::new(axga_ai::providers::anthropic::AnthropicProvider::new(
                    api_key.map(|s| s.to_string()),
                )?)
            }
            _ => return Err(AxgaError::Config(format!("unknown provider: {provider_type}"))),
        };

        let mut stream = provider.stream_chat(&request).await?;

        // Process stream events one by one, calling the handler for each
        let (text, tool_calls, token_count) =
            collect_stream_with_handler(&mut stream, handler).await?;
        total_tokens += token_count;

        if !text.is_empty() {
            final_text = text.clone();
        }

        // No tool calls → turn is done
        if tool_calls.is_empty() {
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent {
                    text: Some(text),
                    tool_calls: None,
                    thinking: None,
                },
            });
            break;
        }

        // Execute tool calls — filter empty IDs
        let valid_tool_calls: Vec<ToolCall> = tool_calls
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .collect();

        if valid_tool_calls.is_empty() {
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent { text: Some(text), tool_calls: None, thinking: None },
            });
            break;
        }

        // Pre-screen tool calls for permissions
        let (approved_tc, pending_tc) = if let Some(ref perms) = permissions {
            perms.check_batch(&valid_tool_calls)
        } else {
            (valid_tool_calls.clone(), vec![])
        };

        // Execute approved tool calls
        let results = if !approved_tc.is_empty() {
            execute_tool_calls(tools, &approved_tc, permissions.clone(), &mut dedup).await?
        } else {
            vec![]
        };

        let should_stop = results.iter().any(|r| r.force_stop);

        for tc in &approved_tc {
            tool_calls_made.push(tc.name.clone());
        }

        // Notify handler of tool results
        for result in &results {
            if result.tool_call_id.is_empty() { continue; }
            let truncated = truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN);
            let tc_name = approved_tc
                .iter()
                .find(|tc| tc.id == result.tool_call_id)
                .map(|tc| tc.name.as_str())
                .unwrap_or("unknown");
            handler.on_tool_call_result(tc_name, &truncated);
        }

        // Build combined tool calls for the assistant message (approved + pending)
        let mut all_tool_calls = approved_tc.clone();
        all_tool_calls.extend(pending_tc.clone());

        conversation.push(AgentMessage::Assistant {
            content: AssistantContent {
                text: Some(text),
                tool_calls: Some(all_tool_calls),
                thinking: None,
            },
        });

        for result in results {
            if result.tool_call_id.is_empty() { continue; }
            conversation.push(AgentMessage::Tool {
                tool_call_id: result.tool_call_id,
                content: truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN),
            });
        }

        if should_stop {
            warn!("dedup force-stop: breaking agent loop");
            break;
        }

        // Return pending approvals if any
        if !pending_tc.is_empty() {
            conversation.set_state(axga_shared::types::AgentState::WaitingForTools);
            handler.on_done();
            return Ok(TurnResult {
                final_text,
                tool_calls_made,
                total_tokens,
                pending_approvals: pending_tc,
            });
        }
    }

    conversation.set_state(axga_shared::types::AgentState::Finished);
    handler.on_done();

    Ok(TurnResult {
        final_text,
        tool_calls_made,
        total_tokens,
        pending_approvals: vec![],
    })
}

/// Continue an agent turn after pending tool approvals are resolved.
///
/// Executes the previously-pending tool calls (PermissionManager has been updated
/// with user's approval/denial decisions), pushes results, and continues the
/// agent loop if needed.
#[allow(clippy::too_many_arguments)]
pub async fn continue_turn_streaming(
    provider_type: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    conversation: &mut Conversation,
    tools: &ToolRegistry,
    system_prompt: Option<&str>,
    max_turns: usize,
    handler: &mut dyn StreamHandler,
    permissions: Option<Arc<PermissionManager>>,
    pending_calls: Vec<ToolCall>,
) -> AxgaResult<TurnResult> {
    let mut tool_calls_made: Vec<String> = Vec::new();
    let mut total_tokens: u32 = 0;

    // Execute the previously-pending tool calls (now resolved by user)
    if !pending_calls.is_empty() {
        let mut dedup = DedupTracker::new();
        let results = execute_tool_calls(tools, &pending_calls, permissions.clone(), &mut dedup).await?;

        for tc in &pending_calls {
            tool_calls_made.push(tc.name.clone());
        }

        // Notify handler of tool results
        for result in &results {
            if result.tool_call_id.is_empty() { continue; }
            let truncated = truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN);
            let tc_name = pending_calls
                .iter()
                .find(|tc| tc.id == result.tool_call_id)
                .map(|tc| tc.name.as_str())
                .unwrap_or("unknown");
            handler.on_tool_call_result(tc_name, &truncated);
        }

        // Push results to conversation
        for result in results {
            if result.tool_call_id.is_empty() { continue; }
            conversation.push(AgentMessage::Tool {
                tool_call_id: result.tool_call_id,
                content: truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN),
            });
        }
    }

    // Continue the agent loop — let LLM respond with tool results now available
    // This follows the same loop pattern as run_turn_streaming
    for _turn in 0..max_turns {
        let tool_defs: Vec<ToolDefinition> = tools
            .names()
            .filter_map(|name| tools.get(name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect();

        let messages: Vec<AgentMessage> = conversation.messages().cloned().collect();
        let request = RequestBuilder::new(model, &messages).with_max_tokens(4096);
        let request = if let Some(sys) = system_prompt {
            request.with_system_prompt(sys)
        } else {
            request
        };
        let request = if !tool_defs.is_empty() {
            request.with_tools(&tool_defs)
        } else {
            request
        };

        let provider: Box<dyn Provider> = match provider_type {
            "openai" => Box::new(axga_ai::providers::openai::OpenAiProvider::new(
                api_key.map(|s| s.to_string()),
                base_url.map(|s| s.to_string()),
            )?),
            "deepseek" => Box::new(axga_ai::providers::deepseek::DeepSeekProvider::new(
                api_key.map(|s| s.to_string()),
                base_url.map(|s| s.to_string()),
            )?),
            "anthropic" => Box::new(axga_ai::providers::anthropic::AnthropicProvider::new(
                api_key.map(|s| s.to_string()),
            )?),
            _ => return Err(AxgaError::Config(format!("unknown provider: {provider_type}"))),
        };

        let mut stream = provider.stream_chat(&request).await?;
        let (text, tool_calls, token_count) =
            collect_stream_with_handler(&mut stream, handler).await?;
        total_tokens += token_count;

        if tool_calls.is_empty() {
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent {
                    text: Some(text),
                    tool_calls: None,
                    thinking: None,
                },
            });
            break;
        }

        let valid_tool_calls: Vec<ToolCall> = tool_calls
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .collect();

        if valid_tool_calls.is_empty() {
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent {
                    text: Some(text),
                    tool_calls: None,
                    thinking: None,
                },
            });
            break;
        }

        // Pre-screen tool calls for permissions
        let (approved_tc, pending_tc) = if let Some(ref perms) = permissions {
            perms.check_batch(&valid_tool_calls)
        } else {
            (valid_tool_calls.clone(), vec![])
        };

        let results = if !approved_tc.is_empty() {
            let mut dedup2 = DedupTracker::new();
            execute_tool_calls(tools, &approved_tc, permissions.clone(), &mut dedup2).await?
        } else {
            vec![]
        };

        for tc in &approved_tc {
            tool_calls_made.push(tc.name.clone());
        }

        for result in &results {
            if result.tool_call_id.is_empty() { continue; }
            let truncated = truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN);
            let tc_name = approved_tc
                .iter()
                .find(|tc| tc.id == result.tool_call_id)
                .map(|tc| tc.name.as_str())
                .unwrap_or("unknown");
            handler.on_tool_call_result(tc_name, &truncated);
        }

        let mut all_tool_calls = approved_tc.clone();
        all_tool_calls.extend(pending_tc.clone());

        conversation.push(AgentMessage::Assistant {
            content: AssistantContent {
                text: Some(text.clone()),
                tool_calls: Some(all_tool_calls),
                thinking: None,
            },
        });

        for result in results {
            if result.tool_call_id.is_empty() { continue; }
            conversation.push(AgentMessage::Tool {
                tool_call_id: result.tool_call_id,
                content: truncate(&result.content, limits::MAX_TOOL_OUTPUT_LEN),
            });
        }

        if !pending_tc.is_empty() {
            conversation.set_state(axga_shared::types::AgentState::WaitingForTools);
            handler.on_done();
            return Ok(TurnResult {
                final_text: text,
                tool_calls_made,
                total_tokens,
                pending_approvals: pending_tc,
            });
        }
    }

    conversation.set_state(axga_shared::types::AgentState::Finished);
    handler.on_done();

    Ok(TurnResult {
        final_text: String::new(),
        tool_calls_made,
        total_tokens,
        pending_approvals: vec![],
    })
}

/// Drain a stream of StreamEvents into collected text, tool calls, and token count,
/// while notifying the handler of each event for real-time UI updates.
async fn collect_stream_with_handler<S>(
    stream: &mut S,
    handler: &mut dyn StreamHandler,
) -> AxgaResult<(String, Vec<ToolCall>, u32)>
where
    S: Stream<Item = AxgaResult<StreamEvent>> + Unpin,
{
    let mut text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut current_tc: Option<ToolCall> = None;
    let mut tokens: u32 = 0;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta { text: t } => {
                handler.on_text_delta(&t);
                text.push_str(&t);
            }
            StreamEvent::ToolCallDelta { id, name, args_fragment } => {
                handler.on_tool_call_delta(&id, &name, &args_fragment);
                if id.is_empty() {
                    if let Some(ref mut tc) = current_tc {
                        if let serde_json::Value::String(ref mut existing) = tc.arguments {
                            existing.push_str(&name);
                            existing.push_str(&args_fragment);
                        }
                    }
                } else if current_tc.as_ref().is_none_or(|tc| tc.id != id) {
                    if let Some(tc) = current_tc.take() {
                        tool_calls.push(tc);
                    }
                    current_tc = Some(ToolCall {
                        id,
                        name,
                        arguments: serde_json::Value::String(args_fragment),
                    });
                } else if let Some(ref mut tc) = current_tc {
                    if let serde_json::Value::String(ref mut existing) = tc.arguments {
                        existing.push_str(&args_fragment);
                    }
                }
            }
            StreamEvent::ThinkingDelta { .. } => {}
            StreamEvent::Usage { input_tokens, output_tokens } => {
                tokens += input_tokens + output_tokens;
            }
            StreamEvent::Done => break,
            StreamEvent::Stop { reason } => {
                debug!(%reason, "stream stopped");
                break;
            }
            StreamEvent::Error { message } => {
                return Err(AxgaError::LlmProvider(message));
            }
        }
    }

    // Finalize last tool call
    if let Some(tc) = current_tc.take() {
        if !tc.id.is_empty() && !tc.name.is_empty() {
            if let serde_json::Value::String(ref args_str) = tc.arguments {
                let parsed: serde_json::Value = serde_json::from_str(args_str).unwrap_or(tc.arguments.clone());
                tool_calls.push(ToolCall { id: tc.id, name: tc.name, arguments: parsed });
            } else {
                tool_calls.push(tc);
            }
        }
    }

    // Filter: only keep tool calls with valid ids and names
    let valid_count = tool_calls.iter().filter(|tc| !tc.id.is_empty() && !tc.name.is_empty()).count();
    if valid_count < tool_calls.len() {
        tracing::warn!(total = tool_calls.len(), valid = valid_count, "some tool calls had empty ids, filtering");
        tool_calls.retain(|tc| !tc.id.is_empty() && !tc.name.is_empty());
    }

    Ok((text, tool_calls, tokens))
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push_str(&format!("\n... [{} more bytes truncated]", s.len() - max_len));
        t
    }
}
