//! Goal/Autonomous Mode — tracked objectives with budget enforcement.
//!
//! Goals carry an objective, completion criterion, status, typed budgets
//! (tokens, turns, wall-clock), and a free-form progress description.
//! On each turn the agent loop feeds usage deltas into `GoalManager` so it can
//! warn when a budget runs low and auto-stop‑or‑block goals that exceed it.
//!
//! Thread‑safety: all mutation goes through `&mut self` on the owning struct.
//! For TUI read‑only access the caller should clone individual `Goal` records
//! or use `iter_goals()` / `active_count()`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ─── Budget ──────────────────────────────────────────────────────────

/// Per‑goal budget — the agent loop decrements these after every turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBudget {
    /// Remaining tokens allowed for this goal.  `None` = unlimited.
    pub tokens: Option<u32>,
    /// Remaining turns allowed.  `None` = unlimited.
    pub turns: Option<usize>,
    /// Remaining wall‑clock time.  `None` = unlimited.
    #[serde(skip)]
    pub time: Option<Duration>,
}

impl GoalBudget {
    pub fn new(tokens: Option<u32>, turns: Option<usize>, time: Option<Duration>) -> Self {
        Self { tokens, turns, time }
    }

    /// Convenience: no budget at all (unlimited).
    pub fn unlimited() -> Self {
        Self { tokens: None, turns: None, time: None }
    }

    /// Percentage used across all three dimensions (0‑100).  Returns the max usage.
    pub fn usage_pct(&self, original: &GoalBudget) -> u32 {
        let mut max_pct: f64 = 0.0;
        if let (Some(rem), Some(orig)) = (self.tokens, original.tokens) {
            if orig > 0 {
                let pct = 1.0 - rem as f64 / orig as f64;
                if pct > max_pct { max_pct = pct; }
            }
        }
        if let (Some(rem), Some(orig)) = (self.turns, original.turns) {
            if orig > 0 {
                let pct = 1.0 - rem as f64 / orig as f64;
                if pct > max_pct { max_pct = pct; }
            }
        }
        if let (Some(rem), Some(orig)) = (&self.time, &original.time) {
            if !orig.is_zero() {
                let pct = 1.0 - rem.as_secs_f64() / orig.as_secs_f64();
                if pct > max_pct { max_pct = pct; }
            }
        }
        ((max_pct * 100.0) as u32).min(100)
    }

    /// Whether **all** budget dimensions are zero or below.
    pub fn is_exhausted(&self) -> bool {
        self.tokens == Some(0) || self.turns == Some(0) || self.time.as_ref().is_some_and(|d| d.is_zero())
    }

    pub fn is_unlimited(&self) -> bool {
        self.tokens.is_none() && self.turns.is_none() && self.time.is_none()
    }

    /// Whether we are in the warning zone (≥80 % used in any dimension).
    pub fn is_low(&self, original: &GoalBudget) -> bool {
        self.usage_pct(original) >= 80
    }
}

// ─── Goal Status ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Complete,
    Blocked,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Complete => write!(f, "complete"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

// ─── Goal ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique identifier (short kebab‑case string).
    pub id: String,
    /// Human readable objective.
    pub objective: String,
    /// Criterion used to decide the goal is "complete".
    pub completion_criterion: String,
    /// Current status.
    pub status: GoalStatus,
    /// The original budget at creation time (used for percentage calculation).
    pub original_budget: GoalBudget,
    /// Remaining budget — decremented each turn.
    pub remaining_budget: GoalBudget,
    /// Free‑form progress description set by the agent.
    pub progress: String,
    /// When the goal was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the time budget started ticking.
    #[serde(skip)]
    pub started_at: Option<Instant>,
}

// ─── GoalManager ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct GoalManager {
    goals: HashMap<String, Goal>,
}

impl GoalManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ── CRUD ─────────────────────────────────────────────────────────

    pub fn create(&mut self, goal: Goal) -> Result<&Goal, String> {
        if self.goals.contains_key(&goal.id) {
            return Err(format!("Goal '{}' already exists", goal.id));
        }
        let id = goal.id.clone();
        self.goals.insert(id.clone(), goal);
        Ok(self.goals.get(&id).unwrap())
    }

    pub fn get(&self, id: &str) -> Option<&Goal> {
        self.goals.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Goal> {
        self.goals.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Goal> {
        self.goals.remove(id)
    }

    /// Update a goal's status and/or progress.  Returns `Err` if goal not found.
    pub fn update(
        &mut self,
        id: &str,
        status: Option<GoalStatus>,
        progress: Option<String>,
    ) -> Result<&Goal, String> {
        let g = self.goals.get_mut(id).ok_or_else(|| format!("Goal '{id}' not found"))?;
        if let Some(s) = status {
            g.status = s;
        }
        if let Some(p) = progress {
            g.progress = p;
        }
        Ok(g)
    }

    /// Replace the remaining budget of a goal.  Also updates the original budget
    /// if `set_original` is true.
    pub fn set_budget(
        &mut self,
        id: &str,
        budget: GoalBudget,
        set_original: bool,
    ) -> Result<&Goal, String> {
        let g = self.goals.get_mut(id).ok_or_else(|| format!("Goal '{id}' not found"))?;
        if set_original {
            g.original_budget = budget.clone();
        }
        g.remaining_budget = budget;
        Ok(g)
    }

    // ── Iteration ────────────────────────────────────────────────────

    pub fn iter(&self) -> impl Iterator<Item = &Goal> {
        self.goals.values()
    }

    pub fn active_goals(&self) -> impl Iterator<Item = &Goal> {
        self.goals.values().filter(|g| g.status == GoalStatus::Active)
    }

    pub fn active_count(&self) -> usize {
        self.goals.values().filter(|g| g.status == GoalStatus::Active).count()
    }

    pub fn len(&self) -> usize {
        self.goals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.goals.is_empty()
    }

    // ── Turn‑by‑turn budget tracking ─────────────────────────────────

    /// Called after every agent turn.  Decrements budgets for every `Active` goal.
    ///
    /// Returns a `Vec` of (goal_id, warning_message) for goals that exceed budget
    /// or enter the warning zone, so the caller can surface them to the user.
    pub fn on_turn(
        &mut self,
        tokens_used: u32,
        turns_used: usize,
        elapsed: Duration,
    ) -> Vec<GoalEvent> {
        let mut events = Vec::new();

        for goal in self.goals.values_mut() {
            if goal.status != GoalStatus::Active {
                continue;
            }

            let rb = &mut goal.remaining_budget;
            let orig = &goal.original_budget;

            // Decrement tokens
            if let Some(ref mut t) = rb.tokens {
                *t = t.saturating_sub(tokens_used);
            }
            // Decrement turns
            if let Some(ref mut n) = rb.turns {
                *n = n.saturating_sub(turns_used);
            }
            // Decrement time
            if let Some(ref mut d) = rb.time {
                *d = d.saturating_sub(elapsed);
            }

            // Check exhaustion → auto‑block or complete
            if rb.is_exhausted() && !orig.is_unlimited() {
                goal.status = GoalStatus::Blocked;
                events.push(GoalEvent::BudgetExceeded {
                    goal_id: goal.id.clone(),
                    objective: goal.objective.clone(),
                });
            } else if rb.is_low(orig) {
                events.push(GoalEvent::BudgetLow {
                    goal_id: goal.id.clone(),
                    objective: goal.objective.clone(),
                    usage_pct: rb.usage_pct(orig),
                });
            }
        }

        events
    }

    /// Mark a goal as complete — returns the updated goal.
    pub fn complete(&mut self, id: &str) -> Result<&Goal, String> {
        let g = self.goals.get_mut(id).ok_or_else(|| format!("Goal '{id}' not found"))?;
        g.status = GoalStatus::Complete;
        Ok(g)
    }
}

// ─── Events ──────────────────────────────────────────────────────────

/// Events emitted by `GoalManager::on_turn` for UI consumption.
#[derive(Debug, Clone)]
pub enum GoalEvent {
    /// A goal's remaining budget dropped below 80 %.
    BudgetLow {
        goal_id: String,
        objective: String,
        usage_pct: u32,
    },
    /// A goal's budget was exhausted — it has been auto‑blocked.
    BudgetExceeded {
        goal_id: String,
        objective: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_goal(id: &str, tokens: u32, turns: usize) -> Goal {
        Goal {
            id: id.into(),
            objective: format!("Goal {id}"),
            completion_criterion: String::new(),
            status: GoalStatus::Active,
            original_budget: GoalBudget::new(Some(tokens), Some(turns), None),
            remaining_budget: GoalBudget::new(Some(tokens), Some(turns), None),
            progress: String::new(),
            created_at: chrono::Utc::now(),
            started_at: None,
        }
    }

    #[test]
    fn create_and_get() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("test-1", 1000, 10)).unwrap();
        assert!(gm.get("test-1").is_some());
        assert_eq!(gm.len(), 1);
    }

    #[test]
    fn duplicate_rejected() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("dup", 100, 1)).unwrap();
        assert!(gm.create(make_goal("dup", 100, 1)).is_err());
    }

    #[test]
    fn budget_decrement_and_warning() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("g", 100, 2)).unwrap();

        // Turn 1: use 40 tokens, 1 turn → 60 remain, 1 turn remain.  Not low.
        let events = gm.on_turn(40, 1, Duration::ZERO);
        assert!(events.is_empty());
        let g = gm.get("g").unwrap();
        assert_eq!(g.remaining_budget.tokens, Some(60));
        assert_eq!(g.remaining_budget.turns, Some(1));

        // Turn 2: use 50 tokens, 1 turn → 10 remain, 0 turns → exhausted
        let events = gm.on_turn(50, 1, Duration::ZERO);
        assert!(!events.is_empty());
        let g = gm.get("g").unwrap();
        assert_eq!(g.status, GoalStatus::Blocked);
    }

    #[test]
    fn budget_low_warning() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("g", 100, 100)).unwrap();
        // Use 85 tokens — should trigger low warning (85 %)
        let events = gm.on_turn(85, 0, Duration::ZERO);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GoalEvent::BudgetLow { usage_pct, .. } => assert!(*usage_pct >= 80),
            _ => panic!("expected BudgetLow"),
        }
    }

    #[test]
    fn complete_goal() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("g", 100, 10)).unwrap();
        gm.complete("g").unwrap();
        assert_eq!(gm.get("g").unwrap().status, GoalStatus::Complete);
    }

    #[test]
    fn update_progress() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("g", 100, 10)).unwrap();
        gm.update("g", None, Some("step 1 done".into())).unwrap();
        assert_eq!(gm.get("g").unwrap().progress, "step 1 done");
    }

    #[test]
    fn active_count() {
        let mut gm = GoalManager::new();
        gm.create(make_goal("a", 100, 10)).unwrap();
        gm.create(make_goal("b", 200, 5)).unwrap();
        assert_eq!(gm.active_count(), 2);
        gm.complete("a").unwrap();
        assert_eq!(gm.active_count(), 1);
    }

    #[test]
    fn unlimited_budget_never_exhausts() {
        let mut gm = GoalManager::new();
        gm.create(Goal {
            id: "inf".into(),
            objective: "unlimited".into(),
            completion_criterion: String::new(),
            status: GoalStatus::Active,
            original_budget: GoalBudget::unlimited(),
            remaining_budget: GoalBudget::unlimited(),
            progress: String::new(),
            created_at: chrono::Utc::now(),
            started_at: None,
        })
        .unwrap();
        let events = gm.on_turn(999_999, 999, Duration::from_secs(9999));
        assert!(events.is_empty());
        assert_eq!(gm.get("inf").unwrap().status, GoalStatus::Active);
    }
}
