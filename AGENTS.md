# AGENTS.md — AXGA Harness Agent (Rust)

Rust port of the `axga-harness-agent` (Pi fork). Primary constraint: **sub-100MB RAM on 1GB VPS**.

## Quick Reference

| | |
|---|---|
| **Language** | Rust 2024 edition, MSRV 1.85 |
| **Workspace** | Cargo workspace: 5 crates |
| **Memory target** | <100 MB typical, <150 MB peak |
| **Binary target** | `x86_64-unknown-linux-musl` (~5-8 MB) |
| **Build** | `cargo build --release` |
| **Test** | `cargo test --all` |
| **Lint** | `cargo clippy -- -D warnings` |
| **Release** | `scripts/build-release.sh` |

## Crates

| Crate | Responsibility |
|-------|---------------|
| `axga-shared` | Types, errors, memory limits — zero heavy deps |
| `axga-ai` | LLM provider abstraction — HTTP + SSE parsing |
| `axga-core` | Agent runtime — state machine, tool registry, context budgeting |
| `axga-tui` | Terminal UI — ratatui + crossterm |
| `axga-cli` | Binary entry — clap args, tokio runtime, mode dispatch |

## Dependency Graph

```
axga-shared
  ├── axga-ai
  │     └── axga-core
  │           └── axga-cli
  └── axga-tui
        └── axga-cli
```

## Memory Rules (Code-Enforced)

1. **No `read_to_string` on unknown files** — check `metadata().len()` first. >1MB → reject.
2. **No unbounded `Vec`** — use `with_capacity` or bounded channels (`TOOL_CHANNEL_CAP = 100`).
3. **No buffering full HTTP responses** — parse SSE chunks as `&str` → `serde_json::from_str`.
4. **Conversation history auto-summarizes** — oldest turns merged into `AgentMessage::System` when full.
5. **Binary is stripped and LTO-optimized** — `.cargo/config.toml` enforces `opt-level=s`, `panic=abort`, `strip=true`.

## Adding a New Provider

1. Create `crates/axga-ai/src/providers/<name>.rs`
2. Implement a `stream()` function returning `AxgaResult<impl Stream<Item = AxgaResult<StreamEvent>>>`
3. Register in the provider factory in `axga-cli/src/main.rs`

## Adding a New Tool

1. Create `crates/axga-core/src/tools/<name>.rs`
2. Implement the `Tool` trait
3. Register in `axga-core` via `ToolRegistry::register()`
4. Write tests — all tools must have unit tests

## Git Rules

- Only commit files YOU changed in THIS session
- Always `git add <specific-files>` — never `git add -A` or `git add .`
- Forbidden: `git reset --hard`, `git checkout .`, `git clean -fd`, `git stash`
- Never `git commit --no-verify`
- Never commit `target/` directory

## Phase Reference

| Phase | Status | Deliverable |
|-------|--------|-------------|
| 0: Foundation | ✅ Done | Workspace scaffold, ADRs, memory model |
| 1: LLM Plumbing | 🚧 Next | OpenAI streaming, SSE parser, single-shot CLI |
| 2: Agent Runtime | ⬜ | Tool registry, conversation state, shell/fs tools |
| 3: TUI Integration | ⬜ | ratatui app, keyboard nav, diff rendering |
| 4: Provider Expansion | ⬜ | Anthropic, config files, env var parity |
| 5: Parity & Hardening | ⬜ | Remaining tools, memory profiling, stress tests |
| 6: Deployment | ⬜ | CI/CD, musl binary, install script |
