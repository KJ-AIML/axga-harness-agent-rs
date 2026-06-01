# Progress — Per-Session Handoff Log

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
- `cargo test --all`: 14 pass, 0 fail
- `cargo check`: clean (9 warnings, auto-fixable)
- Working tree clean

### Live Services
- Telegram bot: @Axga_axtlbot (Alibaba Cloud VPS, admin user)
- DeepSeek API key configured

### Next Actions
1. Fix 9 cargo warnings (`cargo fix --bin axga`)
2. Full chromiumoxide browser implementation (Phase 5)
3. Multi-agent coordination with resource budgets (Phase 6)
4. Publish axga-shared, axga-ai, axga-core to crates.io
5. Create Homebrew tap repo (KJ-AIML/homebrew-axga)
6. Submit AUR package
