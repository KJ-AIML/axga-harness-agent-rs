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
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Parse a single SSE line into zero or more `StreamEvent`s.
pub fn parse_sse_line(line: &str) -> Vec<AxgaResult<StreamEvent>> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let Some(data) = line.strip_prefix("data: ") else {
        return Vec::new();
    };
    if data == "[DONE]" {
        return vec![Ok(StreamEvent::Done)];
    }

    match serde_json::from_str::<Value>(data) {
        Ok(parsed) => parse_json_event(&parsed),
        Err(e) => vec![Err(AxgaError::Serialization(e.to_string()))],
    }
}

fn parse_json_event(parsed: &Value) -> Vec<AxgaResult<StreamEvent>> {
    let mut events = Vec::new();

    if let Some(choices) = parsed["choices"].as_array() {
        for choice in choices {
            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta["content"].as_str() {
                    events.push(Ok(StreamEvent::TextDelta {
                        text: content.to_string(),
                    }));
                }
                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let index = tc["index"].as_u64().map(|value| value as usize);
                        let id = tc["id"].as_str().unwrap_or("").to_string();
                        let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                        let args = tc["function"]["arguments"].as_str().unwrap_or("");
                        events.push(Ok(StreamEvent::ToolCallDelta {
                            index,
                            id,
                            name,
                            args_fragment: args.to_string(),
                        }));
                    }
                }
            }
            if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                if reason == "stop" {
                    events.push(Ok(StreamEvent::Done));
                }
            }
        }
    }

    match parsed["type"].as_str() {
        Some("content_block_start") => {
            let block = &parsed["content_block"];
            if block["type"].as_str() == Some("tool_use") {
                let index = parsed["index"].as_u64().map(|value| value as usize);
                events.push(Ok(StreamEvent::ToolCallDelta {
                    index,
                    id: block["id"].as_str().unwrap_or("").to_string(),
                    name: block["name"].as_str().unwrap_or("").to_string(),
                    args_fragment: String::new(),
                }));
            }
        }
        Some("content_block_delta") => {
            let delta = &parsed["delta"];
            if let Some(text) = delta["text"].as_str() {
                events.push(Ok(StreamEvent::TextDelta {
                    text: text.to_string(),
                }));
            }
            if let Some(partial_json) = delta["partial_json"].as_str() {
                let index = parsed["index"].as_u64().map(|value| value as usize);
                events.push(Ok(StreamEvent::ToolCallDelta {
                    index,
                    id: String::new(),
                    name: String::new(),
                    args_fragment: partial_json.to_string(),
                }));
            }
        }
        Some("message_stop") => {
            events.push(Ok(StreamEvent::Done));
        }
        _ => {}
    }

    if parsed.get("usage").is_some() {
        let usage = &parsed["usage"];
        events.push(Ok(StreamEvent::Usage {
            input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
        }));
    }

    events
}

/// A stream wrapper that parses raw byte chunks from an HTTP response
/// into `StreamEvent` items via SSE line parsing.
pub struct SseStream<S> {
    pub inner: S,
    pub buffer: String,
    pub pending: VecDeque<AxgaResult<StreamEvent>>,
    pub done: bool,
}

impl<S> Stream for SseStream<S>
where
    S: Stream<Item = Result<Bytes, ReqwestError>> + Unpin,
{
    type Item = AxgaResult<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(event) = self.pending.pop_front() {
            return Poll::Ready(Some(event));
        }

        if self.done {
            return Poll::Ready(None);
        }

        loop {
            while let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + 1..].to_string();

                self.pending.extend(parse_sse_line(&line));
                if let Some(event) = self.pending.pop_front() {
                    match event {
                        Ok(StreamEvent::Done) => {
                            self.done = true;
                            return Poll::Ready(Some(Ok(StreamEvent::Done)));
                        }
                        other => return Poll::Ready(Some(other)),
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
                        let line = self.buffer.clone();
                        self.pending.extend(parse_sse_line(&line));
                        if let Some(event) = self.pending.pop_front() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_line_returns_all_tool_call_deltas() {
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"read_file","arguments":"{\"path\":"}},{"index":1,"id":"call_b","function":{"name":"list_directory","arguments":"{\"path\":\".\"}"}}]}}]}"#;

        let events = parse_sse_line(line);

        assert_eq!(events.len(), 2);
        match events[0].as_ref().unwrap() {
            StreamEvent::ToolCallDelta {
                index,
                id,
                name,
                args_fragment,
            } => {
                assert_eq!(*index, Some(0));
                assert_eq!(id, "call_a");
                assert_eq!(name, "read_file");
                assert_eq!(args_fragment, "{\"path\":");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        match events[1].as_ref().unwrap() {
            StreamEvent::ToolCallDelta {
                index, id, name, ..
            } => {
                assert_eq!(*index, Some(1));
                assert_eq!(id, "call_b");
                assert_eq!(name, "list_directory");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn anthropic_tool_use_start_and_json_delta_are_parsed() {
        let start = r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file","input":{}}}"#;
        let delta = r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"README.md\"}"}}"#;

        let start_events = parse_sse_line(start);
        let delta_events = parse_sse_line(delta);

        match start_events[0].as_ref().unwrap() {
            StreamEvent::ToolCallDelta {
                index, id, name, ..
            } => {
                assert_eq!(*index, Some(1));
                assert_eq!(id, "toolu_1");
                assert_eq!(name, "read_file");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        match delta_events[0].as_ref().unwrap() {
            StreamEvent::ToolCallDelta {
                index,
                args_fragment,
                ..
            } => {
                assert_eq!(*index, Some(1));
                assert_eq!(args_fragment, "{\"path\":\"README.md\"}");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
