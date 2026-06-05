//! Native memctrl memory layer — SQLite-backed, no Python dependency.
//!
//! Replaces the `memctrl` CLI subprocess with direct rusqlite calls.
//! Eliminates the ~30-50 MB Python startup overhead per query.
//!
//! Schema:
//! ```sql
//! CREATE TABLE memories (
//!   id TEXT PRIMARY KEY,
//!   layer TEXT NOT NULL,        -- project, session, user
//!   content TEXT NOT NULL,
//!   source TEXT DEFAULT 'manual',
//!   confidence REAL DEFAULT 1.0,
//!   tags TEXT DEFAULT '',
//!   created_at TEXT NOT NULL,
//!   expires_at TEXT
//! );
//! CREATE INDEX idx_layer ON memories(layer);
//! CREATE INDEX idx_tags ON memories(tags);
//! ```

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;

pub struct MemCtrlNative {
    db: Mutex<Connection>,
}

impl MemCtrlNative {
    pub fn new() -> AxgaResult<Self> {
        let db_path = db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&db_path)
            .map_err(|e| AxgaError::Config(format!("memctrl: cannot open db: {}", e)))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                layer TEXT NOT NULL DEFAULT 'session',
                content TEXT NOT NULL,
                source TEXT DEFAULT 'manual',
                confidence REAL DEFAULT 1.0,
                tags TEXT DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_mem_layer ON memories(layer);
            CREATE INDEX IF NOT EXISTS idx_mem_tags ON memories(tags);"
        ).map_err(|e| AxgaError::Config(format!("memctrl: cannot init schema: {}", e)))?;

        Ok(Self { db: Mutex::new(conn) })
    }

    fn add(&self, content: &str, layer: &str, source: &str, confidence: f64) -> AxgaResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.db.lock().map_err(|e| AxgaError::Config(e.to_string()))?;
        conn.execute(
            "INSERT INTO memories (id, layer, content, source, confidence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, layer, content, source, confidence],
        ).map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?;
        Ok(format!("Added memory {} to {} layer", &id[..8], layer))
    }

    fn query(&self, query: &str) -> AxgaResult<String> {
        let conn = self.db.lock().map_err(|e| AxgaError::Config(e.to_string()))?;
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, layer, content, confidence, source, created_at FROM memories
             WHERE content LIKE ?1
             ORDER BY confidence DESC, created_at DESC
             LIMIT 10"
        ).map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?;

        let rows: Vec<String> = stmt.query_map(params![pattern], |row| {
            let id: String = row.get(0)?;
            let layer: String = row.get(1)?;
            let content: String = row.get(2)?;
            let confidence: f64 = row.get(3)?;
            let source: String = row.get(4)?;
            let created: String = row.get(5)?;
            Ok(format!(
                "[{}] {} (confidence: {:.1}, source: {}, layer: {}, {})",
                &id[..8], content, confidence, source, layer, created
            ))
        }).map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?
        .filter_map(|r| r.ok())
        .collect();

        if rows.is_empty() {
            Ok("No memories found.".into())
        } else {
            Ok(rows.join("\n"))
        }
    }

    fn list(&self, layer: Option<&str>) -> AxgaResult<String> {
        let conn = self.db.lock().map_err(|e| AxgaError::Config(e.to_string()))?;
        let sql = if let Some(l) = layer {
            format!("SELECT id, layer, content, confidence FROM memories WHERE layer = '{}' ORDER BY created_at DESC LIMIT 50", l)
        } else {
            "SELECT id, layer, content, confidence FROM memories ORDER BY created_at DESC LIMIT 50".into()
        };
        let mut stmt = conn.prepare(&sql).map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?;
        let rows: Vec<String> = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let layer: String = row.get(1)?;
            let content: String = row.get(2)?;
            let confidence: f64 = row.get(3)?;
            Ok(format!("[{}] [{}] {} (conf: {:.1})", &id[..8], layer, content, confidence))
        }).map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?
        .filter_map(|r| r.ok())
        .collect();

        if rows.is_empty() { Ok("No memories stored.".into()) } else { Ok(rows.join("\n")) }
    }

    fn forget(&self, id_prefix: &str) -> AxgaResult<String> {
        let conn = self.db.lock().map_err(|e| AxgaError::Config(e.to_string()))?;
        let pattern = format!("{}%", id_prefix);
        let deleted = conn.execute("DELETE FROM memories WHERE id LIKE ?1", params![pattern])
            .map_err(|e| AxgaError::ToolError { tool: "memctrl".into(), message: e.to_string() })?;
        Ok(format!("Forgot {} memories matching '{}'", deleted, id_prefix))
    }

    fn doctor(&self) -> AxgaResult<String> {
        let conn = self.db.lock().map_err(|e| AxgaError::Config(e.to_string()))?;
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap_or(0);
        let by_layer: Vec<String> = ["project", "session", "user"].iter().filter_map(|layer| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE layer = ?1", params![layer], |r| r.get(0)
            ).unwrap_or(0);
            if count > 0 { Some(format!("  {}: {} memories", layer, count)) } else { None }
        }).collect();

        Ok(format!("MemCtrl health:\n  DB: {}\n  Total: {} memories\n{}",
            db_path().display(), total, by_layer.join("\n")))
    }
}

fn db_path() -> PathBuf {
    std::env::var("MEMCTRL_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".memctrl/memories.db"))
}

// ── Tool trait impl ──

pub struct MemCtrlTool {
    native: MemCtrlNative,
}

impl MemCtrlTool {
    pub fn new() -> AxgaResult<Self> {
        Ok(Self { native: MemCtrlNative::new()? })
    }
}

impl Tool for MemCtrlTool {
    fn name(&self) -> &str { "memctrl" }
    fn description(&self) -> &str {
        "Persistent memory layer. Store facts, query with fuzzy matching, list by layer, forget by ID. \
         Layers: project (permanent), session (7 days), user (90 days). \
         Backed by SQLite — no Python dependency."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "query", "list", "forget", "doctor"],
                    "description": "Memory action."
                },
                "content": { "type": "string", "description": "Text to store or search query." },
                "layer": {
                    "type": "string", "enum": ["project", "session", "user"],
                    "description": "Memory layer. Default: session."
                },
                "id": { "type": "string", "description": "Memory ID prefix (for forget)." },
                "source": { "type": "string", "description": "Source of memory. Default: manual." },
                "confidence": { "type": "number", "description": "Confidence 0.0-1.0. Default: 1.0." }
            },
            "required": ["action"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        let action = input["action"].as_str().unwrap_or("list").to_string();
        let content = input["content"].as_str().unwrap_or("").to_string();
        let layer = input["layer"].as_str().unwrap_or("session").to_string();
        let source = input["source"].as_str().unwrap_or("manual").to_string();
        let confidence = input["confidence"].as_f64().unwrap_or(1.0);
        let id = input["id"].as_str().unwrap_or("").to_string();

        Box::pin(async move {
            match action.as_str() {
                "add" => {
                    if content.is_empty() { return Err(AxgaError::ToolError { tool: "memctrl".into(), message: "content required".into() }); }
                    self.native.add(&content, &layer, &source, confidence)
                }
                "query" => {
                    if content.is_empty() { return Err(AxgaError::ToolError { tool: "memctrl".into(), message: "content required".into() }); }
                    self.native.query(&content)
                }
                "list" => {
                    let l = if layer == "session" { None } else { Some(layer.as_str()) };
                    self.native.list(l)
                }
                "forget" => {
                    if id.is_empty() { return Err(AxgaError::ToolError { tool: "memctrl".into(), message: "id required".into() }); }
                    self.native.forget(&id)
                }
                "doctor" => self.native.doctor(),
                _ => Err(AxgaError::ToolError { tool: "memctrl".into(), message: format!("unknown action: {}", action) }),
            }
        })
    }
}
