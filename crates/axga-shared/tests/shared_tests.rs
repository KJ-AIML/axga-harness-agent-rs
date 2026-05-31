use axga_shared::limits;
use axga_shared::types::{AgentMessage, TokenBudget};

#[test]
fn token_budget_enforces_limit() {
    let mut budget = TokenBudget::new(1000);
    assert!(budget.reserve(500).is_ok());
    assert!(budget.reserve(600).is_err());
    assert_eq!(budget.used, 500);
}

#[test]
fn token_estimation_is_positive() {
    let tokens = TokenBudget::estimate_tokens("hello world");
    assert!(tokens > 0);
    assert!(tokens <= 10);
}

#[test]
fn limits_are_reasonable() {
    assert!(limits::MAX_FILE_READ_SIZE > 0);
    assert!(limits::MAX_CONVERSATION_TURNS > 0);
    assert!(limits::MAX_CONTEXT_TOKENS > 0);
}

#[test]
fn message_serialization_roundtrip() {
    let msg = AgentMessage::User {
        content: "hello world".into(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        AgentMessage::User { content } => assert_eq!(content, "hello world"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn assistant_message_roundtrip() {
    let msg = AgentMessage::Assistant {
        content: axga_shared::types::AssistantContent {
            text: Some("response".into()),
            tool_calls: None,
            thinking: None,
        },
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
    match parsed {
        AgentMessage::Assistant { content } => {
            assert_eq!(content.text.unwrap(), "response");
        }
        _ => panic!("wrong variant"),
    }
}
