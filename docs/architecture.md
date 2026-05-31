# AXGA Architecture

## Scope

AXGA is a Rust 2024 workspace for a lightweight AI coding agent. It supports single-shot CLI use, an interactive TUI, provider-backed agent turns, local tools, saved config/onboarding, and Telegram bot mode.

The current MSRV is **Rust 1.88**.

## Workspace

```text
axga-shared  -> shared types, errors, and limits
axga-ai      -> provider HTTP/SSE clients
axga-core    -> config, provider registry, agent loop, tools, context budgeting
axga-tui     -> ratatui application state and rendering
axga-cli     -> binary entry for CLI, TUI, Telegram, and spawn mode
```

## Runtime

`axga-cli` builds a small Tokio runtime:

- 2 worker threads
- 4 max blocking threads
- 512 KB stack per worker thread
- `panic=abort` in release builds

This matches the 1GB VPS target better than Tokio defaults, which can spawn more threads and use larger stacks.

## Provider Flow

Provider metadata lives in `axga-core/src/provider_registry.rs`:

- provider name
- API style
- API key env var
- default base URL
- default model
- known model list
- whether an API key is required

CLI, TUI, Telegram, and onboarding all resolve provider config through this registry. OpenAI, DeepSeek, OpenRouter, Groq, and Ollama use the OpenAI-compatible client. Anthropic uses the native Messages client.

## Agent Turn Flow

```text
user input
  -> axga-cli mode handler
  -> axga-core::Conversation
  -> provider registry resolution
  -> axga-ai provider stream
  -> StreamEvent text/tool deltas
  -> tool execution through ToolRegistry
  -> bounded tool output back into Conversation
  -> final assistant text
```

## Telegram Flow

Telegram mode validates the bot token with `getMe`, long-polls `getUpdates`, enforces configured `allowed_users`, and keeps isolated conversation state per chat. Active chat state is capped to keep memory bounded. Replies are sent as plain text and split into chunks below Telegram's message limit.

## Memory Model

Target:

| Metric | Target |
|---|---|
| Typical RSS | <100 MB |
| Peak RSS | <150 MB |
| Release binary | ~5-8 MB target |

Current enforced controls:

| Area | Control |
|---|---|
| Local file reads | metadata size check before reading |
| Fetch URL / search bodies | bounded byte collection |
| LLM responses | streaming SSE parsing |
| Conversation history | bounded turns and summarization |
| Telegram state | capped per-chat conversation map |
| Runtime | 2 workers, 4 blocking threads, 512 KB stacks |
| Release builds | size optimization, LTO, strip, panic abort |

Policy: treat unbounded buffers as bugs unless there is a documented reason. The repo does not claim every allocation is statically bounded; it enforces the highest-risk I/O and conversation paths and keeps remaining exceptions explicit.

## Current Phase

| Phase | Status |
|---|---|
| Foundation | Done |
| LLM streaming | Done |
| Agent runtime and tools | Done |
| TUI | Done |
| Provider registry/config/onboarding | Done |
| Telegram hardening | Done |
| Deployment packaging | Partial |

Useful next hardening work: memory profiling under long Telegram sessions, stress tests for tool-heavy conversations, and install/release validation on a clean VPS image.
