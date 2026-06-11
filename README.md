<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)">
    <h1>⚡ AXGA</h1>
  </picture>
</p>

<p align="center">
  <strong>The 5.8MB AI coding agent that runs on a $5 VPS.</strong>
</p>

<p align="center">
  <a href="https://github.com/KJ-AIML/axga-harness-agent-rs/releases"><img src="https://img.shields.io/github/v/release/KJ-AIML/axga-harness-agent-rs" alt="Release"></a>
  <a href="https://github.com/KJ-AIML/axga-harness-agent-rs/actions"><img src="https://img.shields.io/github/actions/workflow/status/KJ-AIML/axga-harness-agent-rs/release.yml" alt="CI"></a>
  <img src="https://img.shields.io/badge/rust-1.88+-orange.svg" alt="Rust 1.88+">
  <img src="https://img.shields.io/badge/binary-5.8MB-blue.svg" alt="Binary 5.8MB">
  <img src="https://img.shields.io/badge/memory-19MB-green.svg" alt="Memory 19MB">
  <img src="https://img.shields.io/badge/license-MIT-purple.svg" alt="MIT">
</p>

<p align="center">
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-features">Features</a> •
  <a href="#-tools">Tools</a> •
  <a href="#-telegram-bot">Telegram Bot</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-benchmarks">Benchmarks</a> •
  <a href="#-comparison">Comparison</a>
</p>

---

## Why AXGA?

Existing AI coding agents (Hermes, OpenClaw, Claude Code) are built on Node.js and consume **300MB–2GB RAM at idle**. They cannot run on cheap VPS instances. This excludes indie hackers, students, and emerging markets from running autonomous AI agents.

AXGA is built in Rust. **5.8MB binary. 19MB peak RAM. Zero glibc dependency.** It runs on a 1GB Alibaba Cloud VPS with 97% RAM to spare. You can run **10+ instances** before hitting the wall that kills a single Node.js agent.

<table>
<tr>
<td width="50%">

### 🖥️ TUI Mode
```
┌─ AXGA ── deepseek-chat ──────────────────┐
│                                            │
│  ✦  Check my server RAM                    │
│  ⚙  execute_shell → free -h                │
│  ●  Total: 894 MiB, Used: 376 MiB,         │
│     Available: 517 MiB.                    │
│                                            │
│  ✦  Search for Rust async patterns         │
│  ⚙  web_search → DuckDuckGo                │
│  ●  Top 5 results from Rust-lang.org...    │
│                                            │
│  ✦  Remember I am a founder                │
│  ⚙  memctrl → SQLite stored                │
│  ●  Got it! Stored in project memory.      │
│                                            │
├────────────────────────────────────────────┤
│  > /help                  [1/10]  INSERT   │
└────────────────────────────────────────────┘
```

</td>
<td width="50%">

### 🤖 Telegram Bot
```
@Axga_axtlbot is running (6.0 MB RSS)

Sminvl: What's on this server?
Bot:     ⚙ execute_shell → ls
        You have: axga, axtra, project files

Sminvl: Remember I am a founder
Bot:     ⚙ memctrl → Stored in SQLite
        Got it! Confidence: 1.0

Sminvl: Who am I?
Bot:     ⚙ memctrl query → Result
        You are Sminvl — a founder
        Source: explicit | Trace: root→session

Sminvl: Search for Rust news
Bot:     ⚙ web_search → DuckDuckGo
        Top results: Rust 2026 roadmap...
```

</td>
</tr>
</table>

---

## 🚀 Quick Start

```bash
# One-line install (Linux / macOS)
curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sudo sh

# Homebrew
brew install KJ-AIML/axga/axga

# Arch Linux (AUR)
yay -S axga

# Cargo (coming soon — not yet on crates.io)
cargo install axga-cli

# Docker
docker run -e DEEPSEEK_API_KEY="sk-..." ghcr.io/kj-aiml/axga --help
```

**First run:**
```bash
export DEEPSEEK_API_KEY="sk-..."

# Interactive TUI
axga --provider deepseek --model deepseek-chat

# Single-shot
axga --provider deepseek --model deepseek-chat --prompt "explain Rust ownership"

# Telegram bot
axga --telegram --key "YOUR_BOT_TOKEN" --provider deepseek --model deepseek-chat

# Health check
axga doctor --json
```

**CLI Reference:**

| Flag / Command | Description |
|----------------|-------------|
| `--base-url <URL>` | Override API base URL (e.g., `https://api.openai.com/v1`) |
| `--system-prompt <TEXT>` | Custom system prompt for the agent |
| `--max-turns <N>` | Max conversation turns before auto-summarize (default: 10) |
| `--dir <PATH>` | Working directory for tool operations |
| `--verbose` / `-v` | Enable verbose logging |
| `--onboard` | Run the onboarding/interactive setup wizard |
| `axga models` | List available models for the configured provider |
| `axga config` | Show current resolved configuration |
| `axga mcp` | Start the MCP server (JSON-RPC over stdio) |

---

## ✨ Features

|     |     |
|-----|-----|
| 🖥️ **TUI** | ratatui interface: real-time streaming, syntax-highlighted code, diff preview, two-line footer, 20+ slash commands, vim keys |
| 🤖 **Telegram Bot** | Long-polling + webhook modes, typing indicators, session isolation per chat |
| 🔄 **Real-time Streaming** | Text and tool calls appear character-by-character — no spinner wait |
| 🔐 **Permissions** | Manual/Auto modes, approval dialogs for write/shell, session-level approval memory |
| 🧠 **MemCtrl Memory** | Native SQLite-backed memory layer — store, query, forget with confidence scoring and provenance |
| 🔧 **21 Built-in Tools** | Filesystem, shell (denylist-protected), edit, grep, glob, diff, web search, URL fetch, memory, agent/agent_swarm, background tasks, plan mode, ask user, goal, cron |
| 🚀 **3 Providers** | DeepSeek, OpenAI, Anthropic — swap at runtime with `/provider` |
| 📡 **MCP Server** | JSON-RPC 2.0 over stdio — connects to Claude Desktop, Cursor, any MCP client |
| 🎯 **Goal Mode** | Autonomous task execution with budget tracking (tokens/turns/time) |
| 📋 **Plan Mode** | Read-only exploration → plan review → execute with approval |
| ⏰ **Cron Scheduler** | Schedule recurring prompts with cron expressions |
| 💾 **Sessions** | JSONL save/load, resume conversations, LLM-powered auto-compaction, /undo |
| ⚡ **Resilient** | Exponential backoff on 429/5xx, graceful degradation on tool errors |
| 📦 **5.8MB Binary** | Musl static build, zero glibc, no runtime dependencies |
| 🔒 **Memory Safe** | Rust + mimalloc allocator, 19MB peak RSS under full tool load |
| 🐳 **Docker** | 2-stage Alpine build (`~10MB` image) |
| 🔌 **Systemd** | Auto-start service with `MemoryMax=200M` |
| 🔏 **Shell Safety** | Denylist blocks `rm -rf /`, `dd`, `mkfs`, `curl \| sh`, fork bombs. `--dangerous` to bypass. Permission system for interactive approval. |
| 🔐 **Sensitive Files** | Blocks access to `.env`, SSH keys, credentials, `.pem`, `.key` |
| 📊 **Observability** | Health check (`doctor --json`), structured JSON logging (`--json-log`), token tracking |
| 🕸️ **Browser** (feature-gated) | Full chromiumoxide backend: navigate, click, fill, JS, screenshot, PDF |
| 🎨 **Themes** | Dark + light themes, runtime switching with `/theme` |
| 📏 **Tool Dedup** | Detects repeated tool calls (streak at 3/5/8, force-stop at 12) |

---

## 🔧 Tools

| # | Tool | Description | Safety |
|---|------|-------------|--------|
| 1 | `read_file` | Read files (1MB cap, offset/limit, streaming for large) | — |
| 2 | `write_file` | Write/create files, auto parent dirs | 🔒 Permission |
| 3 | `list_directory` | List directory contents | — |
| 4 | `edit` | Exact string replacement (safer than write_file) | 🔒 Permission |
| 5 | `execute_shell` | Run shell commands (60s timeout, streaming output, bg mode) | 🔒 Denylist + Permission |
| 6 | `grep` | Regex search across files with file filters | — |
| 7 | `glob` | Find files by pattern (`src/**/*.rs`) | — |
| 8 | `diff` | Line-by-line unified diff | — |
| 9 | `memctrl` | SQLite memory layer: add/query/list/forget/doctor | — |
| 10 | `web_search` | DuckDuckGo search (no API key required) | — |
| 11 | `fetch_url` | HTTP GET + HTML-to-text extraction | 🔒 Permission |
| 12 | `agent` | Spawn sub-agent with configurable provider/model | 🔒 Permission |
| 13 | `agent_swarm` | Spawn N agents in parallel with shared prompt | 🔒 Permission |
| 14 | `task_list` | List background shell tasks | — |
| 15 | `task_output` | Read output from background task | — |
| 16 | `task_stop` | Cancel/kill a background task | 🔒 Permission |
| 17 | `enter_plan_mode` | Enter read-only planning mode | — |
| 18 | `exit_plan_mode` | Exit plan mode, resume normal tools | — |
| 19 | `ask_user_question` | Ask structured questions to the user | — |
| 20 | `create_goal` | Create autonomous goal with budget tracking | — |
| 21 | `get_goal` | Check goal status and progress | — |
| 22 | `update_goal` | Update goal status (active/paused/complete/blocked) | — |
| 23 | `set_goal_budget` | Adjust goal budget (tokens/turns/time) | — |
| 24 | `cron_create` | Schedule a recurring prompt with cron expression | — |
| 25 | `cron_list` | List all active cron jobs | — |
| 26 | `cron_delete` | Delete a cron job | — |

**Slash commands:** `/help` `/quit` `/clear` `/tools` `/history` `/status` `/usage` `/compact` `/version` `/export` `/title` `/provider` `/apikey` `/yolo` `/manual` `/theme` `/goal` `/undo`

**Vim keys:** `i` insert • `Esc` normal • `j`/`k` scroll • `G` bottom • `gg` top • `:q` quit • `↑↓` scroll in any mode

---

## 🏗️ Architecture

```
axga-shared (types, errors, memory limits)      ← crates.io ready
  ├── axga-ai (LLM providers + SSE streaming)    ← crates.io ready
  │     └── axga-core (agent loop, tools, state) ← crates.io ready
  │           └── axga-cli (binary entry)
  ├── axga-tui (ratatui app)
  │     └── axga-cli
  └── axga-browser (feature-gated)
```

**Data flow:** Input → `run_turn_streaming()` → LLM stream → real-time TUI updates → tool execution → conversation update → response

**Memory model:**
| Component | Typical | Peak |
|-----------|---------|------|
| Binary + tokio (2 threads, 512KB stack) | 5 MB | 8 MB |
| Conversation (20 turns, auto-summarize) | 5 MB | 40 MB |
| HTTP + TLS (reqwest, connection pool) | 5 MB | 15 MB |
| TUI frame buffer (ratatui List) | 2 MB | 5 MB |
| Tool output (bounded channels) | 3 MB | 20 MB |
| MemCtrl SQLite (rusqlite, bundled) | 1 MB | 3 MB |
| **Total** | **~15 MB** | **~91 MB** |

---

## 📊 Benchmarks

Tested in Docker (Ubuntu 24.04, DeepSeek API, x86_64-musl build):

| Test | RSS | % of 100MB Budget |
|------|-----|-------------------|
| `axga --version` (baseline) | **6.0 MB** | 6% |
| Simple prompt | **12.4 MB** | 12% |
| Tool: list_directory | **12.7 MB** | 13% |
| Tool: execute_shell | **14.6 MB** | 15% |
| Tool: memctrl (SQLite) | **14.9 MB** | 15% |
| Tool: web_search (DuckDuckGo) | **18.7 MB** | 19% |
| Multi-tool (3 tools, 1 turn) | **14.5 MB** | 15% |
| **Telegram bot idle** | **17.6 MB** | 18% |

**Peak: 18.7 MB** — leaves 980+ MB free on a 1GB VPS for OS and other services.

Binary size: **5.8 MB** (musl, LTO, stripped, `opt-level=s`, `panic=abort`).

---

## ⚖️ Comparison

| | **AXGA-rs** | Hermes | OpenClaw | Claude Code | Codex CLI |
|---|---|---|---|---|---|
| **Language** | Rust | Python/Node | Node.js | TypeScript | TypeScript |
| **Binary** | **5.8 MB** | ~50 MB | ~100 MB | N/A | N/A |
| **Idle RAM** | **18 MB** | 300–600 MB | 400–800 MB | 100–500 MB | 50–150 MB |
| **1GB VPS viable** | ✅ **Yes** | ⚠️ Tight | ❌ No | ✅ Yes | ✅ Yes |
| **Browser** | ✅ Feature-gated | ✅ Built-in | ✅ Built-in | ❌ No | ❌ No |
| **Multi-model** | ✅ 3 providers | ✅ 300+ | ✅ BYOK | ❌ Anthropic only | ❌ OpenAI only |
| **Memory layer** | ✅ SQLite (tree-based) | ✅ Auto-learning | ✅ Persistent | ⚠️ Manual | ❌ None |
| **Telegram** | ✅ Built-in | ✅ Built-in | ✅ Built-in | ❌ No | ❌ No |
| **MCP Server** | ✅ JSON-RPC stdio | ❌ No | ❌ No | ✅ Built-in | ❌ No |
| **Plan Mode** | ✅ Read-only → review | ✅ Built-in | ❌ No | ❌ No | ❌ No |
| **Goal Mode** | ✅ Budget tracked | ✅ Built-in | ❌ No | ❌ No | ❌ No |
| **Sub-agents** | ✅ Agent/AgentSwarm | ✅ Built-in | ❌ No | ❌ No | ❌ No |
| **Streaming** | ✅ Real-time to TUI | ✅ Built-in | ✅ Built-in | ✅ Built-in | ✅ Built-in |
| **Static binary** | ✅ musl | ❌ Node runtime | ❌ Node runtime | ❌ Node runtime | ❌ Node runtime |
| **Shell safety** | ✅ Denylist + Permission | ⚠️ Configurable | ⚠️ Configurable | ⚠️ Ask-user | ⚠️ Ask-user |
| **License** | MIT | Apache 2.0 | MIT | Proprietary | MIT |

> AXGA owns the "resource-constrained agent" category. It sacrifices browser convenience and local LLM support for ubiquity on cheap hardware.

---

## ⚙️ Configuration

Create `~/.config/axga/config.toml` (optional — falls back to CLI args and env vars):

```toml
[provider]
provider_type = "deepseek"
model = "deepseek-chat"
system_prompt = "You are a helpful coding assistant."
max_turns = 10

[session]
dir = "~/.config/axga/sessions"
auto_save = true

[telegram]
token = "YOUR_BOT_TOKEN"

[tools]
web_search = true
fetch_url = true
memctrl = true

[memory]
enabled = true
```

---

## 🚢 Deployment

### systemd (Recommended for VPS)
```ini
[Unit]
Description=AXGA Agent
After=network.target

[Service]
Type=simple
User=admin
ExecStart=/usr/local/bin/axga --telegram --provider deepseek
Restart=always
RestartSec=5
Environment=DEEPSEEK_API_KEY=sk-...
MemoryMax=200M

[Install]
WantedBy=multi-user.target
```

### GitHub Actions Release
Tagging `v*.*.*` triggers auto-build for 6 targets (`x86_64`/`aarch64` Linux musl, `x86_64`/`aarch64` macOS, plus browser/minimal variants), SHA256 generation, `.tar.gz` packaging, and GitHub Release creation.

---

## 🧑‍💻 Development

```bash
git clone https://github.com/KJ-AIML/axga-harness-agent-rs
cd axga-harness-agent-rs

# Build
cargo build --release

# Static musl (for VPS)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# Test
cargo test --all                                # 99+ tests

# Lint
cargo clippy -- -D warnings

# With browser feature
cargo build --release --features browser -p axga-cli
```

---

## 📜 License

MIT © AXGA Contributors

---

<p align="center">
  <sub>Built with 🦀 Rust · Powered by DeepSeek · Runs on $5/year VPS</sub>
</p>
