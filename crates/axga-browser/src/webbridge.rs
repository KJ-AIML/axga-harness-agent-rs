//! WebBridge backend — talks to Kimi WebBridge at localhost:10086.

use super::BrowserBackend;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;

pub struct WebBridgeBackend {
    client: reqwest::Client,
    session: String,
}

impl WebBridgeBackend {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new(), session: "axga".into() }
    }

    async fn send(&self, action: &str, args: Value) -> AxgaResult<Value> {
        let resp = self.client
            .post("http://127.0.0.1:10086/command")
            .json(&serde_json::json!({"action": action, "args": args, "session": self.session}))
            .send().await
            .map_err(|e| AxgaError::Network(e.to_string()))?;
        resp.json().await.map_err(|e| AxgaError::Network(e.to_string()))
    }
}

#[async_trait::async_trait]
impl BrowserBackend for WebBridgeBackend {
    async fn navigate(&self, url: &str) -> AxgaResult<()> {
        self.send("navigate", serde_json::json!({"url": url})).await?;
        Ok(())
    }
    async fn snapshot(&self) -> AxgaResult<String> {
        let r = self.send("snapshot", serde_json::json!({})).await?;
        Ok(r.to_string())
    }
    async fn click(&self, selector: &str) -> AxgaResult<()> {
        self.send("click", serde_json::json!({"selector": selector})).await?;
        Ok(())
    }
    async fn fill(&self, selector: &str, text: &str) -> AxgaResult<()> {
        self.send("fill", serde_json::json!({"selector": selector, "text": text})).await?;
        Ok(())
    }
    async fn execute_js(&self, js: &str) -> AxgaResult<Value> {
        self.send("execute", serde_json::json!({"js": js})).await
    }
    async fn screenshot(&self) -> AxgaResult<Vec<u8>> { Err(AxgaError::Unsupported("screenshot not available via WebBridge".into())) }
    async fn pdf(&self) -> AxgaResult<Vec<u8>> { Err(AxgaError::Unsupported("pdf not available via WebBridge".into())) }
}
