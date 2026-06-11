//! Agent tool — lets the LLM spawn a sub-agent for a focused subtask.
//!
//! Each sub-agent runs with its own conversation context and turn budget.
//! Uses Orchestrator::spawn() under the hood.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::SubAgentConfig;
use crate::orchestrator::Orchestrator;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct AgentTool {
    /// Orchestrator used to spawn sub-agents.
    orchestrator: Orchestrator,
    /// Default provider type (e.g. "deepseek", "openai").
    default_provider: String,
    /// Default model name.
    default_model: String,
    /// Default API key (inherited from main agent).
    default_api_key: Option<String>,
    /// Default base URL override.
    default_base_url: Option<String>,
}

impl AgentTool {
    /// Create an AgentTool.
    ///
    /// - `orchestrator`: Orchestrator for spawning sub-agents.
    /// - `default_provider`, `default_model`: Used when the LLM doesn't override.
    /// - `default_api_key`, `default_base_url`: Inherited from the main agent session.
    pub fn new(
        orchestrator: Orchestrator,
        default_provider: String,
        default_model: String,
        default_api_key: Option<String>,
        default_base_url: Option<String>,
    ) -> Self {
        Self { orchestrator, default_provider, default_model, default_api_key, default_base_url }
    }
}

impl Tool for AgentTool {
    fn name(&self) -> &str {
        "agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a focused subtask. \
         The sub-agent runs with its own conversation context and turn budget, \
         and returns a result to the calling agent. Use this for delegating \
         work that doesn't need the full conversation history. \
         Sub-agents can read/write files, run shell commands, and use other tools."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task description for the sub-agent to work on."
                },
                "provider": {
                    "type": "string",
                    "description": "Optional LLM provider override (e.g. 'openai', 'deepseek', 'anthropic'). Inherits from the main agent if omitted."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override (e.g. 'gpt-4o-mini', 'deepseek-chat'). Inherits from the main agent if omitted."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum number of LLM turns for the sub-agent. Defaults to 5."
                }
            },
            "required": ["prompt"]
        })
    }

    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let provider = input["provider"]
            .as_str()
            .unwrap_or(&self.default_provider)
            .to_string();
        let model = input["model"]
            .as_str()
            .unwrap_or(&self.default_model)
            .to_string();
        let max_turns = input["max_turns"].as_u64().unwrap_or(5) as usize;
        let prompt = match input["prompt"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                return Box::pin(async {
                    Err(AxgaError::ToolError {
                        tool: "agent".into(),
                        message: "missing required 'prompt' parameter".into(),
                    })
                });
            }
        };

        let config = SubAgentConfig {
            provider,
            model,
            api_key: self.default_api_key.clone(),
            base_url: self.default_base_url.clone(),
            system_prompt: None,
            max_turns,
        };

        Box::pin(async move {
            let result = self.orchestrator.spawn(config, &prompt).await?;

            if let Some(err) = result.error {
                return Err(AxgaError::ToolError {
                    tool: "agent".into(),
                    message: format!("sub-agent error: {err}"),
                });
            }

            Ok(format!(
                "[sub-agent finished in {} turns, {} tokens]\n{}",
                result.turns_taken, result.tokens_used, result.response
            ))
        })
    }
}
