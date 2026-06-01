//! Headless Chrome backend via chromiumoxide.

use super::BrowserBackend;
use axga_shared::error::{AxgaError, AxgaResult};
use serde_json::Value;

pub struct HeadlessBackend {}

#[async_trait::async_trait]
impl BrowserBackend for HeadlessBackend {
    async fn navigate(&self, _url: &str) -> AxgaResult<()> {
        Err(AxgaError::Unsupported("chromiumoxide not yet wired — Phase 2".into()))
    }
    async fn snapshot(&self) -> AxgaResult<String> { Err(AxgaError::Unsupported("not yet".into())) }
    async fn click(&self, _s: &str) -> AxgaResult<()> { Err(AxgaError::Unsupported("not yet".into())) }
    async fn fill(&self, _s: &str, _t: &str) -> AxgaResult<()> { Err(AxgaError::Unsupported("not yet".into())) }
    async fn execute_js(&self, _js: &str) -> AxgaResult<Value> { Err(AxgaError::Unsupported("not yet".into())) }
    async fn screenshot(&self) -> AxgaResult<Vec<u8>> { Err(AxgaError::Unsupported("not yet".into())) }
    async fn pdf(&self) -> AxgaResult<Vec<u8>> { Err(AxgaError::Unsupported("not yet".into())) }
}
