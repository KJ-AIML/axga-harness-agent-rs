//! Edit tool: perform exact string replacements in existing files.
//!
//! Safer than write_file — only changes what's specified, no full rewrite.
//! Finds old_string exactly once; errors on 0 or >1 matches.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::limits;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::path::PathBuf;

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a single exact string with a new string. \
         old_string must match exactly once; errors if 0 or >1 matches. \
         This is safer than write_file — no accidental full rewrites."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to file (absolute or relative)."
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to find and replace. Must match exactly once."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text."
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(
        &self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let path_str = input["path"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "edit".into(),
                message: "missing 'path'".into(),
            })?;
            let old_string =
                input["old_string"]
                    .as_str()
                    .ok_or_else(|| AxgaError::ToolError {
                        tool: "edit".into(),
                        message: "missing 'old_string'".into(),
                    })?;
            let new_string =
                input["new_string"]
                    .as_str()
                    .ok_or_else(|| AxgaError::ToolError {
                        tool: "edit".into(),
                        message: "missing 'new_string'".into(),
                    })?;

            let path = PathBuf::from(path_str);

            if !path.exists() {
                return Err(AxgaError::FileNotFound(path.display().to_string()));
            }

            let metadata = std::fs::metadata(&path)?;
            let size = metadata.len();
            if size > limits::MAX_FILE_READ_SIZE {
                return Err(AxgaError::FileTooLarge {
                    path: path.display().to_string(),
                    size,
                    limit: limits::MAX_FILE_READ_SIZE,
                });
            }

            let content = std::fs::read_to_string(&path)?;

            // Count exact matches
            let count = content.matches(old_string).count();

            if count == 0 {
                return Err(AxgaError::ToolError {
                    tool: "edit".into(),
                    message: format!(
                        "old_string not found in {}. No changes made.",
                        path.display()
                    ),
                });
            }

            if count > 1 {
                return Err(AxgaError::ToolError {
                    tool: "edit".into(),
                    message: format!(
                        "old_string matches {} times in {}. Must match exactly once. \
                         Provide more context to make old_string unique.",
                        count,
                        path.display()
                    ),
                });
            }

            // Exactly one match — replace it
            let new_content = content.replacen(old_string, new_string, 1);
            std::fs::write(&path, &new_content)?;

            Ok(format!(
                "Edited {} — replaced 1 occurrence of old_string.",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(
        path: &str,
        old: &str,
        new: &str,
    ) -> Value {
        serde_json::json!({
            "path": path,
            "old_string": old,
            "new_string": new,
        })
    }

    #[tokio::test]
    async fn edit_replaces_single_match() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let path_str = file_path.to_string_lossy().to_string();
        std::fs::write(&file_path, "hello world\nfoo bar\nbaz qux\n").unwrap();

        let tool = EditTool;
        let result = tool
            .execute(make_input(&path_str, "foo bar", "replaced"))
            .await
            .unwrap();

        assert!(result.contains("Edited"));
        let updated = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(updated, "hello world\nreplaced\nbaz qux\n");
    }

    #[tokio::test]
    async fn edit_errors_on_zero_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let path_str = file_path.to_string_lossy().to_string();
        std::fs::write(&file_path, "hello world\n").unwrap();

        let tool = EditTool;
        let result = tool.execute(make_input(&path_str, "nonexistent", "x")).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn edit_errors_on_multiple_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let path_str = file_path.to_string_lossy().to_string();
        std::fs::write(&file_path, "dup\ndup\ndup\n").unwrap();

        let tool = EditTool;
        let result = tool
            .execute(make_input(&path_str, "dup", "new"))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("matches 3 times"));
    }

    #[tokio::test]
    async fn edit_errors_on_missing_file() {
        let tool = EditTool;
        let result = tool
            .execute(make_input("/nonexistent/file.txt", "a", "b"))
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn edit_tool_name() {
        let tool = EditTool;
        assert_eq!(tool.name(), "edit");
    }

    #[test]
    fn edit_tool_description() {
        let tool = EditTool;
        assert!(tool.description().contains("safer"));
    }

    #[test]
    fn edit_tool_parameters_require_all() {
        let tool = EditTool;
        let params = tool.parameters();
        let req = params["required"].as_array().unwrap();
        assert!(req.contains(&serde_json::Value::String("path".into())));
        assert!(req.contains(&serde_json::Value::String("old_string".into())));
        assert!(req.contains(&serde_json::Value::String("new_string".into())));
    }
}
