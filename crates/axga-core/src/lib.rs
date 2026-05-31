//! `axga-core` — Agent runtime: state machine, tool registry, agent loop, context budgeting.

pub mod state;
pub mod context;
pub mod agent_loop;
pub mod executor;
pub mod tools;

pub use state::Conversation;
pub use agent_loop::run_turn;
pub use tools::registry::ToolRegistry;
pub use tools::Tool;
