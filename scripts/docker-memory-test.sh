#!/bin/bash
# scripts/docker-memory-test.sh
# Stress-test axga memory usage inside Docker.
#
# Usage: bash scripts/docker-memory-test.sh

set -euo pipefail

IMAGE="axga-memory-test"
CONTAINER="axga-mem-test"

echo "=== Building Docker test image ==="
docker build -t "$IMAGE" -f- . <<'DOCKERFILE'
FROM ubuntu:24.04
RUN apt-get update -qq && apt-get install -y -qq curl build-essential pkg-config libssl-dev musl-tools procps time 2>&1 | tail -2
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.88.0 2>&1 | tail -1
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl -p axga-cli 2>&1 | tail -2
RUN strip target/x86_64-unknown-linux-musl/release/axga
RUN cp target/x86_64-unknown-linux-musl/release/axga /usr/local/bin/axga
DOCKERFILE

echo ""
echo "=== Memory Test Suite ==="
echo ""

# Test 1: Binary size
echo "--- Test 1: Binary size ---"
docker run --rm "$IMAGE" ls -lh /usr/local/bin/axga

# Test 2: Baseline RSS (--help)
echo ""
echo "--- Test 2: Baseline RSS (--help) ---"
docker run --rm "$IMAGE" bash -c '
/usr/bin/time -v axga --help 2>&1 | grep "Maximum resident"
'

# Test 3: Single-shot (simple prompt)
echo ""
echo "--- Test 3: Single-shot RSS (simple prompt) ---"
docker run --rm -e DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-}" "$IMAGE" bash -c '
/usr/bin/time -v axga --provider deepseek --model deepseek-chat --prompt "say hello in 5 words" 2>&1 | grep "Maximum resident"
'

# Test 4: Read a large file (1MB limit test)
echo ""
echo "--- Test 4: Read file tool RSS ---"
docker run --rm -e DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-}" "$IMAGE" bash -c '
# Create a 500KB test file
dd if=/dev/urandom of=/tmp/large.txt bs=1024 count=500 2>/dev/null
/usr/bin/time -v axga --provider deepseek --model deepseek-chat --prompt "use read_file to read /tmp/large.txt and tell me its size in bytes" --max-turns 2 2>&1 | grep "Maximum resident"
'

# Test 5: Shell execution
echo ""
echo "--- Test 5: Shell tool RSS ---"
docker run --rm -e DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-}" "$IMAGE" bash -c '
/usr/bin/time -v axga --provider deepseek --model deepseek-chat --prompt "use execute_shell to run: find /usr -name *.so 2>/dev/null | head -50" --max-turns 2 2>&1 | grep "Maximum resident"
'

# Test 6: Multi-turn conversation (simulate 5 messages)
echo ""
echo "--- Test 6: Multi-turn RSS (empty provider, local only) ---"
docker run --rm "$IMAGE" bash -c '
# Test without API key — just measure binary + TUI memory for a simulated session
/usr/bin/time -v timeout 3 axga 2>&1 | grep "Maximum resident" || echo "RSS: N/A (timed out as expected)"
'

echo ""
echo "=== All tests complete ==="
