//! Multi-agent orchestrator — spawns sub-agents with configurable
//! (provider, model) and resource budgets, and collects results.

use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::{SubAgentConfig, SubAgentResult};
use crate::agent_loop::run_turn;
use crate::tools::registry::ToolRegistry;
use crate::Conversation;

/// Orchestrator that spawns sub-agents with their own config and budgets.
pub struct Orchestrator {
    /// Shared tool registry for sub-agents.
    registry: ToolRegistry,
}

impl Orchestrator {
    /// Create a new orchestrator with a tool registry.
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    /// Spawn a sub-agent with the given config and input, and wait for it to complete.
    ///
    /// Returns the sub-agent's final response or an error.
    pub async fn spawn(&self, config: SubAgentConfig, input: &str) -> AxgaResult<SubAgentResult> {
        let api_key = config.api_key.as_deref();
        let base_url = config.base_url.as_deref();

        // Build a fresh conversation for the sub-agent
        let mut conversation = Conversation::new();

        let result = run_turn(
            &config.provider,
            api_key,
            base_url,
            &config.model,
            &mut conversation,
            input,
            &self.registry,
            config.system_prompt.as_deref(),
            config.max_turns,
            None,
        )
        .await;

        match result {
            Ok(turn) => Ok(SubAgentResult {
                response: turn.final_text,
                tokens_used: turn.total_tokens,
                turns_taken: turn.tool_calls_made.len(),
                error: None,
            }),
            Err(e) => Ok(SubAgentResult {
                response: String::new(),
                tokens_used: 0,
                turns_taken: 0,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Spawn multiple sub-agents concurrently and collect all results.
    ///
    /// Each sub-agent runs independently with its own config.
    pub async fn spawn_all(
        &self,
        configs: Vec<(SubAgentConfig, String)>,
    ) -> Vec<AxgaResult<SubAgentResult>> {
        let mut handles = Vec::with_capacity(configs.len());
        for (config, input) in configs {
            let registry = self.registry.clone();
            handles.push(tokio::spawn(async move {
                let orch = Orchestrator::new(registry);
                orch.spawn(config, &input).await
            }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(Ok(result)),
                Ok(Err(e)) => results.push(Err(e)),
                Err(e) => results.push(Err(AxgaError::ToolError {
                    tool: "orchestrator".into(),
                    message: e.to_string(),
                })),
            }
        }
        results
    }
}
