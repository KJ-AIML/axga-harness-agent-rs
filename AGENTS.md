# AGENTS.md — AXGA Harness Agent (Rust)

Production-grade AI coding agent. **5.8MB binary, 18.7MB peak RSS, runs on 1GB VPS.**

## Quick Reference

| | |
|---|---|
| **Language** | Rust 2024 edition, MSRV 1.88 |
| **Workspace** | 6 crates (shared, ai, core, tui, cli, browser) |
| **Binary** | 5.8 MB (musl, static, stripped) |
| **Peak RAM** | 18.7 MB (tested with web search + memctrl + shell) |
| **Build** | `cargo build --release --target x86_64-unknown-linux-musl` |
| **Test** | `cargo test --all` (14 tests) |
| **Lint** | `cargo clippy -- -D warnings` |
| **Release** | `git tag -s v*.*.* && git push origin v*.*.*` (GitHub Actions auto-builds) |

## Crates

| Crate | Responsibility | crates.io |
|-------|---------------|-----------|
| `axga-shared` | Types, errors, memory limits, config | Ready |
| `axga-ai` | LLM providers (OpenAI, Anthropic, DeepSeek) + SSE streaming | Ready |
| `axga-core` | Agent loop, tool registry, conversation state, retry, memctrl | Ready |
| `axga-tui` | ratatui app: theme, markdown, scrollbar, vim keys | Internal |
| `axga-cli` | Binary: TUI, Telegram, single-shot, spawn, MCP server | Internal |
| `axga-browser` | BrowserBackend trait + WebBridge + chromiumoxide stubs | Feature-gated |

## Dependency Graph

```
axga-shared
  ├── axga-ai ──┐
  │              ├── axga-core ── axga-cli
  └── axga-tui ──┘
  axga-browser (feature-gated, optional)
```

## Memory Rules (Code-Enforced)

1. **No `read_to_string` on unknown files** — check `metadata().len()` first. >1MB → reject.
2. **No unbounded `Vec`** — use `with_capacity` or bounded channels (`TOOL_CHANNEL_CAP = 100`).
3. **No buffering full HTTP responses** — parse SSE chunks as `&str` → `serde_json::from_str`.
4. **Conversation auto-summarizes** — 20-turn cap, oldest merged into `AgentMessage::System`.
5. **Shell denylist** — blocks `rm -rf /`, `dd`, `mkfs`, `curl | sh`, fork bombs. Bypass with `--dangerous`.
6. **Binary is stripped and LTO-optimized** — `.cargo/config.toml` enforces `opt-level=s`, `panic=abort`, `strip=true`.

## Adding a New Provider

1. Create `crates/axga-ai/src/providers/<name>.rs`
2. Implement `stream_chat()` returning `Pin<Box<dyn Stream<Item = AxgaResult<StreamEvent>>>>`
3. Reuse `SseStream` from `stream.rs` for parsing
4. Add match arm in `agent_loop.rs::run_turn()`
5. Add env var and CLI docs

## Adding a New Tool

1. Create `crates/axga-core/src/tools/<name>.rs`
2. Implement the `Tool` trait (`name`, `description`, `parameters`, `execute`)
3. Add `pub mod <name>` to `tools/mod.rs`
4. Register in `axga-cli`: `tui_mode.rs`, `main.rs`, `telegram.rs`
5. Add to test in `tool_registry_tests.rs`
6. Update README tools table

## Key Type Mappings

| TypeScript (axga TS) | Rust (axga-rs) |
|----------------------|----------------|
| `AgentMessage` | `axga_shared::types::AgentMessage` |
| `StreamEvent` | `axga_shared::types::StreamEvent` |
| `StreamOptions` | `axga_ai::providers::openai::OpenAiProvider` (builder pattern) |
| `Tool` | `axga_core::tools::Tool` (trait) |
| `AgentLoopConfig` | Inline params to `run_turn()` |
| `Conversation` | `axga_core::Conversation` |
| `TokenBudget` | `axga_shared::types::TokenBudget` |
| `Config` | `axga_core::config::Config` (TOML) |

## Installation Methods

```sh
# One-liner
curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sudo sh

# Homebrew (after tap creation)
brew install KJ-AIML/axga/axga

# AUR (after upload)
yay -S axga

# Cargo
cargo install axga-cli

# Docker
docker run -e DEEPSEEK_API_KEY="sk-..." ghcr.io/kj-aiml/axga --help
```

## Release Process

```sh
# 1. Sign tag
git tag -s v0.1.0 -m "Release v0.1.0"

# 2. Push — GitHub Actions auto-builds + releases
git push origin v0.1.0

# 3. Verify at https://github.com/KJ-AIML/axga-harness-agent-rs/releases
```

## Git Rules

- Only commit files YOU changed in THIS session
- Always `git add <specific-files>` — never `git add -A` or `git add .`
- Forbidden: `git reset --hard`, `git checkout .`, `git clean -fd`, `git stash`
- Never `git commit --no-verify`
- Never commit `target/`, `.memctrl/`, `*.tar.gz`

## Phase Status

| Phase | Description | Status |
|-------|-------------|--------|
| 1: Hardening | Shell safety, SQLite memctrl, retry, systemd, Telegram | ✅ |
| 2: Browser | chromiumoxide + WebBridge trait | ✅ Stubs |
| 3: Scale | Health check, JSON logging, webhook, crates.io | ✅ |
| 4: Distribution | GitHub Actions, GPG, Homebrew, AUR, README | ✅ |
| 5: Browser Full | chromiumoxide full implementation (navigate, click, fill, JS, screenshot, PDF, DOM) | ✅ |
| 6: Multi-agent | Coordinated sub-agent spawning with resource budgets | ⬜ |
