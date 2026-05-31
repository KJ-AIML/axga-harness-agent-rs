#!/bin/bash
# scripts/memory-profile.sh
# Profile RSS during various workloads.
# Requires: /usr/bin/time (GNU time, not shell builtin)

set -euo pipefail

BINARY="./target/release/axga"

echo "=== AXGA Memory Profile ==="
echo

# 1. Baseline
echo "--- Baseline RSS ---"
/usr/bin/time -v "${BINARY}" --version 2>&1 | grep "Maximum resident"

# 2. Single-shot prompt
echo "--- Single-shot RSS ---"
/usr/bin/time -v "${BINARY}" --prompt "hello world" 2>&1 | grep "Maximum resident"

echo
echo "=== Profile complete ==="
echo "Run 'heaptrack ${BINARY}' for detailed allocation tracing."
