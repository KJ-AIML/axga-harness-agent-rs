//! Conversation state machine with bounded history.
//!
//! # Memory Model
//!
//! ```text
//! VecDeque<AgentMessage>  ← capped at MAX_CONVERSATION_TURNS (20)
//!   │
//!   ├── When full: summarize oldest 5 turns into a single AgentMessage::System
//!   └── Summary content ≤ 500 chars (hard cap)
//! ```

use axga_shared::limits;
use axga_shared::types::{AgentMessage, AgentState};
use std::collections::VecDeque;

/// A bounded conversation transcript.
///
/// New messages are pushed to the back. When the history reaches
/// `MAX_CONVERSATION_TURNS`, the oldest turns are summarized into a
/// single system message.
#[derive(Debug, Clone)]
pub struct Conversation {
    messages: VecDeque<AgentMessage>,
    state: AgentState,
    turn_count: usize,
}

impl Conversation {
    /// Create a new conversation with preallocated capacity.
    pub fn new() -> Self {
        Self {
            messages: VecDeque::with_capacity(limits::MAX_CONVERSATION_TURNS),
            state: AgentState::Idle,
            turn_count: 0,
        }
    }

    /// Push a message into the conversation.
    /// Automatically triggers summarization if the history is full.
    pub fn push(&mut self, msg: AgentMessage) {
        if self.messages.len() >= limits::MAX_CONVERSATION_TURNS {
            self.summarize_oldest();
        }
        self.messages.push_back(msg);
        self.turn_count += 1;
    }

    /// Current agent state.
    pub fn state(&self) -> AgentState {
        self.state
    }

    /// Transition to a new state.
    pub fn set_state(&mut self, new_state: AgentState) {
        self.state = new_state;
    }

    /// Iterate over messages (for feeding into LLM context).
    pub fn messages(&self) -> impl Iterator<Item = &AgentMessage> {
        self.messages.iter()
    }

    /// Number of messages currently stored.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns `true` if the conversation has no messages.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Total turns in this conversation (including summarized ones).
    pub fn turn_count(&self) -> usize {
        self.turn_count
    }

    /// Clear all messages and reset state.
    pub fn reset(&mut self) {
        self.messages.clear();
        self.state = AgentState::Idle;
        self.turn_count = 0;
    }

    /// Summarize the oldest 5 turns into a single system message.
    /// This keeps the history bounded without silently dropping context.
    fn summarize_oldest(&mut self) {
        let drain_count = 5.min(self.messages.len());
        if drain_count == 0 {
            return;
        }

        let mut summary = String::with_capacity(512);
        summary.push_str("[Summarized earlier context]\n");

        for _ in 0..drain_count {
            if let Some(msg) = self.messages.pop_front() {
                match msg {
                    AgentMessage::User { content } => {
                        summary.push_str("User: ");
                        summary.push_str(&truncate_for_summary(&content, 100));
                        summary.push('\n');
                    }
                    AgentMessage::Assistant { content } => {
                        if let Some(ref text) = content.text {
                            summary.push_str("Assistant: ");
                            summary.push_str(&truncate_for_summary(text, 100));
                            summary.push('\n');
                        }
                        if let Some(ref calls) = content.tool_calls {
                            for tc in calls {
                                summary.push_str(&format!("  → used tool: {}\n", tc.name));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Truncate summary to 500 chars max
        if summary.len() > 500 {
            summary.truncate(497);
            summary.push_str("...");
        }

        self.messages.push_front(AgentMessage::System {
            content: summary,
        });
    }

    /// Remove the last N user-prompt turns and all subsequent messages.
    /// Returns the number of messages removed.
    pub fn undo(&mut self, n: usize) -> usize {
        if n == 0 || self.messages.is_empty() {
            return 0;
        }

        let mut user_count = 0;
        let mut cut_idx = self.messages.len();
        for i in (0..self.messages.len()).rev() {
            if matches!(self.messages[i], AgentMessage::User { .. }) {
                user_count += 1;
                if user_count >= n {
                    cut_idx = i;
                    break;
                }
            }
        }

        let removed = self.messages.len() - cut_idx;
        self.messages.truncate(cut_idx);
        self.turn_count = self.turn_count.saturating_sub(removed);
        removed
    }

    /// LLM-powered context compaction.
    ///
    /// Replaces the oldest `n = min(10, 50% of total)` messages with a single
    /// system message containing the LLM-generated `summary`. This is used
    /// for explicit `/compact` commands; the naive `summarize_oldest()` still
    /// serves as the automatic hard-cap fallback.
    ///
    /// Returns the number of messages replaced (excluding the new System message).
    pub fn compact(&mut self, summary: String) -> usize {
        let n = (self.messages.len() / 2).min(10);
        if n == 0 {
            return 0;
        }

        for _ in 0..n {
            self.messages.pop_front();
        }

        self.messages.push_front(AgentMessage::System {
            content: summary,
        });
        self.turn_count = self.turn_count.saturating_sub(n);
        n
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate a string for summarization purposes.
fn truncate_for_summary(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push_str("...");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_bounded() {
        let mut conv = Conversation::new();
        for i in 0..25 {
            conv.push(AgentMessage::User {
                content: format!("message {i}"),
            });
        }
        // Should not exceed MAX_CONVERSATION_TURNS
        assert!(conv.len() <= limits::MAX_CONVERSATION_TURNS + 1);
    }

    #[test]
    fn summarization_produces_system_message() {
        let mut conv = Conversation::new();
        for i in 0..25 {
            conv.push(AgentMessage::Assistant {
                content: axga_shared::types::AssistantContent {
                    text: Some(format!("response {i}")),
                    tool_calls: None,
                    thinking: None,
                },
            });
        }
        // First message should be a system summary
        assert!(matches!(conv.messages().next(), Some(AgentMessage::System { .. })));
    }
}
