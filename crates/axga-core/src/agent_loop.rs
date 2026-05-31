//! Agent loop - the main runtime tying LLM, tools, and conversation state.
//!
//! # Flow
//! 1. User prompt -> push to Conversation
//! 2. Build OpenAI/Anthropic request from conversation history
//! 3. Stream LLM response -> collect text + tool calls
//! 4. If tool calls -> execute via ToolRegistry -> push results -> loop
//! 5. If no tool calls -> done, return final text
//!
//! # Memory
//! - Each loop iteration yields a Collector that drains the stream without buffering.
//! - Tool outputs truncated at MAX_TOOL_OUTPUT_LEN before pushing to conversation.

use crate::ToolRegistry;
use crate::executor::execute_tool_calls;
use crate::provider_registry::{ProviderKind, resolve_provider};
use crate::state::Conversation;
use axga_ai::request::RequestBuilder;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::limits;
use axga_shared::types::{AgentMessage, AssistantContent, StreamEvent, ToolCall, ToolDefinition};
use futures::{Stream, StreamExt};
use tracing::debug;

/// Result of one full turn (may include multiple LLM calls + tool executions).
pub struct TurnResult {
    pub final_text: String,
    pub tool_calls_made: Vec<String>,
    pub total_tokens: u32,
}

/// Run a single turn of the agent: user input -> LLM -> tools -> loop -> final response.
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

        let resolved_provider = resolve_provider(provider_type, api_key, base_url)?;
        let stream = match resolved_provider.spec.kind {
            ProviderKind::OpenAiCompatible => {
                let provider = axga_ai::providers::openai::OpenAiProvider::new(
                    resolved_provider.api_key,
                    resolved_provider.base_url,
                )?;
                provider.stream_chat(&request).await?
            }
            ProviderKind::Anthropic => {
                let provider = axga_ai::providers::anthropic::AnthropicProvider::new(
                    resolved_provider.api_key,
                )?;
                provider
                    .stream_chat(model, &messages, system_prompt, &tool_defs, 4096)
                    .await?
            }
        };

        // Collect stream events
        let (text, tool_calls, token_count) = collect_stream(stream).await?;
        total_tokens += token_count;

        if !text.is_empty() {
            final_text = text.clone();
        }

        // No tool calls: turn is done.
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

        // Execute tool calls after filtering empty IDs.
        let valid_tool_calls: Vec<ToolCall> = tool_calls
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .collect();

        if valid_tool_calls.is_empty() {
            // No valid tool calls: treat as text-only response.
            conversation.push(AgentMessage::Assistant {
                content: AssistantContent {
                    text: Some(text),
                    tool_calls: None,
                    thinking: None,
                },
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
            if result.tool_call_id.is_empty() {
                continue;
            }
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
    let mut tool_call_parts: std::collections::HashMap<usize, ToolCallParts> =
        std::collections::HashMap::new();
    let mut tool_call_order: Vec<usize> = Vec::new();
    let mut last_tool_call_key: Option<usize> = None;
    let mut next_fallback_key: usize = 0;
    let mut tokens: u32 = 0;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta { text: t } => {
                text.push_str(&t);
            }
            StreamEvent::ToolCallDelta {
                index,
                id,
                name,
                args_fragment,
            } => {
                let key = resolve_tool_call_key(
                    index,
                    &id,
                    &tool_call_parts,
                    last_tool_call_key,
                    &mut next_fallback_key,
                );
                let parts = tool_call_parts.entry(key).or_insert_with(|| {
                    tool_call_order.push(key);
                    ToolCallParts::default()
                });
                if !id.is_empty() {
                    parts.id = id;
                }
                if !name.is_empty() {
                    parts.name = name;
                }
                parts.args.push_str(&args_fragment);
                last_tool_call_key = Some(key);
            }
            StreamEvent::ThinkingDelta { .. } => {}
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
            } => {
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

    let mut tool_calls = Vec::with_capacity(tool_call_order.len());
    let mut invalid_count = 0usize;
    for key in tool_call_order {
        let Some(parts) = tool_call_parts.remove(&key) else {
            continue;
        };
        match parts.into_tool_call() {
            Some(tool_call) => tool_calls.push(tool_call),
            None => invalid_count += 1,
        }
    }

    if invalid_count > 0 {
        tracing::warn!(
            invalid = invalid_count,
            valid = tool_calls.len(),
            "some tool calls had empty ids, filtering"
        );
    }

    Ok((text, tool_calls, tokens))
}

#[derive(Default)]
struct ToolCallParts {
    id: String,
    name: String,
    args: String,
}

impl ToolCallParts {
    fn into_tool_call(self) -> Option<ToolCall> {
        if self.id.is_empty() || self.name.is_empty() {
            return None;
        }

        let arguments = if self.args.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&self.args).unwrap_or(serde_json::Value::String(self.args))
        };

        Some(ToolCall {
            id: self.id,
            name: self.name,
            arguments,
        })
    }
}

fn resolve_tool_call_key(
    index: Option<usize>,
    id: &str,
    calls: &std::collections::HashMap<usize, ToolCallParts>,
    last_key: Option<usize>,
    next_fallback_key: &mut usize,
) -> usize {
    if let Some(index) = index {
        return index;
    }

    if !id.is_empty() {
        if let Some((key, _)) = calls.iter().find(|(_, parts)| parts.id == id) {
            return *key;
        }
        let key = *next_fallback_key;
        *next_fallback_key += 1;
        return key;
    }

    if let Some(last_key) = last_key {
        return last_key;
    }

    let key = *next_fallback_key;
    *next_fallback_key += 1;
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn collect_stream_assembles_interleaved_openai_tool_calls() {
        let events: Vec<AxgaResult<StreamEvent>> = vec![
            Ok(StreamEvent::TextDelta {
                text: "working".into(),
            }),
            Ok(StreamEvent::ToolCallDelta {
                index: Some(0),
                id: "call_a".into(),
                name: "read_file".into(),
                args_fragment: "{\"path\":".into(),
            }),
            Ok(StreamEvent::ToolCallDelta {
                index: Some(1),
                id: "call_b".into(),
                name: "list_directory".into(),
                args_fragment: "{\"path\":\"src\"}".into(),
            }),
            Ok(StreamEvent::ToolCallDelta {
                index: Some(0),
                id: String::new(),
                name: String::new(),
                args_fragment: "\"README.md\"}".into(),
            }),
            Ok(StreamEvent::Done),
        ];

        let (text, calls, _) = collect_stream(stream::iter(events)).await.unwrap();

        assert_eq!(text, "working");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_a");
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "README.md");
        assert_eq!(calls[1].id, "call_b");
        assert_eq!(calls[1].name, "list_directory");
        assert_eq!(calls[1].arguments["path"], "src");
    }

    #[tokio::test]
    async fn collect_stream_assembles_anthropic_tool_use_delta() {
        let events: Vec<AxgaResult<StreamEvent>> = vec![
            Ok(StreamEvent::ToolCallDelta {
                index: Some(1),
                id: "toolu_1".into(),
                name: "read_file".into(),
                args_fragment: String::new(),
            }),
            Ok(StreamEvent::ToolCallDelta {
                index: Some(1),
                id: String::new(),
                name: String::new(),
                args_fragment: "{\"path\":\"Cargo.toml\"}".into(),
            }),
            Ok(StreamEvent::Done),
        ];

        let (_, calls, _) = collect_stream(stream::iter(events)).await.unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_1");
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "Cargo.toml");
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push_str(&format!(
            "\n... [{} more bytes truncated]",
            s.len() - max_len
        ));
        t
    }
}
