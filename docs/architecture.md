# AXGA Architecture

## Memory Model

The primary design constraint is **sub-100MB RAM on a 1GB VPS**. Every allocation is budgeted.

### Budget

| Component | Typical | Peak | Strategy |
|-----------|---------|------|----------|
| Binary + tokio | 5 MB | 8 MB | `panic=abort`, `opt-level=s` |
| Conversation history | 10 MB | 40 MB | `VecDeque` capped at 20 turns, summarization |
| HTTP + TLS | 8 MB | 15 MB | Connection pooling, stream parsing |
| TUI frame buffer | 2 MB | 5 MB | ratatui differential render |
| LLM context | 20 MB | 60 MB | Token cap, streaming |
| File I/O buffers | 5 MB | 20 MB | Size limits, streaming reads |
| **TOTAL** | **~50 MB** | **~148 MB** | Peak only if all maxed simultaneously |

### Rules (enforced in code)
1. No `read_to_string` on unknown files — check `metadata().len()` first
2. No unbounded `Vec` — use `with_capacity` or bounded channels
3. No buffering full HTTP responses — parse SSE chunks as `&str`
4. Conversation history automatically summarizes when full

## Crate Dependency Graph

```
axga-shared (types, errors, limits)
     │
     ├── axga-ai (LLM provider abstraction)
     │       └── axga-core (agent runtime)
     │               └── axga-cli (binary entry)
     │
     └── axga-tui (terminal UI)
             └── axga-cli
```

## Data Flow

```
User Input (stdin / TUI)
    → axga-cli main.rs
    → axga-core::Conversation.push(user_message)
    → axga-core::context::estimate_conversation_tokens(history)
    → if budget exceeded → summarize oldest turns
    → axga-ai::providers::openai::stream(context, messages)
    → SSE bytes → StreamEvent (text, tool_call, thinking, usage)
    → if tool_call → axga-core::executor::execute_tool_calls(registry, calls)
    → axga-core::Conversation.push(assistant_message + tool_results)
    → axga-tui::App::render() updates UI
    → Loop
```
