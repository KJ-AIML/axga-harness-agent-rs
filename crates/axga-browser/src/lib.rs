//! Browser automation — feature-gated (chromiumoxide or WebBridge).
//!
//! # RAM Impact
//! chromiumoxide: +80-120MB per tab. Enable only if >512MB available.
//! WebBridge: +0MB (uses existing Chrome/Edge via localhost:10086).

use axga_shared::error::AxgaResult;
use serde_json::Value;

/// Trait for browser backends. Agent code is backend-agnostic.
#[async_trait::async_trait]
pub trait BrowserBackend: Send + Sync {
    async fn navigate(&self, url: &str) -> AxgaResult<()>;
    async fn snapshot(&self) -> AxgaResult<String>;
    async fn click(&self, selector: &str) -> AxgaResult<()>;
    async fn fill(&self, selector: &str, text: &str) -> AxgaResult<()>;
    async fn execute_js(&self, js: &str) -> AxgaResult<Value>;
    async fn screenshot(&self) -> AxgaResult<Vec<u8>>;
    async fn pdf(&self) -> AxgaResult<Vec<u8>>;
}

/// Check if enough RAM is available for browser (512MB threshold).
pub fn has_enough_ram() -> bool {
    // Stub — reads /proc/meminfo on Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    let kb: u64 = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    return kb > 512 * 1024; // 512 MB
                }
            }
        }
    }
    true // Assume enough on non-Linux
}

#[cfg(feature = "webbridge")]
pub mod webbridge;

#[cfg(feature = "chromiumoxide")]
pub mod headless;
