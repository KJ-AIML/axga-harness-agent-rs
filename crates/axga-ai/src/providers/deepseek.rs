//! DeepSeek provider — OpenAI-compatible API via api.deepseek.com.
//!
//! Under the hood this uses `OpenAiProvider` with a preset base URL,
//! since DeepSeek's chat API is fully OpenAI-compatible.

use axga_shared::error::AxgaResult;
use axga_shared::types::StreamEvent;
use crate::request::RequestBuilder;
use crate::providers::openai::OpenAiProvider;
use crate::providers::Provider;
use futures::Stream;
use std::future::Future;
use std::pin::Pin;

/// DeepSeek LLM provider.
///
/// Wraps `OpenAiProvider` with the DeepSeek base URL.
#[derive(Clone)]
pub struct DeepSeekProvider {
    inner: OpenAiProvider,
}

impl DeepSeekProvider {
    /// Create a new DeepSeek provider.
    ///
    /// `api_key` — DeepSeek API key (from `DEEPSEEK_API_KEY` env var).
    /// `base_url` — optional override; defaults to `https://api.deepseek.com/v1`.
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> AxgaResult<Self> {
        let url = base_url.unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
        let inner = OpenAiProvider::new(api_key, Some(url))?;
        Ok(Self { inner })
    }

    /// Send a streaming chat request to DeepSeek.
    pub async fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>> {
        self.inner.stream_chat(request).await
    }
}

impl Provider for DeepSeekProvider {
    fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>>> + Send>> {
        let this = self.clone();
        let request = request.clone();
        Box::pin(async move { this.stream_chat(&request).await })
    }
}
