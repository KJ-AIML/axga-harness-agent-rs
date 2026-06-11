//! Cron scheduler tools — CronCreate, CronList, CronDelete.
//!
//! Supports a simple 5-field cron expression parser (*, */N, literal values)
//! with persistence to `~/.config/axga/cron.json`. A background tokio task
//! fires scheduled prompts, which are injected as system messages before
//! the next agent turn.
//!
//! # Expression Format
//! ```text
//! * * * * *
//! │ │ │ │ └── day of week   (0-6, Sun=0)
//! │ │ │ └──── month         (1-12)
//! │ │ └────── day of month  (1-31)
//! │ └──────── hour          (0-23)
//! └────────── minute        (0-59)
//! ```

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

// ─── Cron Expression Parser ───────────────────────────────────────────

/// A single field in a cron expression (minute, hour, dom, month, dow).
#[derive(Debug, Clone, PartialEq)]
enum CronField {
    /// `*` — matches every value.
    Any,
    /// `*/N` — matches every Nth value starting from the minimum.
    Step { interval: u8 },
    /// A literal value like `5` or `1,2,3`.
    Literal(Vec<u8>),
}

/// Parsed 5-field cron expression.
#[derive(Debug, Clone)]
struct CronExpr {
    minute: CronField,
    hour: CronField,
    dom: CronField,
    month: CronField,
    dow: CronField,
}

impl CronExpr {
    /// Parse a space-separated 5-field cron expression.
    fn parse(expr: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "expected 5 fields, got {} — use '* * * * *' format",
                fields.len()
            ));
        }

        Ok(Self {
            minute: parse_field(fields[0], 0, 59)?,
            hour: parse_field(fields[1], 0, 23)?,
            dom: parse_field(fields[2], 1, 31)?,
            month: parse_field(fields[3], 1, 12)?,
            dow: parse_field(fields[4], 0, 6)?,
        })
    }

    /// Returns `true` if the given time components match this expression.
    fn matches(&self, minute: u8, hour: u8, dom: u8, month: u8, dow: u8) -> bool {
        field_matches(&self.minute, minute)
            && field_matches(&self.hour, hour)
            && field_matches(&self.dom, dom)
            && field_matches(&self.month, month)
            && field_matches(&self.dow, dow)
    }
}

fn parse_field(s: &str, min: u8, max: u8) -> Result<CronField, String> {
    if s == "*" {
        return Ok(CronField::Any);
    }
    if let Some(rest) = s.strip_prefix("*/") {
        let interval: u8 = rest
            .parse()
            .map_err(|_| format!("invalid */N step: {s}"))?;
        if interval == 0 || interval > max {
            return Err(format!("*/N step out of range [{min}-{max}]: {s}"));
        }
        return Ok(CronField::Step { interval });
    }
    if s.contains(',') {
        let mut values = Vec::new();
        for part in s.split(',') {
            let v: u8 = part
                .parse()
                .map_err(|_| format!("invalid literal: {s}"))?;
            if v < min || v > max {
                return Err(format!("value {v} out of range [{min}-{max}] in: {s}"));
            }
            values.push(v);
        }
        return Ok(CronField::Literal(values));
    }
    let v: u8 = s.parse().map_err(|_| format!("invalid field: {s}"))?;
    if v < min || v > max {
        return Err(format!("value {v} out of range [{min}-{max}] in: {s}"));
    }
    Ok(CronField::Literal(vec![v]))
}

fn field_matches(field: &CronField, value: u8) -> bool {
    match field {
        CronField::Any => true,
        CronField::Step { interval } => value % interval == 0,
        CronField::Literal(vals) => vals.contains(&value),
    }
}

// ─── Cron Job Data ────────────────────────────────────────────────────

/// A single cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub expr: String,
    pub prompt: String,
    pub created_at: String,
}

/// Persistence store of all cron jobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CronStore {
    jobs: Vec<CronJob>,
}

impl CronStore {
    fn path() -> std::path::PathBuf {
        let base = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
        let dir = base.join(".config").join("axga");
        std::fs::create_dir_all(&dir).ok();
        dir.join("cron.json")
    }

    fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> AxgaResult<()> {
        let path = Self::path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AxgaError::Serialization(e.to_string()))?;
        std::fs::write(&path, json).map_err(AxgaError::Io)?;
        Ok(())
    }
}

fn dirs_next() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .or_else(|_| {
            std::env::var("USERPROFILE").or_else(|_| {
                let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
                let path = std::env::var("HOMEPATH").unwrap_or_default();
                if drive.is_empty() && path.is_empty() {
                    Err(std::env::VarError::NotPresent)
                } else {
                    Ok(format!("{drive}{path}"))
                }
            })
        })
        .ok()
        .map(std::path::PathBuf::from)
}

// ─── Cron Scheduler ───────────────────────────────────────────────────

/// Fired cron event sent from the scheduler to the agent loop.
#[derive(Debug, Clone)]
pub struct CronEvent {
    pub job_id: String,
    pub prompt: String,
}

/// Background cron scheduler that fires prompts at scheduled times.
pub struct CronScheduler {
    /// Receiver for fired cron events. The agent loop polls this.
    pub rx: mpsc::Receiver<CronEvent>,
    /// Handle to the background task.
    handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender.
    shutdown_tx: mpsc::Sender<()>,
}

impl CronScheduler {
    /// Start the cron scheduler. Checks every 30 seconds.
    pub fn start() -> Self {
        let (event_tx, event_rx) = mpsc::channel::<CronEvent>(16);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let handle = tokio::spawn(async move {
            let mut last_minute: Option<(u8, u8, u8, u8, u8)> = None;
            loop {
                // Check for shutdown signal first
                if shutdown_rx.try_recv().is_ok() {
                    tracing::info!("cron scheduler shutting down");
                    break;
                }

                // Sleep 30 seconds, checking for shutdown periodically
                // We use a shorter sleep to be responsive to shutdown
                let slept = tokio::time::sleep(std::time::Duration::from_secs(30));
                tokio::pin!(slept);
                tokio::select! {
                    _ = &mut slept => {}
                    _ = shutdown_rx.recv() => {
                        tracing::info!("cron scheduler shutting down");
                        break;
                    }
                }

                let now = chrono_now();
                let current_minute = (
                    now.minute as u8,
                    now.hour as u8,
                    now.day as u8,
                    now.month as u8,
                    now.weekday as u8,
                );

                // Skip if we already checked this minute
                if last_minute == Some(current_minute) {
                    continue;
                }
                last_minute = Some(current_minute);

                let store = CronStore::load();
                for job in &store.jobs {
                    match CronExpr::parse(&job.expr) {
                        Ok(expr) => {
                            if expr.matches(
                                current_minute.0,
                                current_minute.1,
                                current_minute.2,
                                current_minute.3,
                                current_minute.4,
                            ) {
                                tracing::info!(job_id = %job.id, expr = %job.expr, "cron job firing");
                                let send_result = event_tx
                                    .send(CronEvent {
                                        job_id: job.id.clone(),
                                        prompt: job.prompt.clone(),
                                    })
                                    .await;
                                if send_result.is_err() {
                                    tracing::warn!("cron event channel closed");
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(job_id = %job.id, expr = %job.expr, "invalid cron expression: {e}");
                        }
                    }
                }
            }
        });

        Self {
            rx: event_rx,
            handle: Some(handle),
            shutdown_tx,
        }
    }

    /// Poll for any pending cron events without blocking.
    pub fn try_recv(&mut self) -> Option<CronEvent> {
        self.rx.try_recv().ok()
    }

    /// Shut down the background scheduler.
    pub async fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(()).await;
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// A lightweight clone of chrono-like fields for the scheduler.
/// We avoid adding a chrono dependency; the stdlib suffices for minute-granularity.
struct Now {
    minute: u32,
    hour: u32,
    day: u32,
    month: u32,
    weekday: u32,
}

/// Get current time components using only stdlib (via epoch math).
fn chrono_now() -> Now {
    use std::time::SystemTime;

    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Use civil time calculation: days since 1970-01-01 + seconds within day
    let days = (secs / 86400) as i64;
    let secs_of_day = (secs % 86400) as u32;

    // Convert days to year/month/day using a simple algorithm
    let (_year, month, day) = civil_from_days(days);

    let minute = (secs_of_day / 60) % 60;
    let hour = (secs_of_day / 3600) % 24;

    // Zeller-like for day of week (1970-01-01 was Thursday = 4)
    let weekday = ((days + 4) % 7) as u32; // 0=Sun

    Now {
        minute,
        hour,
        day,
        month,
        weekday,
    }
}

/// Convert days since Unix epoch to (year, month, day).
/// Uses a simple algorithm that works for years 1970–2099.
fn civil_from_days(mut days: i64) -> (i64, u32, u32) {
    days += 719468; // shift to 0000-03-01 (simplifies leap year math)
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let doe = days - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month phase [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day of month [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

// ─── CronCreate Tool ──────────────────────────────────────────────────

pub struct CronCreateTool;

impl Tool for CronCreateTool {
    fn name(&self) -> &str {
        "cron_create"
    }
    fn description(&self) -> &str {
        "Create a cron job that fires a prompt on a schedule. \
         Uses 5-field cron format: '* * * * *' (min hour dom month dow). \
         Supports * (any), */N (every N), and literal values (0-59, etc.). \
         Example: '0 9 * * 1,2,3,4,5' fires at 9:00 AM weekdays; '*/5 * * * *' every 5 minutes."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "5-field cron expression: 'minute hour dom month dow'. Supports *, */N, literals, and comma-separated lists."
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt text to inject as a system message when the cron fires."
                }
            },
            "required": ["expression", "prompt"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let expr = input["expression"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "cron_create".into(),
                    message: "missing 'expression'".into(),
                })?;
            let prompt = input["prompt"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "cron_create".into(),
                    message: "missing 'prompt'".into(),
                })?;

            // Validate expression
            CronExpr::parse(expr).map_err(|e| AxgaError::ToolError {
                tool: "cron_create".into(),
                message: format!("invalid cron expression: {e}"),
            })?;

            let id = uuid::Uuid::new_v4().to_string();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let created_at = format_iso8601(now);

            let job = CronJob {
                id: id.clone(),
                expr: expr.to_string(),
                prompt: prompt.to_string(),
                created_at,
            };

            let mut store = CronStore::load();
            store.jobs.push(job);
            store.save()?;

            Ok(format!(
                "Cron job created.\n  ID: {id}\n  Expression: {expr}\n  Prompt: {prompt}\n  File: {}",
                CronStore::path().display()
            ))
        })
    }
}

// ─── CronList Tool ────────────────────────────────────────────────────

pub struct CronListTool;

impl Tool for CronListTool {
    fn name(&self) -> &str {
        "cron_list"
    }
    fn description(&self) -> &str {
        "List all active cron jobs with their IDs, expressions, and prompts."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
    fn execute(&self, _input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let store = CronStore::load();
            if store.jobs.is_empty() {
                return Ok("No cron jobs. Use cron_create to add one.".into());
            }

            let mut lines = vec![format!("{} cron job(s):", store.jobs.len())];
            for job in &store.jobs {
                lines.push(format!(
                    "  {}  |  {}  |  \"{}\"  |  created {}",
                    job.id, job.expr, job.prompt, job.created_at
                ));
            }
            Ok(lines.join("\n"))
        })
    }
}

// ─── CronDelete Tool ──────────────────────────────────────────────────

pub struct CronDeleteTool;

impl Tool for CronDeleteTool {
    fn name(&self) -> &str {
        "cron_delete"
    }
    fn description(&self) -> &str {
        "Delete a cron job by its ID. Use cron_list to see active jobs."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The ID of the cron job to delete."
                }
            },
            "required": ["id"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "cron_delete".into(),
                    message: "missing 'id'".into(),
                })?;

            let mut store = CronStore::load();
            let len_before = store.jobs.len();
            store.jobs.retain(|j| j.id != id);

            if store.jobs.len() == len_before {
                return Err(AxgaError::ToolError {
                    tool: "cron_delete".into(),
                    message: format!("no cron job found with id '{id}'"),
                });
            }

            store.save()?;
            Ok(format!("Cron job {id} deleted. {} job(s) remaining.", store.jobs.len()))
        })
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────

/// Format a Unix timestamp as a simple ISO 8601 string (UTC).
fn format_iso8601(unix_secs: u64) -> String {
    let secs = unix_secs;
    let days = (secs / 86400) as i64;
    let secs_of_day = (secs % 86400) as u32;

    let (y, m, d) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;

    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parser tests ──

    #[test]
    fn parse_any_field() {
        assert!(matches!(parse_field("*", 0, 59), Ok(CronField::Any)));
    }

    #[test]
    fn parse_step_field() {
        assert_eq!(parse_field("*/5", 0, 59).unwrap(), CronField::Step { interval: 5 });
        assert_eq!(parse_field("*/15", 0, 59).unwrap(), CronField::Step { interval: 15 });
    }

    #[test]
    fn parse_literal_field() {
        assert_eq!(parse_field("5", 0, 59).unwrap(), CronField::Literal(vec![5]));
        assert_eq!(
            parse_field("1,15,30", 0, 59).unwrap(),
            CronField::Literal(vec![1, 15, 30])
        );
    }

    #[test]
    fn parse_out_of_range() {
        assert!(parse_field("60", 0, 59).is_err());
        assert!(parse_field("*/100", 0, 59).is_err());
    }

    #[test]
    fn parse_empty() {
        assert!(parse_field("", 0, 59).is_err());
    }

    #[test]
    fn field_matches_any() {
        assert!(field_matches(&CronField::Any, 0));
        assert!(field_matches(&CronField::Any, 59));
    }

    #[test]
    fn field_matches_step() {
        let f = CronField::Step { interval: 5 };
        assert!(field_matches(&f, 0));
        assert!(field_matches(&f, 5));
        assert!(field_matches(&f, 10));
        assert!(!field_matches(&f, 7));
    }

    #[test]
    fn field_matches_literal() {
        let f = CronField::Literal(vec![1, 5, 10]);
        assert!(field_matches(&f, 1));
        assert!(field_matches(&f, 5));
        assert!(!field_matches(&f, 3));
    }

    #[test]
    fn parse_full_expression() {
        let expr = CronExpr::parse("*/5 9,17 * * 1,5").unwrap();
        assert_eq!(expr.minute, CronField::Step { interval: 5 });
        assert!(matches!(expr.hour, CronField::Literal(_)));
        assert_eq!(expr.dom, CronField::Any);
        assert_eq!(expr.month, CronField::Any);
        assert!(matches!(expr.dow, CronField::Literal(_)));
    }

    #[test]
    fn parse_every_minute() {
        let expr = CronExpr::parse("* * * * *").unwrap();
        assert!(expr.matches(0, 0, 1, 1, 0));
        assert!(expr.matches(59, 23, 31, 12, 6));
    }

    #[test]
    fn parse_specific_time() {
        let expr = CronExpr::parse("30 9 * * 1,2,3,4,5").unwrap();
        // Monday 9:30 → matches
        assert!(expr.matches(30, 9, 15, 3, 1));
        // Monday 9:31 → no match (wrong minute)
        assert!(!expr.matches(31, 9, 15, 3, 1));
        // Saturday 9:30 → no match (not weekday)
        assert!(!expr.matches(30, 9, 15, 3, 6));
    }

    #[test]
    fn parse_every_5_minutes() {
        let expr = CronExpr::parse("*/5 * * * *").unwrap();
        assert!(expr.matches(0, 0, 1, 1, 0));
        assert!(expr.matches(5, 0, 1, 1, 0));
        assert!(expr.matches(55, 0, 1, 1, 0));
        assert!(!expr.matches(3, 0, 1, 1, 0));
    }

    #[test]
    fn invalid_field_count() {
        assert!(CronExpr::parse("* * *").is_err());
        assert!(CronExpr::parse("* * * * * *").is_err());
    }

    // ── Civil date tests ──

    #[test]
    fn civil_epoch() {
        // 1970-01-01
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn civil_known_date() {
        // 2024-01-01 = day 19723
        let days = 19723;
        let (y, m, d) = civil_from_days(days);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn chrono_now_works() {
        let now = chrono_now();
        assert!(now.month >= 1 && now.month <= 12);
        assert!(now.day >= 1 && now.day <= 31);
        assert!(now.hour <= 23);
        assert!(now.minute <= 59);
        assert!(now.weekday <= 6);
    }

    // ── Tool name tests ──

    #[test]
    fn cron_create_name() {
        assert_eq!(CronCreateTool.name(), "cron_create");
    }

    #[test]
    fn cron_list_name() {
        assert_eq!(CronListTool.name(), "cron_list");
    }

    #[test]
    fn cron_delete_name() {
        assert_eq!(CronDeleteTool.name(), "cron_delete");
    }
}
