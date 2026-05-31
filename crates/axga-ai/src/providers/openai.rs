//! OpenAI Chat Completions provider with streaming.

use crate::request::RequestBuilder;
use crate::stream::SseStream;
use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::StreamEvent;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;

#[derive(Clone)]
pub struct OpenAiProvider {
    client: Client,
    api_key: Option<String>,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> AxgaResult<Self> {
        let base_url = base_url
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let client = Client::builder()
            .pool_max_idle_per_host(2)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AxgaError::Network(e.to_string()))?;
        Ok(Self {
            client,
            api_key,
            base_url,
        })
    }

    pub async fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>> {
        let body = request.build_openai_body();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AxgaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(AxgaError::RateLimited(text));
            }
            return Err(AxgaError::Http {
                status: status.as_u16(),
                body: text,
            });
        }

        Ok(Box::pin(SseStream {
            inner: response.bytes_stream(),
            buffer: String::with_capacity(4096),
            pending: std::collections::VecDeque::new(),
            done: false,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_allows_missing_api_key_for_local_compatible_servers() {
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
        assert!(OpenAiProvider::new(None, Some("http://localhost:11434/v1".into())).is_ok());
    }
}
