# Architecture Decision Records

## ADR-008: Native MemCtrl (SQLite) over Python Subprocess
**Date:** 2026-05-31
**Status:** Accepted

**Decision.** Replace the `memctrl` Python CLI subprocess with native `rusqlite` calls.

**Rationale.** Spawning Python adds 30-50 MB per memory query. On a 1GB VPS, this is unacceptable. SQLite via `rusqlite` with the `bundled` feature adds ~500KB to the binary and zero runtime RAM overhead beyond the query itself. The schema is simple (memories table with id, layer, content, confidence, source, timestamps) and easily maintained in Rust.

**Alternatives.** Keep Python subprocess (rejected — RAM cost). Use JSON file (rejected — no query capability). Use external vector DB (rejected — overkill for <10K memories).

**Consequences.** `rusqlite` adds `libsqlite3-sys` as a build dependency. Binary size increased from 4.7MB to 5.8MB. No Python dependency needed on target VPS.

## ADR-007: Binary Target — musl
*(unchanged from original)*

## ADR-006: Token Counting — Heuristic First
*(unchanged from original)*

## ADR-005: No Local Embedding Model
*(unchanged from original)*

## ADR-004: Async Runtime — tokio (2 threads)
*(unchanged from original)*

## ADR-003: TUI — ratatui
*(unchanged from original)*

## ADR-002: HTTP — reqwest + rustls
*(unchanged from original)*

## ADR-001: Language — Rust
*(unchanged from original)*
