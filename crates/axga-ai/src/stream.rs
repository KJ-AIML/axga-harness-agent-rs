//! SSE (Server-Sent Events) stream parser.
//!
//! # Memory Rule
//! Parse SSE chunks as `&str` → `serde_json::from_str`.
//! Never buffer the full response body.

use axga_shared::error::{AxgaError, AxgaResult};
use axga_shared::types::StreamEvent;
use bytes::Bytes;
use futures::Stream;
use reqwest::Error as ReqwestError;
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Parse a single SSE line into a `StreamEvent`.
pub fn parse_sse_line(line: &str) -> Option<AxgaResult<StreamEvent>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let data = line.strip_prefix("data: ")?;
    if data == "[DONE]" {
        return Some(Ok(StreamEvent::Done));
    }

    match serde_json::from_str::<Value>(data) {
        Ok(parsed) => {
            if let Some(choices) = parsed["choices"].as_array() {
                for choice in choices {
                    if let Some(delta) = choice.get("delta") {
                        if let Some(content) = delta["content"].as_str() {
                            return Some(Ok(StreamEvent::TextDelta {
                                text: content.to_string(),
                            }));
                        }
                        if let Some(tool_calls) = delta["tool_calls"].as_array() {
                            if let Some(tc) = tool_calls.iter().next() {
                                let id = tc["id"].as_str().unwrap_or("").to_string();
                                let name =
                                    tc["function"]["name"].as_str().unwrap_or("").to_string();
                                let args = tc["function"]["arguments"].as_str().unwrap_or("");
                                return Some(Ok(StreamEvent::ToolCallDelta {
                                    id,
                                    name,
                                    args_fragment: args.to_string(),
                                }));
                            }
                        }
                    }
                    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                        if reason == "stop" {
                            return Some(Ok(StreamEvent::Done));
                        }
                    }
                }
            }
            if parsed["type"].as_str() == Some("content_block_delta") {
                let delta = &parsed["delta"];
                if let Some(text) = delta["text"].as_str() {
                    return Some(Ok(StreamEvent::TextDelta {
                        text: text.to_string(),
                    }));
                }
            }
            if parsed["type"].as_str() == Some("message_stop") {
                return Some(Ok(StreamEvent::Done));
            }
            if parsed.get("usage").is_some() {
                let usage = &parsed["usage"];
                return Some(Ok(StreamEvent::Usage {
                    input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
                    output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                }));
            }
            None
        }
        Err(e) => Some(Err(AxgaError::Serialization(e.to_string()))),
    }
}

/// A stream wrapper that parses raw byte chunks from an HTTP response
/// into `StreamEvent` items via SSE line parsing.
pub struct SseStream<S> {
    pub inner: S,
    pub buffer: String,
    pub done: bool,
}

impl<S> Stream for SseStream<S>
where
    S: Stream<Item = Result<Bytes, ReqwestError>> + Unpin,
{
    type Item = AxgaResult<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        loop {
            while let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + 1..].to_string();

                if let Some(event) = parse_sse_line(&line) {
                    match &event {
                        Ok(StreamEvent::Done) => {
                            self.done = true;
                            return Poll::Ready(Some(Ok(StreamEvent::Done)));
                        }
                        _ => return Poll::Ready(Some(event)),
                    }
                }
            }

            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    if self.buffer.len() + text.len() > 100_000 {
                        return Poll::Ready(Some(Err(AxgaError::LlmProvider(
                            "SSE buffer limit exceeded".into(),
                        ))));
                    }
                    self.buffer.push_str(&text);
                }
                Poll::Ready(Some(Err(e))) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(AxgaError::Network(e.to_string()))));
                }
                Poll::Ready(None) => {
                    self.done = true;
                    if !self.buffer.trim().is_empty() {
                        if let Some(event) = parse_sse_line(&self.buffer) {
                            return Poll::Ready(Some(event));
                        }
                    }
                    return Poll::Ready(Some(Ok(StreamEvent::Done)));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
