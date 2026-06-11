//! Goal tools — create, get, update, and budget goals for autonomous mode.
//!
//! These tools interact with a shared `GoalManager` behind `Arc<Mutex<>>` so
//! the agent loop and TUI can both access the same goal state.

use super::Tool;
use axga_shared::error::AxgaResult;
use crate::goal::{Goal, GoalBudget, GoalManager, GoalStatus};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// ─── CreateGoalTool ──────────────────────────────────────────────────

pub struct CreateGoalTool {
    manager: Arc<Mutex<GoalManager>>,
}

impl CreateGoalTool {
    pub fn new(manager: Arc<Mutex<GoalManager>>) -> Self {
        Self { manager }
    }
}

impl Tool for CreateGoalTool {
    fn name(&self) -> &str {
        "create_goal"
    }

    fn description(&self) -> &str {
        "Create a new autonomous goal with objective, completion criterion, and budgets. \
         Use this to define what you want to accomplish. Provide a unique id (kebab-case), \
         an objective, a completion_criterion, and optional budgets (max_tokens, max_turns, \
         max_time_seconds). If no budgets are given the goal has unlimited resources."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Unique goal identifier (kebab-case, e.g. 'fix-login-bug')"
                },
                "objective": {
                    "type": "string",
                    "description": "What the goal aims to accomplish"
                },
                "completion_criterion": {
                    "type": "string",
                    "description": "How to determine the goal is complete"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Optional token budget for this goal"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Optional turn budget for this goal"
                },
                "max_time_seconds": {
                    "type": "integer",
                    "description": "Optional time budget in seconds"
                }
            },
            "required": ["id", "objective"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let id = input["id"].as_str().unwrap_or("").to_string();
        let objective = input["objective"].as_str().unwrap_or("").to_string();
        let criterion = input["completion_criterion"]
            .as_str()
            .unwrap_or("not specified")
            .to_string();
        let max_tokens = input["max_tokens"].as_u64().map(|v| v as u32);
        let max_turns = input["max_turns"].as_u64().map(|v| v as usize);
        let max_time = input["max_time_seconds"]
            .as_u64()
            .map(std::time::Duration::from_secs);

        let budget = GoalBudget::new(max_tokens, max_turns, max_time);

        let goal = Goal {
            id: id.clone(),
            objective: objective.clone(),
            completion_criterion: criterion,
            status: GoalStatus::Active,
            original_budget: budget.clone(),
            remaining_budget: budget,
            progress: String::new(),
            created_at: chrono::Utc::now(),
            started_at: Some(std::time::Instant::now()),
        };

        let manager = self.manager.clone();
        Box::pin(async move {
            let mut gm = manager.lock().unwrap();
            match gm.create(goal) {
                Ok(g) => Ok(format!(
                    "Goal '{}' created:\n  objective: {}\n  budget: {} tokens / {} turns / {}s",
                    g.id,
                    g.objective,
                    g.remaining_budget
                        .tokens
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    g.remaining_budget
                        .turns
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    g.remaining_budget
                        .time
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_else(|| "∞".into()),
                )),
                Err(e) => Ok(format!("Failed to create goal: {e}")),
            }
        })
    }
}

// ─── GetGoalTool ─────────────────────────────────────────────────────

pub struct GetGoalTool {
    manager: Arc<Mutex<GoalManager>>,
}

impl GetGoalTool {
    pub fn new(manager: Arc<Mutex<GoalManager>>) -> Self {
        Self { manager }
    }
}

impl Tool for GetGoalTool {
    fn name(&self) -> &str {
        "get_goal"
    }

    fn description(&self) -> &str {
        "Get information about a goal. Pass 'all' as the id to list all goals."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Goal id, or 'all' to list every goal"
                }
            },
            "required": ["id"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let id = input["id"].as_str().unwrap_or("all").to_string();
        let manager = self.manager.clone();

        Box::pin(async move {
            let gm = manager.lock().unwrap();
            if id == "all" {
                let goals: Vec<String> = gm
                    .iter()
                    .map(|g| {
                        format!(
                            "  [{}] {} — {} ({}% budget used)",
                            g.status,
                            g.id,
                            g.objective,
                            g.remaining_budget.usage_pct(&g.original_budget)
                        )
                    })
                    .collect();
                if goals.is_empty() {
                    Ok("No goals defined.".into())
                } else {
                    Ok(format!("Goals:\n{}", goals.join("\n")))
                }
            } else {
                match gm.get(&id) {
                    Some(g) => Ok(format!(
                        "Goal: {}\n  status: {}\n  objective: {}\n  completion: {}\n  progress: {}\n  budget: {} tokens / {} turns / {}s ({}% used)",
                        g.id,
                        g.status,
                        g.objective,
                        g.completion_criterion,
                        if g.progress.is_empty() { "(none)" } else { &g.progress },
                        g.remaining_budget.tokens.map(|t| t.to_string()).unwrap_or_else(|| "∞".into()),
                        g.remaining_budget.turns.map(|t| t.to_string()).unwrap_or_else(|| "∞".into()),
                        g.remaining_budget.time.map(|d| format!("{}", d.as_secs())).unwrap_or_else(|| "∞".into()),
                        g.remaining_budget.usage_pct(&g.original_budget),
                    )),
                    None => Ok(format!("Goal '{id}' not found")),
                }
            }
        })
    }
}

// ─── UpdateGoalTool ──────────────────────────────────────────────────

pub struct UpdateGoalTool {
    manager: Arc<Mutex<GoalManager>>,
}

impl UpdateGoalTool {
    pub fn new(manager: Arc<Mutex<GoalManager>>) -> Self {
        Self { manager }
    }
}

impl Tool for UpdateGoalTool {
    fn name(&self) -> &str {
        "update_goal"
    }

    fn description(&self) -> &str {
        "Update a goal's status or progress. \
         Valid statuses: active, paused, complete, blocked. \
         Use this to mark milestones, change direction, or report blocked status."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Goal id to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "complete", "blocked"],
                    "description": "New status for the goal"
                },
                "progress": {
                    "type": "string",
                    "description": "Free-form progress update"
                }
            },
            "required": ["id"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let id = input["id"].as_str().unwrap_or("").to_string();
        let status_str = input["status"].as_str();
        let progress = input["progress"].as_str().map(|s| s.to_string());

        let status = status_str.map(|s| match s {
            "active" => GoalStatus::Active,
            "paused" => GoalStatus::Paused,
            "complete" => GoalStatus::Complete,
            "blocked" => GoalStatus::Blocked,
            _ => GoalStatus::Active,
        });

        let manager = self.manager.clone();
        Box::pin(async move {
            let mut gm = manager.lock().unwrap();
            match gm.update(&id, status, progress) {
                Ok(g) => Ok(format!(
                    "Goal '{}' updated: status={}, progress={}",
                    g.id,
                    g.status,
                    if g.progress.is_empty() { "(unchanged)" } else { &g.progress }
                )),
                Err(e) => Ok(format!("Failed to update goal: {e}")),
            }
        })
    }
}

// ─── SetGoalBudgetTool ───────────────────────────────────────────────

pub struct SetGoalBudgetTool {
    manager: Arc<Mutex<GoalManager>>,
}

impl SetGoalBudgetTool {
    pub fn new(manager: Arc<Mutex<GoalManager>>) -> Self {
        Self { manager }
    }
}

impl Tool for SetGoalBudgetTool {
    fn name(&self) -> &str {
        "set_goal_budget"
    }

    fn description(&self) -> &str {
        "Set or adjust the budget for an existing goal. \
         Provide at least one of max_tokens, max_turns, max_time_seconds. \
         The budget resets the remaining counters and optionally updates the original budget."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Goal id to adjust budget for"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "New token budget (omit to keep current)"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "New turn budget (omit to keep current)"
                },
                "max_time_seconds": {
                    "type": "integer",
                    "description": "New time budget in seconds (omit to keep current)"
                },
                "set_original": {
                    "type": "boolean",
                    "description": "Also update the original budget for percentage calc (default false)"
                }
            },
            "required": ["id"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let id = input["id"].as_str().unwrap_or("").to_string();
        let set_original = input["set_original"].as_bool().unwrap_or(false);

        let manager = self.manager.clone();

        Box::pin(async move {
            let mut gm = manager.lock().unwrap();
            // Start from current remaining budget for fields not specified
            let current = match gm.get(&id) {
                Some(g) => g.remaining_budget.clone(),
                None => return Ok(format!("Goal '{id}' not found")),
            };

            let tokens = input["max_tokens"]
                .as_u64()
                .map(|v| v as u32)
                .or(current.tokens);
            let turns = input["max_turns"]
                .as_u64()
                .map(|v| v as usize)
                .or(current.turns);
            let time = input["max_time_seconds"]
                .as_u64()
                .map(std::time::Duration::from_secs)
                .or(current.time);

            let budget = GoalBudget::new(tokens, turns, time);

            match gm.set_budget(&id, budget, set_original) {
                Ok(g) => Ok(format!(
                    "Budget updated for goal '{}': {} tokens / {} turns / {}s (original {})",
                    g.id,
                    g.remaining_budget
                        .tokens
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    g.remaining_budget
                        .turns
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    g.remaining_budget
                        .time
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_else(|| "∞".into()),
                    if set_original { "updated" } else { "unchanged" },
                )),
                Err(e) => Ok(format!("Failed to update budget: {e}")),
            }
        })
    }
}
