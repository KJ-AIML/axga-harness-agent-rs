//! URL fetch tool — read web page content.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct FetchUrlTool;

impl Tool for FetchUrlTool {
    fn name(&self) -> &str { "fetch_url" }
    fn description(&self) -> &str {
        "Fetch and extract text content from a URL. Returns the page text (HTML stripped)."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch." },
                "max_chars": { "type": "integer", "description": "Max characters to return (default 5000)." }
            },
            "required": ["url"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let url = input["url"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "fetch_url".into(), message: "missing 'url'".into(),
            })?;
            let max_chars = input["max_chars"].as_u64().unwrap_or(5000) as usize;

            let client = reqwest::Client::builder()
                .user_agent("axga/1.0")
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(|e| AxgaError::Network(e.to_string()))?;

            let html = client.get(url).send().await
                .map_err(|e| AxgaError::Network(e.to_string()))?
                .text().await
                .map_err(|e| AxgaError::Network(e.to_string()))?;

            // Simple HTML-to-text: strip tags
            let mut text = String::new();
            let mut in_tag = false;
            let mut in_script = false;

            for c in html.chars() {
                if in_script {
                    if c == '>' { in_script = false; }
                    continue;
                }
                match c {
                    '<' => {
                        in_tag = true;
                    }
                    '>' => in_tag = false,
                    _ if !in_tag => {
                        if text.len() < max_chars {
                            text.push(c);
                        }
                    }
                    _ => {}
                }
            }

            // Clean up whitespace
            let cleaned: String = text
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");

            Ok(if cleaned.len() > max_chars {
                format!("{}...\n[truncated at {} chars]", &cleaned[..max_chars], max_chars)
            } else {
                cleaned
            })
        })
    }
}
