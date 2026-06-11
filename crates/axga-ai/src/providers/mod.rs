//! LLM provider implementations.

use axga_shared::error::AxgaResult;
use axga_shared::types::StreamEvent;
use crate::request::RequestBuilder;
use futures::Stream;
use std::future::Future;
use std::pin::Pin;

/// Unified provider trait for LLM streaming chat.
///
/// All providers implement this trait so the agent loop can call
/// any provider through a single interface.
pub trait Provider: Send + Sync {
    /// Send a streaming chat request and return a stream of events.
    #[allow(clippy::type_complexity)]
    fn stream_chat(
        &self,
        request: &RequestBuilder,
    ) -> Pin<Box<dyn Future<Output = AxgaResult<Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>> + Send>>>> + Send>>;
}

pub mod deepseek;
pub mod openai;
pub mod anthropic;
