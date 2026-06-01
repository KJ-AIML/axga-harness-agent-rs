# AXGA Architecture

## Memory Model

Primary constraint: **sub-100MB RAM on 1GB VPS**. Every allocation is budgeted.

### Budget (verified)

| Component | Typical | Peak | Strategy |
|-----------|---------|------|----------|
| Binary + tokio | 5 MB | 8 MB | `panic=abort`, `opt-level=s`, mimalloc |
| Conversation (20 turns) | 10 MB | 40 MB | `VecDeque` capped, auto-summarization |
| HTTP + TLS | 8 MB | 15 MB | Connection pooling, SSE stream parsing |
| TUI frame buffer | 2 MB | 5 MB | ratatui `List` widget, differential render |
| Tool output buffers | 5 MB | 20 MB | Size limits, streaming reads |
| MemCtrl (SQLite) | 1 MB | 3 MB | In-process rusqlite |
| **TOTAL** | **~15 MB** | **~91 MB** | Peak only if all maxed simultaneously |

### Verified RSS (Docker, 2026-05-31)

| Test | RSS |
|------|-----|
| Baseline (`--version`) | 6.0 MB |
| Simple prompt | 12.4 MB |
| Tool execution | 14.2 MB |
| Native memctrl | 14.9 MB |
| Web search | 18.7 MB |
| Multi-tool (3 tools) | 14.5 MB |

## Crate Dependency Graph

```
axga-shared (types, errors, limits)
  ├── axga-ai (LLM providers, SSE streaming)
  │     └── axga-core (agent loop, tool registry, state, retry)
  │           └── axga-cli (binary: TUI, Telegram, single-shot, spawn, MCP)
  ├── axga-tui (ratatui app, theme, markdown)
  │     └── axga-cli
  └── axga-browser (feature-gated: WebBridge + chromiumoxide)
```

## Data Flow

```
User Input (stdin / TUI / Telegram / MCP)
  → axga-cli main.rs (parse args, load config)
  → axga-core::run_turn()
    → axga-ai::providers (HTTP streaming to LLM)
    → axga-core::stream (SSE → StreamEvent parsing)
    → if tool_calls → axga-core::executor (parallel tool execution)
    → axga-core::Conversation (push results, auto-summarize if full)
    → loop until done or max_turns
  → axga-tui::App::render() (scrollbar, markdown, status bar)
  → Response to user
```

## Providers

All providers implement streaming via the shared `SseStream` adapter:

| Provider | API | Base URL Override | Env Var |
|----------|-----|-------------------|---------|
| OpenAI | Chat Completions | `OPENAI_BASE_URL` | `OPENAI_API_KEY` |
| DeepSeek | OpenAI-compatible | Auto `api.deepseek.com` | `DEEPSEEK_API_KEY` |
| Anthropic | Messages API | Fixed | `ANTHROPIC_API_KEY` |

## Tool Registry

10 tools registered at startup. Each implements the `Tool` trait:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, input: Value) -> Pin<Box<dyn Future<Output = AxgaResult<String>> + Send + '_>>;
}
```

Tools are executed in parallel via `tokio::spawn` with a bounded channel.

## Configuration

`~/.config/axga/config.toml` (TOML format, loaded at startup, falls back to CLI args and env vars).

## Deployment

- **Recommended**: Binary + systemd (`scripts/axga-bot.service`)
- **Docker**: 2-stage Alpine build (adds ~200MB overhead, only for 2GB+ VPS)
- **GitHub Actions**: Auto-build on signed tag, uploads to release
