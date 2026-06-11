# AXGA Project Audit & Enhancement Plan

**Generated**: 2026-06-11 | **Version**: v0.1.0

---

## Executive Summary

Comprehensive audit across 6 crates, 8 scripts, CI/CD, docs, and tooling. Found **3 critical issues**, **10 medium issues**, and **9 low-priority cleanups**. The codebase is structurally sound (zero orphan files, all modules match) but has documentation drift, dead code, and one broken feature (`--dangerous` flag).

---

## 🔴 Critical Issues (Must Fix)

### C1. `--dangerous` CLI flag is dead — never wired to ShellTool
**Files**: `main.rs:87`, `main.rs:226,245`, `tui_mode.rs:25`, `telegram.rs:40`
**Problem**: Flag is parsed but never passed. All 4 call sites hardcode `ShellTool::new(false)`.
**Fix**: Thread `cli.dangerous` into every `ShellTool::new()` call.
```rust
// Before (all 4 sites):
ShellTool::new(false)
// After:
ShellTool::new(cli.dangerous)  // or param
```

### C2. AUR PKGBUILD broken for aarch64 — sha256sums mismatch
**File**: `scripts/aur/PKGBUILD`
**Problem**: `arch=('x86_64' 'aarch64')` but `sha256sums=('c751933392...')` is single-element. aarch64 users get checksum failure.
**Fix**: Use per-arch conditional SHA256 or separate source arrays.

### C3. `scripts/github-release.sh` stale — only 2/6 targets
**File**: `scripts/github-release.sh`
**Problem**: Only builds x86_64 + aarch64 Linux musl. Missing macOS (x2), browser variant, minimal variant. Superseded by CI workflow.
**Fix**: Either delete (CI handles it) or update to match CI matrix.

---

## 🟡 Medium Issues (Should Fix)

### M1. README config shows `[security]` section that doesn't exist in code
**Files**: `README.md:272-275`, `config.rs`
**Problem**: README shows `[security] shell_denylist = [...]` but `Config` struct has no `SecuritySection`. Denylist is hardcoded in `shell.rs`.
**Fix**: Either add `SecuritySection` to `Config` and wire it in, or remove `[security]` from README example.

### M2. README claims 14 slash commands — code has 11
**Files**: `README.md:138`, `tui_mode.rs:127-232`
**Problem**: "14 slash commands" in Features table, but only 11 exist. README's own listing (line 171) shows 11 too — internal inconsistency.
**Fix**: Change README line 138 from "14 slash commands" to "11 slash commands".

### M3. `gg` (scroll to top) vim key claimed but not implemented
**Files**: `README.md:173`, `tui_mode.rs`
**Problem**: README lists `gg` as a vim key. Code has `G` (bottom) and `j`/`k` (scroll) but no `scroll_to_top()` function or `gg` binding.
**Fix**: Implement `gg` in TUI key handler calling a new `scroll_to_top()` method.

### M4. Tool registration duplicated 4 times — no shared helper
**Files**: `main.rs:220-233`, `main.rs:237-251`, `tui_mode.rs:21-31`, `telegram.rs:36-46`
**Problem**: Identical 10-tool registration code in 4 places. DRY violation, future skew risk.
**Fix**: Extract `fn build_registry() -> AxgaResult<ToolRegistry>` in `axga-core`.

### M5. `memctrl.rs` is dead code — declared module, never used
**File**: `crates/axga-core/src/tools/memctrl.rs`
**Problem**: CLI-based memctrl tool still declared in `mod.rs` but superseded by `memctrl_native`. Always compiled.
**Fix**: Delete `memctrl.rs` or feature-gate it behind `memctrl-cli`.

### M6. No per-tool unit tests — only registry-level tests
**Files**: All `crates/axga-core/src/tools/*.rs`
**Problem**: 14 total tests, but zero test individual tool behavior (denylist, file cap, SQLite ops, HTML parsing).
**Fix**: Add unit tests for: shell denylist matching, read_file 1MB cap, memctrl SQL CRUD, web_search parsing.

### M7. `session.rs` violates project memory rule #1
**File**: `crates/axga-core/src/session.rs:24`
**Problem**: `std::fs::read_to_string(path)?` without `metadata().len()` check. Memory rule says ">1MB → reject".
**Fix**: Add `std::fs::metadata(path)?.len()` check before reading.

### M8. Missing DeepSeek provider module — docs misleading
**Files**: `axga-ai` Cargo.toml, `providers/mod.rs`, AGENTS.md
**Problem**: Crate claims "OpenAI, Anthropic, DeepSeek" but only 2 provider files. DeepSeek works via OpenAiProvider alias in agent_loop.rs.
**Fix**: Either add thin `deepseek.rs` wrapper or update docs to say "OpenAI-compatible (including DeepSeek)".

### M9. Missing feature gates — memctrl_native always pulls rusqlite
**File**: `crates/axga-core/Cargo.toml`
**Problem**: `rusqlite` with `bundled` feature (compiles SQLite from C) is always pulled in. No way to opt out for CLI-only memctrl.
**Fix**: Add `[features]` with `memctrl-native = ["rusqlite"]` as default.

### M10. 6 CLI args/subcommands undocumented in README
**Missing from README**: `--base-url`, `--system-prompt`, `--max-turns`, `--dir`, `--verbose`, `--onboard`, `axga models`, `axga config`, `axga mcp`
**Fix**: Add to README first-run docs or create a CLI reference section.

---

## 🟢 Low Priority (Nice to Have)

### L1. `unicode-width = "0.2"` unused in axga-tui
**File**: `crates/axga-tui/Cargo.toml` — remove the dependency.

### L2. `light_theme()` dead code in axga-tui
**File**: `crates/axga-tui/src/theme.rs` — remove or wire up with `--light` flag.

### L3. `events.rs` deprecated but retained
**File**: `crates/axga-tui/src/events.rs` — either delete or refactor as planned.

### L4. Unused workspace deps in axga-tui Cargo.toml
**Deps**: `serde`, `serde_json`, `thiserror`, `tracing`, `base64`, `regex` — not used in TUI source, remove from Cargo.toml.

### L5. `unsafe` block in OpenAiProvider test
**File**: `crates/axga-ai/src/providers/openai.rs:73` — `std::env::remove_var` is not unsafe, remove `unsafe` block.

### L6. Provider interface divergence
**Issue**: `OpenAiProvider::stream_chat` takes `&RequestBuilder`, `AnthropicProvider::stream_chat` takes individual params. No shared trait.
**Fix**: Extract `Provider` trait with unified signature.

### L7. Homebrew formula has 3 TODO SHA256 placeholders
**File**: `scripts/homebrew/axga.rb` — run `sha256-update.sh` at next release.

### L8. `sha256-update.sh` only handles x86_64 for AUR
**File**: `scripts/sha256-update.sh` — add aarch64 SHA256 update support.

### L9. REAMDE memory badge says "19MB" vs PROGRESS says "18.7MB"
**File**: `README.md:17` — minor inconsistency. Pick one.

---

## ✅ Verified Correct (No Action)

| Check | Status |
|-------|--------|
| 14 tests count | ✅ Match |
| 10 runtime tools | ✅ Correct |
| Shell denylist (8 entries + regex) | ✅ Exceeds docs |
| MCP server (JSON-RPC stdio) | ✅ Real implementation |
| install.sh (download + SHA256) | ✅ Correct |
| Dockerfile (2-stage Alpine) | ✅ Correct |
| CI workflow (6 targets) | ✅ Complete |
| KEYS.md (no secrets) | ✅ Clean |
| All `pub mod` → file on disk | ✅ Zero orphans |
| All module declarations | ✅ Zero mismatches |
| Crate dependency graph | ✅ Matches AGENTS.md |

---

## 🔧 Useful Skills for This Work

| Skill | Use |
|-------|-----|
| `code-safety-audit` | Security review of shell denylist, session.rs read_to_string |
| `tdd-coach` | Writing per-tool unit tests (M6) |
| `repository-analyzer` | Full codebase documentation |
| `test-suite-architect` | Systematic test coverage improvement |
| `pipeline-blueprint` | CI workflow improvements |

---

## 📋 Implementation Order (Recommended)

### Phase A: Critical Fixes (1-2 sessions)
1. ✅ Fix `--dangerous` flag (C1) — 4-line change at 4 call sites
2. ✅ Fix AUR PKGBUILD (C2) — dual-arch sha256sums
3. ✅ Delete or update `github-release.sh` (C3)

### Phase B: Doc Sync (1 session)
1. ✅ Fix README `[security]` section (M1)
2. ✅ Fix README "14 slash commands" → 11 (M2)
3. ✅ Document missing CLI args in README (M10)

### Phase C: Code Quality (1-2 sessions)
1. ✅ Implement `gg` vim key (M3)
2. ✅ Extract shared `build_registry()` (M4)
3. ✅ Delete `memctrl.rs` dead code (M5)
4. ✅ Fix `session.rs` memory check (M7)
5. ✅ Clean unused deps (L1-L4)

### Phase D: Tests & Design (2+ sessions)
1. ✅ Add per-tool unit tests (M6)
2. ✅ Add DeepSeek provider module or clarify docs (M8)
3. ✅ Add memctrl feature gates (M9)
4. ✅ Extract `Provider` trait (L6)

### Phase E: Release Readiness (before next tag)
1. ✅ Run `sha256-update.sh` (L7, L8)
2. ✅ Fix Homebrew formula SHA256s
