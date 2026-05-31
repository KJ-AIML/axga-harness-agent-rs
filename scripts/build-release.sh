#!/bin/bash
# scripts/build-release.sh
# Build a statically-linked release binary for 1GB VPS deployment.
#
# Requires: rustup target add x86_64-unknown-linux-musl
#
# Output: target/x86_64-unknown-linux-musl/release/axga (~5-8 MB)

set -euo pipefail

TARGET="x86_64-unknown-linux-musl"
BINARY="target/${TARGET}/release/axga"

echo "=== AXGA Release Build ==="
echo "Target: ${TARGET}"
echo

# Ensure musl target is installed
rustup target add "${TARGET}" 2>/dev/null || true

# Build
echo "[1/3] Building..."
cargo build --release --target "${TARGET}"

# Strip (already stripped by Cargo profile, but double-check)
echo "[2/3] Stripping..."
strip "${BINARY}" 2>/dev/null || true

# Report
echo "[3/3] Done."
echo
echo "Binary: ${BINARY}"
ls -lh "${BINARY}"
echo
echo "Dynamic dependencies (should be none for musl):"
ldd "${BINARY}" 2>/dev/null || echo "  (static binary — no dynamic deps)"
echo
echo "=== Build complete ==="
