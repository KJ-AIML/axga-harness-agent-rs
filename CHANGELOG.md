# Changelog

All notable changes to AXGA.

## [0.1.0] — 2026-05-31

### Added
- **6 crates**: axga-shared, axga-ai, axga-core, axga-tui, axga-cli, axga-browser
- **10 built-in tools**: read_file, write_file, list_directory, execute_shell, grep, glob, diff, memctrl (native SQLite), web_search (DuckDuckGo), fetch_url
- **3 LLM providers**: DeepSeek, OpenAI, Anthropic with SSE streaming
- **4 interfaces**: TUI (ratatui), CLI single-shot, Telegram bot (long-polling + webhook), MCP server (JSON-RPC stdio)
- **Shell safety**: Denylist blocks `rm -rf /`, `dd`, `mkfs`, `curl | sh`, fork bombs. `--dangerous` bypass.
- **Native MemCtrl**: SQLite-backed memory layer (rusqlite, no Python dependency)
- **Conversation management**: 20-turn cap, auto-summarization, JSONL session persistence
- **Resilience**: Exponential backoff on 429/5xx, graceful tool error degradation
- **Observability**: Health check (`axga doctor --json`), structured JSON logging (`--json-log`)
- **Deployment**: musl static binary (5.8MB), systemd service, Docker (2-stage Alpine), install script with SHA256
- **Distribution**: GitHub Actions release workflow (tag-triggered), Homebrew formula, AUR PKGBUILD, GPG signing docs
- **TUI features**: Scrollbar, markdown rendering, 14 slash commands, vim keybindings
- **Browser crate**: `BrowserBackend` trait, WebBridge client, chromiumoxide stubs (feature-gated)
- **Memory optimization**: mimalloc allocator, 512KB tokio thread stacks
- **Tests**: 14 passing (types, state, tool registry, provider)
