//! File system tools: read_file, write_file, list_directory.
//!
//! # Memory Safety Rules
//! - `read_file`: checks `metadata().len()` first; >1MB -> stream or reject.
//! - `write_file`: no size limit (trust user), reports bytes written.
//! - All paths resolved relative to CWD.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::limits;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::path::PathBuf;

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str {
        "Read a file from the local filesystem. Rejects files larger than 1MB."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file (absolute or relative)." },
                "offset": { "type": "integer", "description": "Line to start from (1-based)." },
                "limit": { "type": "integer", "description": "Max lines to read." }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let path_str = input["path"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "read_file".into(), message: "missing 'path'".into(),
            })?;
            let path = PathBuf::from(path_str);
            if !path.exists() {
                return Err(AxgaError::FileNotFound(path.display().to_string()));
            }
            let metadata = std::fs::metadata(&path)?;
            let size = metadata.len();
            if size > limits::MAX_FILE_READ_SIZE {
                return Err(AxgaError::FileTooLarge {
                    path: path.display().to_string(), size, limit: limits::MAX_FILE_READ_SIZE,
                });
            }
            // Streaming read for large-but-ok files
            use std::io::Read;
            let mut file = std::fs::File::open(&path)?;
            let mut content = String::with_capacity(size as usize);
            file.read_to_string(&mut content)?;

            let offset = input["offset"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
            let limit = input["limit"].as_u64().map(|l| l as usize);
            let lines: Vec<&str> = content.lines().collect();
            if offset >= lines.len() { return Ok(String::new()); }
            let end = limit.map(|l| (offset + l).min(lines.len())).unwrap_or(lines.len());
            Ok(lines[offset..end].join("\n"))
        })
    }
}

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file, creating parent directories as needed." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file." },
                "content": { "type": "string", "description": "Content to write." }
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let path_str = input["path"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "write_file".into(), message: "missing 'path'".into(),
            })?;
            let content = input["content"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "write_file".into(), message: "missing 'content'".into(),
            })?;
            let path = PathBuf::from(path_str);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            Ok(format!("Wrote {} bytes to {}", content.len(), path.display()))
        })
    }
}

pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "List contents of a directory." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path. Default: current directory." }
            },
            "required": []
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let path_str = input["path"].as_str().unwrap_or(".");
            let entries: Vec<String> = std::fs::read_dir(path_str)?
                .filter_map(|e| e.ok())
                .map(|e| {
                    let ft = if e.file_type().map(|t| t.is_dir()).unwrap_or(false) { "/" } else { "" };
                    format!("{}{}", e.file_name().to_string_lossy(), ft)
                })
                .collect();
            Ok(entries.join("\n"))
        })
    }
}
