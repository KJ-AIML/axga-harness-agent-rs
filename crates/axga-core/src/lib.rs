//! `axga-core` — Agent runtime.

pub mod agent_loop;
pub mod config;
pub mod context;
pub mod executor;
pub mod io_limits;
pub mod retry;
pub mod session;
pub mod state;
pub mod tools;

pub use agent_loop::run_turn;
pub use config::{Config, load_config, save_config};
pub use session::{list_sessions, load_session, save_session};
pub use state::Conversation;
pub use tools::Tool;
pub use tools::registry::ToolRegistry;
