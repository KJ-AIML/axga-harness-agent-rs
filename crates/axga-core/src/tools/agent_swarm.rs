//! AgentSwarm tool — lets the LLM spawn multiple sub-agents in parallel.
//!
//! Each item in the `items` array becomes a separate sub-agent task, executed
//! concurrently via the Orchestrator. An optional `shared_prompt` is prepended
//! to every item's prompt before dispatch.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::{SubAgentConfig, SubAgentResult};
use crate::orchestrator::Orchestrator;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct AgentSwarmTool {
    orchestrator: Orchestrator,
    default_provider: String,
    default_model: String,
    api_key: Option<String>,
    base_url: Option<String>,
}

impl AgentSwarmTool {
    pub fn new(
        orchestrator: Orchestrator,
        default_provider: String,
        default_model: String,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        Self {
            orchestrator,
            default_provider,
            default_model,
            api_key,
            base_url,
        }
    }
}

impl Tool for AgentSwarmTool {
    fn name(&self) -> &str {
        "agent_swarm"
    }

    fn description(&self) -> &str {
        "Spawn multiple sub-agents in parallel, each with its own prompt. \
         Provide an `items` array of prompt strings. Optionally provide \
         `shared_prompt` (prepended to every item), `provider`, `model`, \
         and `max_turns` (default 3). Returns combined results from all sub-agents."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "description": "Array of prompt strings, one per sub-agent.",
                    "items": { "type": "string" }
                },
                "shared_prompt": {
                    "type": "string",
                    "description": "Optional prefix prepended to every item prompt."
                },
                "provider": {
                    "type": "string",
                    "description": "LLM provider for sub-agents (default: inherits from main agent)."
                },
                "model": {
                    "type": "string",
                    "description": "Model name for sub-agents (default: inherits from main agent)."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max turns per sub-agent (default 3)."
                }
            },
            "required": ["items"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let items: Vec<String> = input["items"]
                .as_array()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "agent_swarm".into(),
                    message: "missing 'items' array".into(),
                })?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| v.to_string())
                })
                .collect();

            if items.is_empty() {
                return Err(AxgaError::ToolError {
                    tool: "agent_swarm".into(),
                    message: "'items' array is empty".into(),
                });
            }

            let shared_prompt = input["shared_prompt"].as_str().map(|s| s.to_string());
            let provider = input["provider"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.default_provider.clone());
            let model = input["model"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.default_model.clone());
            let max_turns = input["max_turns"]
                .as_u64()
                .unwrap_or(3)
                .clamp(1, 20) as usize;

            // Build configs: one SubAgentConfig per item.
            let mut configs: Vec<(SubAgentConfig, String)> = Vec::with_capacity(items.len());
            for item in &items {
                let prompt = match &shared_prompt {
                    Some(sp) => format!("{sp}\n\n{item}"),
                    None => item.clone(),
                };

                let config = SubAgentConfig {
                    provider: provider.clone(),
                    model: model.clone(),
                    api_key: self.api_key.clone(),
                    base_url: self.base_url.clone(),
                    system_prompt: None,
                    max_turns,
                };

                configs.push((config, prompt));
            }

            // Spawn all sub-agents concurrently.
            let results: Vec<AxgaResult<SubAgentResult>> =
                self.orchestrator.spawn_all(configs).await;

            // Format combined output.
            let mut output = String::new();
            output.push_str(&format!(
                "# Agent Swarm Results\n\n{} sub-agents spawned | provider: {provider} | model: {model} | max_turns: {max_turns}\n\n",
                results.len()
            ));

            for (i, result) in results.iter().enumerate() {
                output.push_str(&format!("---\n\n## Agent {}\n\n", i + 1));
                output.push_str(&format!("**Prompt:** {}\n\n", items[i]));

                match result {
                    Ok(sub) => {
                        if let Some(ref err) = sub.error {
                            output.push_str(&format!("**Status:** ❌ Error\n**Error:** {err}\n\n"));
                        } else {
                            output.push_str(&format!(
                                "**Status:** ✅ Completed | tokens: {} | turns: {}\n\n",
                                sub.tokens_used, sub.turns_taken
                            ));
                            output.push_str(&sub.response);
                            output.push('\n');
                        }
                    }
                    Err(e) => {
                        output.push_str(&format!("**Status:** ❌ Orchestration Error\n**Error:** {e}\n\n"));
                    }
                }
            }

            Ok(output)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolRegistry;

    #[test]
    fn tool_name() {
        let registry = ToolRegistry::new();
        let orch = Orchestrator::new(registry);
        let tool = AgentSwarmTool::new(
            orch,
            "openai".into(),
            "gpt-4o-mini".into(),
            None,
            None,
        );
        assert_eq!(tool.name(), "agent_swarm");
    }

    #[test]
    fn parameters_requires_items() {
        let registry = ToolRegistry::new();
        let orch = Orchestrator::new(registry);
        let tool = AgentSwarmTool::new(
            orch,
            "openai".into(),
            "gpt-4o-mini".into(),
            None,
            None,
        );
        let params = tool.parameters();
        let required = params["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("items")));
    }

    #[test]
    fn description_mentions_agent_swarm() {
        let registry = ToolRegistry::new();
        let orch = Orchestrator::new(registry);
        let tool = AgentSwarmTool::new(
            orch,
            "openai".into(),
            "gpt-4o-mini".into(),
            None,
            None,
        );
        let desc = tool.description().to_lowercase();
        assert!(desc.contains("agent"));
        assert!(desc.contains("parallel"));
    }
}
