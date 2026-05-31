# AGENTS.md - AXGA Harness Agent (Rust)

Rust port of `axga-harness-agent`. Primary constraint: low memory usage on a 1GB VPS.

## Quick Reference

| Item | Value |
|---|---|
| Language | Rust 2024 edition, MSRV 1.88 |
| Workspace | Cargo workspace: 5 crates |
| Memory target | <100 MB typical, <150 MB peak |
| Binary target | `x86_64-unknown-linux-musl` (~5-8 MB target) |
| Build | `cargo build --release` |
| Test | `cargo test --all` |
| Lint | `cargo clippy -- -D warnings` |
| Release | `scripts/build-release.sh` |

## Crates

| Crate | Responsibility |
|---|---|
| `axga-shared` | Types, errors, limits |
| `axga-ai` | LLM provider HTTP clients and SSE parsing |
| `axga-core` | Agent loop, tool registry, provider registry, config, context budgeting |
| `axga-tui` | Terminal UI with ratatui and crossterm |
| `axga-cli` | Binary entry for CLI, TUI, Telegram, spawn |

## Dependency Graph

```text
axga-shared
  -> axga-ai
       -> axga-core
            -> axga-cli
  -> axga-tui
       -> axga-cli
```

## Memory Rules

These are enforced for the highest-risk paths and should guide new code:

1. Check file size before reading unknown local files. Files over the configured cap are rejected.
2. Avoid unbounded buffers. Use explicit caps, `with_capacity`, bounded maps, or bounded channels.
3. Do not buffer full LLM streams. Parse SSE incrementally.
4. Keep conversation history bounded; old turns are summarized when the budget is full.
5. Keep runtime thread counts and stack sizes aligned with `crates/axga-cli/src/runtime.rs`.
6. Keep release size settings aligned with `.cargo/config.toml` and `Cargo.toml`.

Known exception: some command output and serialization paths still allocate complete strings after upstream limits. Do not add new unbounded reads without documenting why.

## Adding a New Provider

1. Add or update the provider spec in `crates/axga-core/src/provider_registry.rs`.
2. Reuse `axga-ai/src/providers/openai.rs` for OpenAI-compatible providers when possible.
3. Add provider-specific client code only when the API is not OpenAI-compatible.
4. Add tests for default model, base URL, API key behavior, and custom base URL override.
5. Confirm `axga models`, CLI, TUI, Telegram, and onboarding all use the registry path.

## Adding a New Tool

1. Create `crates/axga-core/src/tools/<name>.rs`.
2. Implement the `Tool` trait.
3. Register via `ToolRegistry::register()`.
4. Add focused unit tests. Tool output should be bounded before entering conversation history.

## Git Rules

- Only commit files changed in this session.
- Use `git add <specific-files>`.
- Do not use `git add -A` or `git add .`.
- Forbidden unless explicitly requested: `git reset --hard`, `git checkout .`, `git clean -fd`, `git stash`.
- Never use `git commit --no-verify`.
- Never commit `target/`.

## Phase Reference

| Phase | Status | Deliverable |
|---|---|---|
| 0: Foundation | Done | Workspace, ADRs, shared memory model |
| 1: LLM plumbing | Done | OpenAI-compatible and Anthropic streaming, SSE tool-call parsing |
| 2: Agent runtime | Done | Tool registry, conversation state, shell/fs/code/web/memory tools |
| 3: TUI integration | Done | Ratatui app, keyboard modes, slash commands |
| 4: Provider/config | Done | Provider registry, onboarding, saved config, env parity |
| 5: Parity and hardening | In progress | Telegram hardening done; stress/memory profiling still useful |
| 6: Deployment | Partial | CI and release scripts exist; install/package flow still needs validation |
