//! Unified request builder for LLM providers.
//!
//! Converts `AgentMessage[]` to provider-specific JSON bodies.
//! Phase 1: OpenAI format only.

use axga_shared::types::{AgentMessage, ToolDefinition};
use serde_json::Value;

#[derive(Clone)]
pub struct RequestBuilder {
    pub model: String,
    pub messages: Vec<Value>,
    pub original_messages: Vec<AgentMessage>,
    pub system_prompt: Option<String>,
    pub tools: Vec<Value>,
    pub original_tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stream: bool,
}

impl RequestBuilder {
    pub fn new(model: &str, messages: &[AgentMessage]) -> Self {
        Self {
            model: model.to_string(),
            messages: messages.iter().map(message_to_openai_json).collect(),
            original_messages: messages.to_vec(),
            system_prompt: None,
            tools: Vec::new(),
            original_tools: Vec::new(),
            max_tokens: 4096,
            temperature: None,
            stream: true,
        }
    }

    pub fn with_system_prompt(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn with_tools(mut self, tools: &[ToolDefinition]) -> Self {
        self.tools = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();
        self.original_tools = tools.to_vec();
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn build_openai_body(&self) -> Value {
        let mut messages = Vec::new();

        if let Some(ref sys) = self.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        messages.extend(self.messages.clone());

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": self.max_tokens,
            "stream": self.stream,
        });

        if !self.tools.is_empty() {
            body["tools"] = self.tools.clone().into();
        }

        if let Some(temp) = self.temperature {
            body["temperature"] = Value::from(temp as f64);
        }

        body
    }
}

fn message_to_openai_json(msg: &AgentMessage) -> Value {
    match msg {
        AgentMessage::User { content } => {
            serde_json::json!({
                "role": "user",
                "content": content
            })
        }
        AgentMessage::Assistant { content } => {
            let text_content = content.text.clone().unwrap_or_default();
            let mut msg = serde_json::json!({
                "role": "assistant",
                "content": text_content
            });

            if let Some(ref calls) = content.tool_calls {
                if !calls.is_empty() {
                    msg["tool_calls"] = calls.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments.to_string()
                            }
                        })
                    }).collect::<Vec<_>>().into();
                }
            }
            msg
        }
        AgentMessage::System { content } => {
            serde_json::json!({
                "role": "system",
                "content": content
            })
        }
        AgentMessage::Tool { tool_call_id, content } => {
            serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": content
            })
        }
    }
}
