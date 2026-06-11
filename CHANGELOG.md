# Changelog

All notable changes to AXGA.

## [0.1.1] — 2026-06-11

### Added
- **Discord bot**: REST API polling client with @mention detection. Fetches 10-message history for thread context, shows typing indicator, and supports rich formatting (bold, italic, `code`, code blocks, quotes, underline, strikethrough). Long replies auto-split across 2000-char chunks.
- **Streaming TUI**: `run_turn_streaming()` + `StreamHandler` trait — real-time text and tool deltas replace the spinner, providing live streaming of LLM responses in the TUI.
- **Permissions system**: `PermissionManager` with Manual and Auto modes. `/yolo` slash command and `--yolo` CLI flag for auto-approve. Read-only tools (read, grep, glob) auto-approved by default.
- **Plan mode**: Enter/Exit plan mode tools block write, shell, and network-mutating tools while allowing read-only exploration. Global `PLAN_MODE` flag enforced across all tools.
- **Goal mode**: `GoalManager` with persistent SQLite-backed goal tracking. `create_goal`, `get_goal`, `update_goal`, and `set_goal_budget` tools for structured multi-step task management.
- **Cron**: Background scheduled task system. `cron_create`, `cron_list`, and `cron_delete` tools for recurring agent tasks.
- **Edit tool**: Incremental file editing (`EditTool`) — exact string replacement in existing files, complementing overwrite/appends from write_file.
- **Ask user tool**: Interactive clarification prompts (`AskUserQuestionTool`) for resolving ambiguity during agent runs.
- **Multi-agent (swarm)**: `AgentTool` for spawning sub-agents and `AgentSwarmTool` for coordinated multi-agent orchestration (feature-gated behind `swarm`).
- **Tool count**: Expanded from 10 to 26 tools covering filesystem, shell, code search, web, memory, tasks, plan/goal modes, cron, multi-agent, and interactive prompts.

### Changed
- **Discord thread context**: Messages now include up to 10 prior messages as system-prompt context so the LLM follows multi-turn conversations naturally within a channel.
- **TUI**: Streaming replaces the spinner; token usage shown inline; two-line footer with status, model, and permissions mode.
- **Providers**: DeepSeek, OpenAI, and Anthropic providers all support streaming SSE output path.

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
