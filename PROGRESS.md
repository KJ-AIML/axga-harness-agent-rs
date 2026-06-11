# Progress — Per-Session Handoff Log

## 2026-06-11 — v0.1.0: Hardening & Docs Sync

### Context
Comprehensive codebase audit and cleanup. Fixed 3 critical, 10 medium, 9 low issues across all crates, scripts, and docs.

### Done
- **`--dangerous` wired**: Flag now flows from CLI → `build_default_registry()` → `ShellTool::new()`. No longer a dead flag.
- **AUR PKGBUILD fixed**: Dual-arch SHA256 support via `_sha256()` function.
- **`scripts/github-release.sh` deleted**: Superseded by CI workflow (6 targets).
- **`build_registry()` extracted**: 4 duplicate registration sites unified into `axga_core::build_default_registry(dangerous)`.
- **`gg` scroll-to-top**: Double-tap `g` in TUI normal mode scrolls to top.
- **`session.rs` memory check**: 1MB size guard before `read_to_string` (adheres to project memory rule #1).
- **Feature gates**: `axga-core` has `memctrl-native` (default) and `memctrl-cli` features.
- **Dead code removed**: `memctrl.rs` (CLI-based, superseded), `events.rs` (deprecated), `light_theme()` (unused).
- **DeepSeek provider module**: Dedicated `axga_ai::providers::deepseek` — no longer an invisible alias.
- **Provider trait**: Unified `Provider` trait in `axga-ai`, all 3 providers implement it. `AgentLoop` dispatches through `Box<dyn Provider>`.
- **Per-tool unit tests**: 36+ new tests across all 7 tool modules (denylist, parameters, memctrl CRUD).
- **Phase 5 Browser Full**: `HeadlessBackend` fully implemented with chromiumoxide — navigate, click, fill, JS execution, screenshot, PDF, DOM snapshot. Gated behind `--features browser`.
- **Phase 6 Multi-agent**: `Orchestrator` with `spawn()` and `spawn_all()` — sub-agents with per-agent (provider, model) config, concurrent execution via `tokio::spawn`. CLI: `axga orchestrate --config agents.json --prompt "..."`.
- **Homebrew tap**: Created `KJ-AIML/homebrew-axga` — `brew tap KJ-AIML/axga && brew install axga`.
- **Unused deps cleaned**: 7 removed from `axga-tui`.
- **Unsafe block removed**: Spurious `unsafe` wrapper on `std::env::remove_var`.
- **README synced**: Fixed "14" → "11" slash commands, removed fake `[security]` config section, added CLI reference table (9 undocumented flags/commands).
- **SHA256 pipeline**: `sha256-update.sh` handles dual-arch AUR; Homebrew formula has correct structure.
- **All 3 phases from PLAN.md complete**: Critical fixes, doc sync, code quality, tests & design.

## 2026-06-11 — v0.1.0: Streaming UI + Permission System

### Context
Analyzed kimi-code architecture and identified top UX gaps. Implemented the two highest-impact features.

### Done
- **Real-time streaming to TUI**: `StreamHandler` trait with `run_turn_streaming()` — text deltas, tool calls, and results appear in real-time instead of blocking with spinner.
- **Permission/approval system**: `PermissionManager` with `--yolo` / `Manual` modes. Read-only tools auto-approved. `/yolo` and `/manual` slash commands.
- **Provider wizard flow**: Guided `/provider deepseek` → API key prompt → model selection → saved to config. Interactive `PendingPrompt` state machine.
- **Dashboard improvements**: Compare tool list, two-line footer, streaming status.
- **All 66 tests passing, clippy clean**

## 2026-05-31 — v0.1.0: Initial Production Release

### Context
Completed full Rust port of axga-harness-agent. Target: sub-100MB RAM on 1GB VPS.

### Done
- **6 crates**: axga-shared, axga-ai, axga-core, axga-tui, axga-cli, axga-browser
- **10 tools**: read_file, write_file, list_directory, execute_shell, grep, glob, diff, memctrl (native SQLite), web_search (DuckDuckGo), fetch_url
- **3 providers**: DeepSeek, OpenAI, Anthropic (SSE streaming)
- **4 interfaces**: TUI (ratatui + scrollbar + markdown + 14 slash commands), CLI single-shot, Telegram bot (long-polling + webhook), MCP server (JSON-RPC stdio)
- **Safety**: Shell denylist (blocks rm -rf /, dd, mkfs, curl|sh, fork bombs), `--dangerous` bypass
- **Memory**: Native SQLite memctrl (no Python dep), 20-turn conversation cap + auto-summarization
- **Resilience**: Exponential backoff on 429/5xx, graceful degradation on tool errors
- **Deployment**: musl static binary, systemd service, Docker (2-stage Alpine), install script with SHA256
- **Distribution**: GitHub Actions release workflow, Homebrew formula, AUR PKGBUILD, crates.io metadata, GPG docs
- **Tests**: 14 passing (shared types, core state, tool registry, provider)

### Verified Metrics
| Metric | Value |
|--------|-------|
| Binary size | 5.8 MB |
| Baseline RSS | 6.0 MB |
| Prompt RSS | 12.4 MB |
| Tool RSS | 14.2 MB |
| Web search RSS | 18.7 MB |
| Peak RSS | 18.7 MB |

### Repo State
- `main` @ `83b0a49`
- `cargo test --all`: **53 pass, 0 fail** (was 14)
- `cargo clippy -- -D warnings`: **clean** (was 9+ warnings)
- `cargo check`: **clean** (was 9 warnings)
- Working tree clean

### Live Services
- Telegram bot: @Axga_axtlbot (Alibaba Cloud VPS, admin user)
- DeepSeek API key configured

### Next Actions
1. Publish axga-shared, axga-ai, axga-core to crates.io
2. Submit AUR package
3. Implement TUI approval dialogs for permission.ask (currently falls back to allow)
4. Add tabbed model selector dialog (like kimi-code's /model picker)
5. Add LLM-powered context compaction
6. Add event-sourced session persistence (replay/resume/fork)
