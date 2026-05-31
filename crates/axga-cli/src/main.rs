//! `axga` — AI Coding Agent CLI.

// Use mimalloc for better memory efficiency (replaces system allocator).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod runtime;
mod tui_mode;