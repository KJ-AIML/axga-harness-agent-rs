# AXGA Harness Agent (Rust)

Rust 2024 AI coding agent built for small VPS deployments. The current MSRV is **Rust 1.88**, matching `Cargo.toml`, `rust-toolchain.toml`, and CI.

AXGA can run as:

- a single-shot CLI
- an interactive ratatui TUI
- a Telegram bot
- a spawned sub-agent process

The primary design target is low memory use on a 1GB VPS: **under 100 MB typical RSS and under 150 MB peak** for normal workflows.

## Quick Start

```bash
# install/build from source
cargo build --release

# first-run setup; validates provider credentials and can save Telegram config
axga --onboard

# list supported providers and default models
axga models

# single-shot prompt
axga --provider deepseek --prompt "explain Rust ownership"

# TUI mode; saved config is used when flags are omitted
axga

# Telegram mode; uses saved [telegram] token or --key
axga --telegram
```

## Providers

Provider config is registry-backed in `axga-core`, so CLI, TUI, and Telegram use the same defaults and API-key resolution.

| Provider | API style | Env var | Default base URL | Default model |
|---|---|---|---|---|
| `openai` | OpenAI-compatible | `OPENAI_API_KEY` | `https://api.openai.com/v1` | `gpt-4o-mini` |
| `deepseek` | OpenAI-compatible | `DEEPSEEK_API_KEY` | `https://api.deepseek.com/v1` | `deepseek-chat` |
| `anthropic` | Native Anthropic | `ANTHROPIC_API_KEY` | native endpoint | `claude-sonnet-4-20250514` |
| `openrouter` | OpenAI-compatible | `OPENROUTER_API_KEY` | `https://openrouter.ai/api/v1` | `openai/gpt-4o-mini` |
| `groq` | OpenAI-compatible | `GROQ_API_KEY` | `https://api.groq.com/openai/v1` | `llama-3.3-70b-versatile` |
| `ollama` | OpenAI-compatible local | none | `http://localhost:11434/v1` | `llama3.2` |

Use `--base-url` for a custom OpenAI-compatible endpoint.

## Configuration

Run onboarding for the common path:

```bash
axga --onboard
axga --onboard --telegram --key "YOUR_BOT_TOKEN"
```

Onboarding writes `~/.config/axga/config.toml`, validates provider credentials before saving, and validates Telegram tokens with `getMe`.

Example config:

```toml
[provider]
provider_type = "deepseek"
model = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
system_prompt = "You are a helpful coding assistant."
max_turns = 10

[telegram]
token = "YOUR_BOT_TOKEN"
allowed_users = [123456789]

[session]
dir = "~/.config/axga/sessions"
auto_save = true

[tools]
web_search = true
fetch_url = true
memctrl = true

[memory]
enabled = true
memctrl_path = "memctrl"
```

`axga config` shows saved config state without printing API keys or bot tokens. Explicit CLI flags override saved config.

## Telegram Mode

Telegram mode is designed for multi-user safety:

- each chat has isolated bounded conversation state
- active chat state is capped
- `allowed_users` is enforced when configured
- replies are plain text, not Markdown-formatted model output
- long replies are split into Telegram-safe chunks
- `/help`, `/reset`, and `/status` are available

## Tools

| Tool | Description |
|---|---|
| `read_file` | Read files with the configured size cap |
| `write_file` | Write files and create parent directories |
| `list_directory` | List directory contents |
| `execute_shell` | Run shell commands with timeout handling |
| `grep` | Regex search across bounded files |
| `glob` | Find files by pattern |
| `diff` | Line-by-line diff for bounded files |
| `memctrl` | Store, query, and forget project memory facts |
| `web_search` | DuckDuckGo web search |
| `fetch_url` | Fetch and extract bounded URL text |

## Workspace

```text
axga-shared  -> shared types, errors, and limits
axga-ai      -> provider HTTP/SSE clients
axga-core    -> agent loop, tools, config, provider registry
axga-tui     -> ratatui UI
axga-cli     -> binary entry for CLI, TUI, Telegram, spawn
```

## Memory Policy

The memory target is a design constraint, not a blanket proof over every allocation. The code currently enforces the highest-risk paths:

- bounded local file reads before loading content
- bounded HTTP response collection for non-streaming fetches
- streaming LLM SSE parsing
- bounded conversation history with summarization
- capped Telegram chat conversations
- small Tokio runtime: 2 worker threads, 4 blocking threads, 512 KB stack per worker
- release profile optimized for size: `opt-level=s`, LTO, `strip=true`, `panic=abort`

Known exceptions: some command output and serialization paths still allocate complete strings after upstream size checks. Treat new unbounded buffers as bugs unless there is a documented reason.

## Status

| Phase | Status | Current state |
|---|---|---|
| 0: Foundation | Done | Workspace, shared types, ADRs, memory limits |
| 1: LLM plumbing | Done | OpenAI-compatible and Anthropic streaming, SSE tool-call parsing |
| 2: Agent runtime | Done | Tool registry, shell/fs/code/web/memory tools, conversation state |
| 3: TUI integration | Done | Ratatui app, keyboard modes, slash commands |
| 4: Provider/config | Done | Registry-backed providers, onboarding, saved config |
| 5: Hardening | In progress | Telegram hardened; memory profiling and stress tests still useful |
| 6: Deployment | Partial | CI and release scripts exist; install/package flow still needs validation |

## Development

```bash
cargo fmt --check
cargo test --all
cargo clippy -- -D warnings
cargo build --release
```

Release build:

```bash
scripts/build-release.sh
```

## License

MIT
