//! Code analysis tools: grep, glob.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search file contents using a regular expression."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern." },
                "path": { "type": "string", "description": "File or directory to search." },
                "include": { "type": "string", "description": "File glob filter (e.g., '*.rs')." }
            },
            "required": ["pattern"]
        })
    }
    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let pattern = input["pattern"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "grep".into(),
                    message: "missing 'pattern'".into(),
                })?;
            let re = regex::Regex::new(pattern).map_err(|e| AxgaError::ToolError {
                tool: "grep".into(),
                message: e.to_string(),
            })?;
            let search_path = input["path"].as_str().unwrap_or(".");
            let include_filter = input["include"].as_str();

            let mut results = Vec::new();
            search_files(search_path, include_filter, &mut |file_path| {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    for (i, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            results.push(format!("{}:{}: {}", file_path, i + 1, line));
                            if results.len() >= 200 {
                                break;
                            } // limit results
                        }
                    }
                }
            });
            if results.is_empty() {
                Ok("No matches found.".into())
            } else {
                Ok(results.join("\n"))
            }
        })
    }
}

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Find files matching a glob pattern."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern (e.g., 'src/**/*.rs')." }
            },
            "required": ["pattern"]
        })
    }
    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let pattern = input["pattern"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "glob".into(),
                    message: "missing 'pattern'".into(),
                })?;
            let paths = glob::glob(pattern).map_err(|e| AxgaError::ToolError {
                tool: "glob".into(),
                message: e.to_string(),
            })?;
            let mut results: Vec<String> = paths
                .filter_map(|e| e.ok())
                .map(|p| p.display().to_string())
                .collect();
            results.truncate(200); // limit
            if results.is_empty() {
                Ok("No files matched.".into())
            } else {
                Ok(results.join("\n"))
            }
        })
    }
}

fn search_files(base: &str, include_filter: Option<&str>, cb: &mut dyn FnMut(String)) {
    let path = std::path::Path::new(base);
    if path.is_file() {
        if include_filter.is_none_or(|f| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| glob::Pattern::new(f).map(|p| p.matches(n)).unwrap_or(false))
        }) {
            cb(base.to_string());
        }
        return;
    }
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                // Skip hidden dirs and common noise
                if p.is_dir() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') || name == "target" || name == "node_modules" {
                            continue;
                        }
                    }
                    search_files(&p.display().to_string(), include_filter, cb);
                } else {
                    let display = p.display().to_string();
                    if include_filter.is_none_or(|f| {
                        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                            glob::Pattern::new(f).map(|p| p.matches(n)).unwrap_or(false)
                        })
                    }) {
                        cb(display);
                    }
                }
            }
        }
    }
}

pub struct DiffTool;

impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff"
    }
    fn description(&self) -> &str {
        "Show differences between two files."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path_a": { "type": "string", "description": "First file path." },
                "path_b": { "type": "string", "description": "Second file path." }
            },
            "required": ["path_a", "path_b"]
        })
    }
    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let path_a = input["path_a"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "diff".into(),
                    message: "missing 'path_a'".into(),
                })?;
            let path_b = input["path_b"]
                .as_str()
                .ok_or_else(|| AxgaError::ToolError {
                    tool: "diff".into(),
                    message: "missing 'path_b'".into(),
                })?;
            let a = std::fs::read_to_string(path_a)?;
            let b = std::fs::read_to_string(path_b)?;
            let diff = similar::TextDiff::from_lines(&a, &b);
            let result: String = diff
                .unified_diff()
                .header(path_a, path_b)
                .iter_hunks()
                .flat_map(|h| h.to_string().chars().collect::<Vec<_>>())
                .collect();
            Ok(if result.is_empty() {
                "Files are identical.".into()
            } else {
                result
            })
        })
    }
}
