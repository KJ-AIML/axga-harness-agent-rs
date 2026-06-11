# AXGA-rs — Comprehensive Architecture & Roadmap

**Generated**: 2026-06-11 | **Based on**: Deep analysis of kimi-code (TypeScript) vs axga-rs (Rust)

---

## Executive Summary

axga-rs is a **5.8MB Rust AI coding agent** that runs on a 1GB VPS. It has 6 crates, 3 LLM providers, 10 tools, TUI, Telegram bot, and MCP server. Compared to kimi-code (TypeScript, ~300MB RAM), axga-rs is vastly more efficient but lacks polish in user experience, safety, and tool depth.

This document captures the full comparison and a prioritized roadmap to close the gap while keeping axga-rs's efficiency advantage.

---

## 1. Architecture Comparison

### axga-rs (6 crates, Rust)
```
axga-shared (types, errors, limits)   — 3 files
axga-ai (3 providers, SSE)            — 6 files
axga-core (agent loop, tools, state)  — 17 files
axga-tui (ratatui, markdown)          — 4 files
axga-cli (binary, TUI, Telegram, MCP) — 5 files
axga-browser (chromiumoxide stubs)    — 3 files

Total: ~38 source files
```

### kimi-code (11 packages, TypeScript/pnpm)
```
agent-core (engine, 560-line Agent class, 20+ managers)
kosong (LLM abstraction, 6 providers)
kaos (OS abstraction)
node-sdk (public harness)
kimi-code (TUI/CLI, component-based)
protocol, oauth, telemetry, acp-adapter, migration-legacy

Total: ~200+ source files
```

---

## 2. Feature-by-Feature Comparison

### 2.1 User Experience

| Feature | kimi-code | axga-rs | Status |
|---------|-----------|---------|--------|
| Real-time streaming | SSE → live text + tool calls | ✅ Implemented 2026-06-11 | ✅ |
| Permission/approval | 18-policy chain with UI dialogs | ✅ Basic system, dialogs TODO | ⬜ |
| Model picker | Tabbed per-provider with fuzzy search | Manual text input | ⬜ |
| Provider wizard | Interactive API key → model → catalog | ✅ /provider guided flow | ✅ |
| Two-line footer | git, CWD, context%, permissions, tips | Single line: model/tokens/mode | ⬜ |
| Slash commands | 42 declarative with autocomplete | ~15 hardcoded in if-else chain | ⬜ |
| Input autocomplete | File @mentions + command completion | None | ⬜ |
| Input history | Persisted per-workdir to disk | None | ⬜ |
| Theme | Dark/light/auto with runtime switch | Dark only, static | ⬜ |
| Syntax highlighting | 30+ languages via cli-highlight | Basic markdown only | ⬜ |
| Diff preview | Line-by-line LCS with intra-line bold | None | ⬜ |
| Welcome panel | Logo + contextual info in rounded box | 3 hardcoded lines | ⬜ |
| Rotating toolbar tips | Weighted priority round-robin | None | ⬜ |

### 2.2 Agent Engine

| Feature | kimi-code | axga-rs | Status |
|---------|-----------|---------|--------|
| Turn loop | Event-driven with 8 hook points | Flat imperative loop | ⬜ |
| Context compaction | LLM-powered with retries, block ratio | 500-char naive string summarization | ⬜ |
| Micro compaction | Old tool result truncation | None | ⬜ |
| Session persistence | Event-sourced JSONL with replay | JSONL message dump, no metadata | ⬜ |
| Session fork/export | Fork + debug-zip with manifest | None | ⬜ |
| Session resume | Full state replay from wire records | Parse JSONL, messages only | ⬜ |
| Goal/autonomous mode | Objective + budget + continuation | None | ⬜ |
| Plan mode | Read-only → plan file → review | None | ⬜ |
| Injection system | Dynamic per-step context injection | None | ⬜ |
| Profiles | 4 built-in: agent, coder, explorer, plan | None | ⬜ |
| Undo | Rollback last N prompts | None | ⬜ |

### 2.3 Tools

| Tool | kimi-code | axga-rs | Status |
|------|-----------|---------|--------|
| Read | Line offset, tail, line-ending detection | ✅ read_file | ✅ |
| Write | Create/overwrite | ✅ write_file | ✅ |
| Edit | Exact string replace (core editing tool) | ❌ | ⬜ |
| Glob | Pattern matching | ✅ | ✅ |
| Grep | Content search (rg-compatible) | ✅ | ✅ |
| Bash | Foreground/background, SIGTERM→SIGKILL | ✅ execute_shell | ✅ |
| WebSearch | External search | ✅ | ✅ |
| FetchURL | HTTP GET | ✅ | ✅ |
| ReadMediaFile | Image/video/audio | ❌ | ⬜ |
| EnterPlanMode | Plan mode entry | ❌ | ⬜ |
| ExitPlanMode | Plan submission | ❌ | ⬜ |
| CreateGoal | Autonomous goal start | ❌ | ⬜ |
| GetGoal | Goal status check | ❌ | ⬜ |
| UpdateGoal | Goal lifecycle | ❌ | ⬜ |
| TaskList | Background task enumeration | ❌ | ⬜ |
| TaskOutput | Task output streaming | ❌ | ⬜ |
| TaskStop | Task cancellation | ❌ | ⬜ |
| CronCreate/List/Delete | Scheduled tasks | ❌ | ⬜ |
| Agent | Subagent spawn/resume | ❌ (Partial: Orchestrator) | ⬜ |
| AgentSwarm | Parallel multi-agent | ❌ (Partial: Orchestrator) | ⬜ |
| Skill | Skill invocation | ❌ | ⬜ |
| TodoList | Persistent task tracking | ❌ | ⬜ |
| AskUserQuestion | Structured user prompts | ❌ | ⬜ |

### 2.4 Safety

| Feature | kimi-code | axga-rs | Status |
|---------|-----------|---------|--------|
| Permission system | 18-policy chain | ✅ Basic implementation | ✅ |
| Sensitive file blocking | .env, id_rsa, credentials, .aws/ | ❌ | ⬜ |
| Path security | Enforced within workspace | ❌ | ⬜ |
| Shell safety | SIGTERM→grace→SIGKILL, env hardening | String-match denylist | ⬜ |
| Tool dedup | Same-step + cross-step, streak detection | ❌ | ⬜ |
| Output limits | 50K char cap, 2K char/line, streaming | Simple truncation | ⬜ |
| JSON Schema validation | Ajv strict mode | ❌ | ⬜ |
| Conflict-aware execution | ToolAccesses-based file conflict resolution | ❌ | ⬜ |

### 2.5 Provider System

| Feature | kimi-code | axga-rs | Status |
|---------|-----------|---------|--------|
| Providers | 6 (Kimi, OpenAI, Anthropic, Google, VertexAI, OpenAI Responses) | 3 (OpenAI, Anthropic, DeepSeek) | ✅ |
| Provider trait | ChatProvider with 8 methods | Provider with 1 method (stream_chat) | ✅ |
| Model aliases | Separate config layer, capabilities per model | Flat string, no validation | ⬜ |
| Thinking effort | off/low/medium/high/xhigh/max, per-request | Not supported | ⬜ |
| Finish reason normalization | Unified enum across providers | Raw provider strings | ⬜ |
| OAuth support | Full OAuth with token refresh | Static API keys only | ⬜ |
| Provider catalog | models.dev API fetch, auto-populate | Manual model name entry | ⬜ |
| Env-model mode | Zero-config via KIMI_MODEL_* env vars | Config file required | ⬜ |

---

## 3. axga-rs's Advantages (Keep These!)

| Advantage | Detail |
|-----------|--------|
| **Binary size** | 5.8MB (musl, static, stripped) — kimi-code needs Node.js runtime |
| **Memory** | 19MB peak RSS — kimi-code uses 300MB+ |
| **Startup** | Instant — no JIT, no module loading |
| **Deployment** | Single binary, no dependencies, runs on $5 VPS |
| **Static build** | musl target, zero glibc dependency |
| **Tool trait** | Clean, typed, composable — better than JS duck-typing |
| **Compiler safety** | Rust's type system catches errors at compile time |
| **MCP server** | JSON-RPC stdio — kimi-code doesn't expose this externally |

---

## 4. Prioritized Roadmap

### Phase A: Core UX (P0) — Current Sprint ✅
- [x] Real-time SSE streaming to TUI
- [x] Basic permission system (Manual/Auto modes)
- [x] Interactive provider setup wizard (/provider → key → model)
- [x] /apikey persistence to config

### Phase B: Safety & Tools (P1)
- [ ] TUI approval dialog (ask user before running shell/write tools)
- [ ] Edit tool (exact string replacement, the core coding tool)
- [ ] Sensitive file blocking (.env, SSH keys, credentials)
- [ ] Tool dedup (same-step + cross-step streak detection)
- [ ] Output streaming (bash output streams to TUI instead of buffering)

### Phase C: TUI Polish (P1)
- [ ] Two-line footer (context%, git branch, CWD, permission badge, tips)
- [ ] Syntax-highlighted code blocks (syntect for Rust)
- [ ] Tabbed model selector (/model with fuzzy search)
- [ ] Diff preview (line-by-line with intra-line bold)
- [ ] Input autocomplete (slash commands, file @mentions)
- [ ] Input history (persisted per-workdir)
- [ ] Welcome panel (logo + contextual info)

### Phase D: Agent Engine (P1-P2)
- [ ] LLM-powered context compaction (auto-trigger, retry, blocking)
- [ ] Event-sourced session persistence (replay, resume, fork, export)
- [ ] Undo support (rollback last N prompts)
- [ ] Dynamic system prompts with profiles (agent, coder, explorer, plan)

### Phase E: Multi-Agent (P2)
- [ ] Agent tool (subagent spawn/resume, foreground/background)
- [ ] AgentSwarm tool (parallel multi-agent with DAG)
- [ ] Subagent lifecycle (resource budgets, communication protocol)

### Phase F: Advanced Features (P2-P3)
- [x] Goal/autonomous mode (objective + budget + self-driving)
- [ ] Plan mode (read-only → plan review → execute)
- [ ] Background task system (TaskList/TaskOutput/TaskStop)
- [ ] Cron scheduler (CronCreate/CronList/CronDelete)
- [ ] Skills ecosystem (Skills registry, invocation, scoping)
- [ ] MCP integration as tool source (connect to MCP servers)
- [x] AskUserQuestion tool (structured user prompts)
- [ ] ReadMediaFile tool (image/video/audio reading)

---

## 5. Current File Structure

```
D:/KJ/repos/axga-harness-agent-rs/
├── Cargo.toml                         # Workspace root
├── Cargo.lock
├── AGENTS.md                          # Agent instructions
├── README.md                          # Public docs
├── PROGRESS.md                        # Session handoff log
├── CHANGELOG.md
├── PLAN.md                            # Audit & enhancement plan
├── ROADMAP.md                         # This file
├── KEYS.md                            # GPG signing guide
├── install.sh                         # One-line installer
├── Dockerfile                         # Production Docker build
├── Dockerfile.test                    # Minimal test Docker image
├── rust-toolchain.toml               # Rust 1.88 MSRV
├── .cargo/config.toml                 # musl build config
├── .github/workflows/release.yml      # CI/CD: 6-target build
├── docs/
│   ├── architecture.md                # Memory model + data flow
│   └── adr/                           # Architectural decisions
├── scripts/
│   ├── aur/PKGBUILD                   # Arch Linux package
│   ├── homebrew/axga.rb               # Homebrew formula
│   ├── axga-bot.service               # systemd service
│   ├── build-release.sh               # Local release build
│   ├── sha256-update.sh               # SHA256 hash updater
│   ├── docker-memory-test.sh          # Memory benchmark
│   └── memory-profile.sh              # Quick memory check
├── crates/
│   ├── axga-shared/
│   │   └── src/
│   │       ├── lib.rs                 # Crate root + limits
│   │       ├── error.rs               # AxgaError (13 variants)
│   │       └── types.rs               # AgentMessage, StreamEvent, SubAgentConfig
│   ├── axga-ai/
│   │   └── src/
│   │       ├── lib.rs                 # Crate root
│   │       ├── request.rs             # RequestBuilder
│   │       ├── stream.rs              # SseStream, parse_sse_line
│   │       └── providers/
│   │           ├── mod.rs             # Provider trait
│   │           ├── openai.rs          # OpenAI + OpenAI-compatible
│   │           ├── deepseek.rs        # DeepSeek (wraps OpenAI)
│   │           └── anthropic.rs       # Anthropic (Messages API)
│   ├── axga-core/
│   │   └── src/
│   │       ├── lib.rs                 # Crate root
│   │       ├── state.rs               # Conversation (VecDeque, summarization)
│   │       ├── context.rs             # Token estimation
│   │       ├── agent_loop.rs          # run_turn(), run_turn_streaming(), StreamHandler
│   │       ├── executor.rs            # Tool call execution + parallel dispatch
│   │       ├── session.rs             # Session save/load (JSONL)
│   │       ├── retry.rs               # Exponential backoff
│   │       ├── config.rs              # Config loading/saving (TOML)
│   │       ├── permission.rs          # PermissionManager (Manual/Auto modes)
│   │       ├── orchestrator.rs        # Multi-agent orchestrator
│   │       └── tools/
│   │           ├── mod.rs             # Tool trait
│   │           ├── registry.rs        # ToolRegistry (HashMap)
│   │           ├── fs.rs              # ReadFile, WriteFile, ListDirectory
│   │           ├── shell.rs           # ShellTool (with denylist)
│   │           ├── code.rs            # GrepTool, GlobTool, DiffTool
│   │           ├── memctrl_native.rs  # MemCtrlTool (SQLite)
│   │           ├── web_search.rs      # DuckDuckGo search
│   │           └── fetch_url.rs       # HTTP GET + HTML extraction
│   ├── axga-tui/
│   │   └── src/
│   │       ├── lib.rs                 # Crate root
│   │       ├── app.rs                 # App state, ChatLine, PendingPrompt
│   │       ├── theme.rs               # Color tokens (dark mode)
│   │       └── markdown.rs            # Markdown → Span rendering
│   ├── axga-cli/
│   │   └── src/
│   │       ├── main.rs                # CLI entry, subcommands, telemetry
│   │       ├── tui_mode.rs            # TUI loop, slash commands, streaming
│   │       ├── telegram.rs            # Telegram bot (long-polling)
│   │       └── mcp.rs                 # MCP server (JSON-RPC stdio)
│   └── axga-browser/
│       └── src/
│           ├── lib.rs                 # BrowserBackend trait
│           ├── headless.rs            # chromiumoxide backend (full impl)
│           └── webbridge.rs           # WebBridge client (localhost:10086)
```

---

## 6. Key Design Decisions

### Keep: Simple Tool trait
The `Tool` trait (name, description, parameters, execute) is clean. Don't over-engineer it.

### Keep: Static binary
Musl target, LTO, stripped, panic=abort. This is axga-rs's superpower.

### Adopt: Slash command registry
Replace the inline if-else with a declarative registry like kimi-code's `BUILTIN_SLASH_COMMANDS`.

### Adopt: Event-sourced persistence
Replace JSONL message dump with typed discriminated union records for replayable sessions.

### Adopt: Permission approval dialogs
Add TUI approval panel for write/shell tool confirmation (like kimi-code's 18-policy chain).

### Consider: Component-based TUI
The 545-line `tui_loop()` is approaching unmaintainable. Extract controllers for streaming, commands, editor, etc.

---

## 7. Key Metrics

| Metric | axga-rs | kimi-code |
|--------|---------|-----------|
| Binary size | 5.8 MB | N/A (Node.js) |
| Peak RAM | 19 MB | 300-600 MB |
| Startup time | <100ms | 2-5s |
| Build time | ~90s (release) | ~30s (tsc/bundle) |
| Tests | 53 (0 failures) | Unknown |
| Source files | ~38 | ~200+ |
| Slash commands | 15 | 42 |
| Builtin tools | 10 | 26 |
| Providers | 3 | 6 |
| TUI framework | ratatui | Custom pi-tui |
| CLI framework | clap | Commander.js |
