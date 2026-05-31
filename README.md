# AXGA Harness Agent (Rust)

Rust port of the `axga-harness-agent` (Pi fork) targeting **sub-100MB RAM on 1GB VPS**.

## Architecture

5-crate Cargo workspace following ADR-001 through ADR-007:

```
axga-shared → axga-ai → axga-core → axga-cli
            ↘ axga-tui ↗
```

See [`docs/architecture.md`](docs/architecture.md) for the memory model and data flow.
See [`docs/adr/README.md`](docs/adr/README.md) for all Architecture Decision Records.

## Building

```bash
# Prerequisites: Rust 1.85+
cargo build --release
```

For VPS deployment:
```bash
rustup target add x86_64-unknown-linux-musl
scripts/build-release.sh
# Output: target/x86_64-unknown-linux-musl/release/axga (~5-8 MB)
```

## Running

```bash
# Interactive TUI (Phase 3)
cargo run -p axga-cli

# Single-shot prompt (Phase 1)
cargo run -p axga-cli -- --prompt "explain Rust ownership"

# Diagnostics
cargo run -p axga-cli -- doctor
```

## Memory Profiling

```bash
scripts/memory-profile.sh
# Requires: GNU time (/usr/bin/time) or heaptrack
```

## Testing

```bash
cargo test --all                        # Unit + integration
cargo clippy -- -D warnings             # Lint
cargo fmt --check                       # Format
```

## Key Constraints

| Constraint | Value | Source |
|-----------|-------|--------|
| Max file read size | 1 MB | `axga_shared::limits::MAX_FILE_READ_SIZE` |
| Max conversation turns | 20 | `axga_shared::limits::MAX_CONVERSATION_TURNS` |
| Max context tokens | 32,768 | `axga_shared::limits::MAX_CONTEXT_TOKENS` |
| Max tool output length | 50,000 chars | `axga_shared::limits::MAX_TOOL_OUTPUT_LEN` |
| Tool channel capacity | 100 | `axga_shared::limits::TOOL_CHANNEL_CAP` |
| Tokio worker threads | 2 | ADR-004 |
| Tokio blocking threads | 4 | ADR-004 |
| Thread stack size | 2 MB | `axga-cli::runtime::build_runtime()` |

## License

MIT
