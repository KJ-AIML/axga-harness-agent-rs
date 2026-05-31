# Architecture Decision Records

Recorded decisions for the `axga-harness-agent-rs` project.
Format: date, decision, rationale, alternatives considered, consequences.

## ADR-001: Language — Rust
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Write the axga agent harness in Rust.

**Rationale.** The primary constraint is sub-100MB RAM on a 1GB VPS. Node.js garbage collection makes memory usage unpredictable; a large conversation history can spike RSS 200MB+. Rust gives deterministic control over allocations, stack size, and buffer limits. The `mem::size_of` and `valgrind/massif` tooling enable precise memory budgeting at every allocation site.

**Alternatives.** Go (rejected — GC unpredictability, no zero-cost abstractions for memory limits). Zig (rejected — smaller ecosystem for TUI/HTTP). Bun + careful coding (rejected — OS signals and native modules are still GC-bound).

**Consequences.** Must maintain a separate codebase from the TypeScript fork. Rust learning curve for contributors. Compile times are slower. Binary distribution is simpler (single static binary).

## ADR-002: HTTP — reqwest + rustls
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Use `reqwest` with `rustls` (not `native-tls` / OpenSSL).

**Rationale.** OpenSSL requires a system library (`libssl-dev`) that may be missing or outdated on VPS images. `rustls` is pure Rust, statically linked, and eliminates a runtime dependency. `reqwest` is the most mature Rust HTTP client and handles connection pooling, streaming, and TLS natively.

**Alternatives.** `ureq` (rejected — sync-only, no streaming). `hyper` directly (rejected — more boilerplate, `reqwest` is hyper + convenience). `native-tls` (rejected — OpenSSL dependency).

## ADR-003: TUI — ratatui
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Use `ratatui` + `crossterm` for the terminal UI.

**Rationale.** `ratatui` is the most mature Rust TUI framework with built-in differential rendering, layout system, and style support. `crossterm` handles cross-platform terminal I/O. Together they replace the custom differential renderer in the TS `@axga/tui` package.

**Alternatives.** `tui` (rejected — unmaintained, ratatui is the community fork). `termion` (rejected — Linux-only). Custom ANSI (rejected — unnecessary work).

## ADR-004: Async Runtime — tokio (2 threads)
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Use `tokio` multi-thread runtime with exactly 2 worker threads.

**Rationale.** The target VPS has 1–2 vCPUs. The default tokio thread pool (`num_cpus`) would spawn unnecessary threads that each consume 2MB+ stack space. 2 threads gives enough parallelism for concurrent tool execution + TUI event loop without wasting memory.

**Consequences.** `tokio::spawn` must be used sparingly. Long CPU-bound work should be offloaded to `spawn_blocking` (max 4 blocking threads).

## ADR-005: No Local Embedding Model
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Do not bundle or run a local embedding model (e.g., `ollama`, `sentence-transformers`).

**Rationale.** Running any local model requires 500MB–2GB RAM. The 1GB VPS cannot support this alongside the agent runtime. All LLM calls go to remote APIs (OpenAI, Anthropic). If local inference becomes a requirement, it must run on a separate machine.

## ADR-006: Token Counting — Heuristic First
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Use the `4 chars ≈ 1 token` heuristic initially. Add `tiktoken-rs` only if accuracy becomes critical.

**Rationale.** The heuristic is 90% accurate for English text and requires zero dependencies. `tiktoken-rs` adds ~50MB of tokenizer data and a native build dependency. The heuristic is sufficient for the memory budget's token cap (32K tokens). Can be swapped for exact counting later without changing the public API.

**Consequences.** Token estimates may be off by 10–20% for non-English text. The 32K token cap should be set conservatively (e.g., 28K) to account for estimation error.

## ADR-007: Binary Target — musl
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Release builds target `x86_64-unknown-linux-musl` with static linking.

**Rationale.** `musl` statically links libc — no glibc version dependency on the VPS. Binary is ~5–8MB with `opt-level=s`, LTO, and strip. Users can download and run without installing Rust or any system libraries.

**Alternatives.** `x86_64-unknown-linux-gnu` (rejected — glibc version hell). AppImage/Snap (rejected — overkill for a CLI tool).
