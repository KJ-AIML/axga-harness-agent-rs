//! OpenAI Chat Completions provider with streaming.

use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::StreamEvent;
use crate::request::RequestBuilder;
use crate::stream::SseStream;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;

#[derive(Clone)]
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> AxgaResult<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| AxgaError::Config("OPENAI_API_KEY not set".into()))?;
        let base_url = base_url
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let client = Client::builder()
            .pool_max_idle_per_host(2)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AxgaError::Network(e.to_string()))?;
        Ok(Self { client, api_key, base_url })
    }

    pub async fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>> {
        let body = request.build_openai_body();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AxgaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(AxgaError::RateLimited(text));
            }
            return Err(AxgaError::Http { status: status.as_u16(), body: text });
        }

        Ok(Box::pin(SseStream {
            inner: response.bytes_stream(),
            buffer: String::with_capacity(4096),
            done: false,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_requires_api_key() {
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
        assert!(OpenAiProvider::new(None, None).is_err());
    }
}
