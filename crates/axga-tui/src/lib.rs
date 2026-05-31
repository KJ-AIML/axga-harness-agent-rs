//! `axga-tui` — Terminal UI using ratatui + crossterm.
//!
//! # Layout (Phase 3)
//! ```text
//! ┌──────────────────────────────────────┐
//! │  Chat History                        │
//! │                                      │
//! │  User: ...                           │
//! │  Assistant: ...                      │
//! │                                      │
//! ├──────────────────────────────────────┤
//! │  Status Bar: [model] [tokens] [mem]  │
//! ├──────────────────────────────────────┤
//! │  > user input here                   │
//! └──────────────────────────────────────┘
//! ```

pub mod app;
pub mod events;
pub mod ui;

pub use app::App;
