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

        let stream = match provider_type {
            "openai" => {
                let provider = axga_ai::providers::openai::OpenAiProvider::new(
                    api_key.map(|s| s.to_string()),
                    base_url.map(|s| s.to_string()),
                )?;
                provider.stream_chat(&request).await?
            }
            "deepseek" => {
                let key = api_key
                    .map(|s| s.to_string())
                    .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok());
                let url = base_url
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
                let provider = axga_ai::providers::openai::OpenAiProvider::new(key, Some(url))?;
                provider.stream_chat(&request).await?
            }
            "anthropic" => {
                let provider = axga_ai::providers::anthropic::AnthropicProvider::new(
                    api_key.map(|s| s.to_string()),
                )?;
                provider.stream_chat(model, &messages, system_prompt, &tool_defs, 4096).await?
            }
            _ => return Err(AxgaError::Config(format!("unknown provider: {}", provider_type))),
        };

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

        // Execute tool calls
        let results = execute_tool_calls(tools, &tool_calls).await?;

        // Record tool calls
        for tc in &tool_calls {
            tool_calls_made.push(tc.name.clone());
        }

        // Push assistant message with tool calls
        conversation.push(AgentMessage::Assistant {
            content: AssistantContent {
                text: Some(text),
                tool_calls: Some(tool_calls.clone()),
                thinking: None,
            },
        });

        // Push tool results — filter out empty tool_call_ids
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
                } else if current_tc.as_ref().map_or(true, |tc| tc.id != id) {
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
        // Try to parse accumulated args as JSON
        if let serde_json::Value::String(ref args_str) = tc.arguments {
            let parsed: serde_json::Value = serde_json::from_str(args_str).unwrap_or(tc.arguments.clone());
            tool_calls.push(ToolCall {
                id: tc.id,
                name: tc.name,
                arguments: parsed,
            });
        } else {
            tool_calls.push(tc);
        }
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
