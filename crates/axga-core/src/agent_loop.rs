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
use crate::executor::execute_tool_calls;
use crate::ToolRegistry;
use futures::{Stream, StreamExt};
use tracing::debug;

/// Result of one full turn (may include multiple LLM calls + tool executions).
pub struct TurnResult {
    pub final_text: String,
    pub tool_calls_made: Vec<String>,
    pub total_tokens: u32,
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
) -> AxgaResult<TurnResult> {
    // Push user message
    conversation.push(AgentMessage::User {
        content: user_input.to_string(),
    });

    let mut final_text = String::new();
    let mut tool_calls_made: Vec<String> = Vec::new();
    let mut total_tokens: u32 = 0;

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

        let results = execute_tool_calls(tools, &valid_tool_calls).await?;

        for tc in &valid_tool_calls {
            tool_calls_made.push(tc.name.clone());
        }

        conversation.push(AgentMessage::Assistant {
            content: AssistantContent {
                text: Some(text),
                tool_calls: Some(valid_tool_calls.clone()),
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
    }

    conversation.set_state(axga_shared::types::AgentState::Finished);

    Ok(TurnResult {
        final_text,
        tool_calls_made,
        total_tokens,
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

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push_str(&format!("\n... [{} more bytes truncated]", s.len() - max_len));
        t
    }
}
