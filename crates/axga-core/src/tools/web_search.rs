//! Web search tool using DuckDuckGo HTML (no API key needed).
//!
//! Scrapes DuckDuckGo HTML search results.

use super::Tool;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str {
        "Search the web using DuckDuckGo. Returns top results with titles, URLs, and snippets."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query." },
                "max_results": { "type": "integer", "description": "Max results (default 5)." }
            },
            "required": ["query"]
        })
    }
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>> {
        Box::pin(async move {
            let query = input["query"].as_str().ok_or_else(|| AxgaError::ToolError {
                tool: "web_search".into(), message: "missing 'query'".into(),
            })?;
            let max = input["max_results"].as_u64().unwrap_or(5).min(10);

            let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding(query));
            let client = reqwest::Client::builder()
                .user_agent("axga/1.0")
                .build()
                .map_err(|e| AxgaError::Network(e.to_string()))?;

            let html = client.get(&url).send().await
                .map_err(|e| AxgaError::Network(e.to_string()))?
                .text().await
                .map_err(|e| AxgaError::Network(e.to_string()))?;

            let results = parse_ddg_html(&html, max as usize);
            if results.is_empty() {
                Ok("No results found.".into())
            } else {
                Ok(results.join("\n\n"))
            }
        })
    }
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
}

fn parse_ddg_html(html: &str, max: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut i = 0;

    // Simple HTML extraction for DuckDuckGo results
    while i < html.len() && results.len() < max {
        // Find result title
        if let Some(start) = html[i..].find("class=\"result__title\"") {
            let chunk = &html[i + start..];
            // Extract link text
            if let Some(link_start) = chunk.find("class=\"result__a\"") {
                let link_chunk = &chunk[link_start..];
                if let Some(href_start) = link_chunk.find("href=\"") {
                    let href_chunk = &link_chunk[href_start + 6..];
                    if let Some(href_end) = href_chunk.find('"') {
                        // Extract snippet
                        let snippet = if let Some(snip_start) = chunk.find("class=\"result__snippet\"") {
                            let snip = &chunk[snip_start..];
                            snip.find(">").and_then(|gt| {
                                let after = &snip[gt + 1..];
                                after.find("</").map(|end| after[..end].trim().to_string())
                            }).unwrap_or_default()
                        } else {
                            String::new()
                        };

                        let title = &href_chunk[..href_end];
                        results.push(format!("- **{title}**\n  {snippet}"));
                        i += start + href_start + 6 + href_end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_html_returns_no_results() {
        let result = parse_ddg_html("", 5);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_html_no_results_block_returns_empty() {
        // HTML with no result__title divs should yield empty results
        let html = "<html><body><p>No results here</p></body></html>";
        let result = parse_ddg_html(html, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn web_search_tool_name() {
        let tool = WebSearchTool;
        assert_eq!(tool.name(), "web_search");
    }

    #[test]
    fn urlencoding_replaces_spaces() {
        assert_eq!(urlencoding("hello world"), "hello+world");
        assert_eq!(urlencoding("test"), "test");
        assert_eq!(urlencoding(""), "");
        assert_eq!(urlencoding("a b c"), "a+b+c");
    }
}
