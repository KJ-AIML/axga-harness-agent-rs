//! Anthropic Messages API provider with streaming.

use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::{AgentMessage, StreamEvent, ToolDefinition};
use crate::request::RequestBuilder;
use crate::stream::SseStream;
use crate::providers::Provider;
use futures::Stream;
use reqwest::Client;
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>) -> AxgaResult<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| AxgaError::Config("ANTHROPIC_API_KEY not set".into()))?;
        let client = Client::builder()
            .pool_max_idle_per_host(2)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AxgaError::Network(e.to_string()))?;
        Ok(Self { client, api_key })
    }

    pub async fn stream_chat_inner(
        &self,
        model: &str,
        messages: &[AgentMessage],
        system_prompt: Option<&str>,
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>> {
        let anthropic_messages: Vec<serde_json::Value> = messages.iter().filter_map(|msg| match msg {
            AgentMessage::User { content } => Some(serde_json::json!({"role": "user", "content": content})),
            AgentMessage::Assistant { content } => {
                let mut parts: Vec<serde_json::Value> = Vec::new();
                if let Some(ref text) = content.text {
                    parts.push(serde_json::json!({"type": "text", "text": text}));
                }
                if let Some(ref calls) = content.tool_calls {
                    for tc in calls {
                        parts.push(serde_json::json!({"type": "tool_use", "id": tc.id, "name": tc.name, "input": tc.arguments}));
                    }
                }
                Some(serde_json::json!({"role": "assistant", "content": parts}))
            }
            AgentMessage::Tool { tool_call_id, content } => Some(serde_json::json!({
                "role": "user", "content": [{"type": "tool_result", "tool_use_id": tool_call_id, "content": content}]
            })),
            _ => None,
        }).collect();

        let mut body = serde_json::json!({
            "model": model, "messages": anthropic_messages,
            "max_tokens": max_tokens, "stream": true,
        });
        if let Some(sys) = system_prompt { body["system"] = serde_json::Value::String(sys.to_string()); }
        if !tools.is_empty() {
            body["tools"] = tools.iter().map(|t| serde_json::json!({
                "name": t.name, "description": t.description, "input_schema": t.parameters
            })).collect::<Vec<_>>().into();
        }

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await.map_err(|e| AxgaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 { return Err(AxgaError::RateLimited(text)); }
            return Err(AxgaError::Http { status: status.as_u16(), body: text });
        }

        Ok(Box::pin(SseStream {
            inner: response.bytes_stream(),
            buffer: String::with_capacity(4096),
            done: false,
        }))
    }
}

impl Provider for AnthropicProvider {
    fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>>> + Send>> {
        let this = self.clone();
        let model = request.model.clone();
        let messages = request.original_messages.clone();
        let system_prompt = request.system_prompt.clone();
        let tools = request.original_tools.clone();
        let max_tokens = request.max_tokens;
        Box::pin(async move {
            this.stream_chat_inner(&model, &messages, system_prompt.as_deref(), &tools, max_tokens).await
        })
    }
}
