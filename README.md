<p align="center">
  <img src="https://img.shields.io/badge/rust-1.88+-orange.svg" alt="Rust 1.88+">
  <img src="https://img.shields.io/badge/memory-<18MB-green.svg" alt="Memory <18MB">
  <img src="https://img.shields.io/badge/binary-4.7MB-blue.svg" alt="Binary 4.7MB">
  <img src="https://img.shields.io/badge/license-MIT-purple.svg" alt="MIT License">
  <img src="https://img.shields.io/badge/tools-10-cyan.svg" alt="10 Tools">
  <img src="https://img.shields.io/badge/providers-DeepSeek%20%7C%20OpenAI%20%7C%20Anthropic-red.svg" alt="Providers">
</p>

<h1 align="center">⚡ AXGA</h1>
<p align="center"><strong>AI Coding Agent — 4.7MB binary, 18MB RAM, runs anywhere.</strong></p>

<p align="center">
  <a href="#quick-start">Quick Start</a> •
  <a href="#features">Features</a> •
  <a href="#tools">Tools</a> •
  <a href="#telegram-bot">Telegram Bot</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#benchmarks">Benchmarks</a>
</p>

---

## What is AXGA?

AXGA is a **production-grade AI coding agent** written in Rust. It runs in your terminal as a TUI, as a single-shot CLI, or as a Telegram bot. It uses your LLM provider (DeepSeek, OpenAI, Anthropic) and gives the model access to real tools — filesystem, shell, web search, memory.

> **4.7 MB binary. 18 MB RAM. Zero glibc dependency. Fits on a 1GB VPS with room to spare.**

<table>
<tr><td width="50%">

### TUI Mode
```
┌─ AXGA ────────────────────────────────────┐
│  ✦  Can you check my server RAM?           │
│  ⚙  execute_shell → free -h                │
│  ●  Your server has 894 MiB total,         │
│     376 MiB used, 517 MiB available.       │
│                                            │
│  ✦  Search for Rust async patterns         │
│  ⚙  web_search → "Rust async patterns"      │
│  ●  Top results from DuckDuckGo...         │
├────────────────────────────────────────────┤
│  > type your message...           [INSERT] │
└────────────────────────────────────────────┘
```

</td><td width="50%">

### Telegram Bot
```
🤖 @Axga_axtlbot is running

Sminvl: What's the weather?
Bot:     Let me search for you...
         ⚙ web_search → "weather today"

Sminvl: Remember I'm a founder
Bot:     ⚙ memctrl add "Sminvl is a founder"
         Got it! Stored in project memory.

Sminvl: Who am I?
Bot:     ⚙ memctrl query "who is Sminvl"
         You are Sminvl — a founder.
```

</td></tr>
</table>

---

## Quick Start

```bash
# One-line install
curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/master/install.sh | sudo sh

# Set your API key
export DEEPSEEK_API_KEY="sk-..."

# Launch TUI
axga --provider deepseek --model deepseek-chat

# Single-shot
axga --provider deepseek --model deepseek-chat --prompt "explain Rust ownership"

# Telegram bot
axga --telegram --key "YOUR_BOT_TOKEN" --provider deepseek --model deepseek-chat

# Spawn sub-agent
axga --spawn "write unit tests for all modules"
```

---

## Features

| | |
|---|---|
| 🖥️ **TUI** | Full ratatui terminal interface with scrollbar, markdown, vim keys, 14 slash commands |
| 🤖 **Telegram Bot** | Long-polling bot with full tool access, typing indicators, markdown |
| 🧠 **MemCtrl Memory** | Persistent project memory — store facts, query with provenance |
| 🔧 **10 Built-in Tools** | Filesystem, shell, grep, glob, diff, web search, URL fetch, memory |
| 🚀 **3 Providers** | DeepSeek, OpenAI, Anthropic — swap with `--provider` |
| 💾 **Session Persistence** | JSONL save/load, resume conversations |
| ⚡ **Retry & Backoff** | Exponential backoff on 429/5xx, graceful degradation |
| 📦 **4.7 MB Binary** | Musl static build, no glibc, no runtime deps |
| 🔒 **Memory Safe** | Rust + mimalloc, 18MB RSS under full tool load |
| 🐳 **Docker** | 2-stage Alpine build, `docker run -e KEY=... axga` |
| 🔌 **Systemd** | Auto-start service included |

---

## Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read files (1MB cap, streaming for large files) |
| `write_file` | Write/create files with auto parent directory creation |
| `list_directory` | List directory contents |
| `execute_shell` | Run shell commands (60s timeout, cross-platform) |
| `grep` | Regex search across files |
| `glob` | Find files by pattern |
| `diff` | Line-by-line diff between files |
| `memctrl` | **Memory layer** — store/query/forget facts with confidence tracking |
| `web_search` | DuckDuckGo web search (no API key required) |
| `fetch_url` | Fetch and extract text from any URL |

---

## TUI Commands

| `/help` | `/status` | `/usage` | `/compact` |
| `/tools` | `/history` | `/export <file>` | `/title <text>` |
| `/clear` | `/quit` | `/version` | |

**Keys:** `↑↓` scroll • `Esc` normal mode • `i` insert mode • `j/k` vim scroll • `G` bottom • `Ctrl+C` quit

---

## Architecture

```
axga-shared (types, errors, memory limits)
  ├── axga-ai (LLM providers: OpenAI, Anthropic, DeepSeek — SSE streaming)
  │     └── axga-core (agent loop, tool registry, conversation state)
  │           └── axga-cli (binary entry: TUI, Telegram, single-shot, spawn)
  └── axga-tui (ratatui app: theme, markdown, scrollbar, events)
        └── axga-cli
```

**Memory model:**
| Component | Typical | Peak |
|-----------|---------|------|
| Binary + tokio | 5 MB | 8 MB |
| Conversation (20 turns) | 10 MB | 40 MB |
| HTTP + TLS | 8 MB | 15 MB |
| TUI frame buffer | 2 MB | 5 MB |
| Tool output buffers | 5 MB | 20 MB |
| **Total** | **~18 MB** | **~88 MB** |

---

## Benchmarks

| Test | RSS |
|------|-----|
| `axga --version` | 6.0 MB |
| Simple prompt | 11.6 MB |
| File read (500KB) | 15.6 MB |
| Shell execution | 13.6 MB |
| Multi-tool (3 tools) | 13.6 MB |
| **Telegram bot idle** | **17.6 MB** |

All well under the 100MB budget. Peak 18MB under full tool load on a 1GB VPS.

---

## Configuration

Create `~/.config/axga/config.toml`:

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
memctrl_path = "memctrl"
```

---

## Docker

```bash
docker build -t axga .
docker run -e DEEPSEEK_API_KEY="sk-..." axga --provider deepseek --model deepseek-chat --prompt "hello"
docker run -e DEEPSEEK_API_KEY="sk-..." axga --telegram --key "BOT_TOKEN" --provider deepseek
```

---

## Development

```bash
git clone https://github.com/KJ-AIML/axga-harness-agent-rs
cd axga-harness-agent-rs
cargo build --release
cargo test
cargo clippy -- -D warnings
```

---

## License

MIT © AXGA Contributors

---

<p align="center">
  <sub>Built with 🦀 Rust • Powered by DeepSeek • Runs on 1GB VPS</sub>
</p>
