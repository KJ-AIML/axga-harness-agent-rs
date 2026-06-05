#!/bin/bash
# scripts/github-release.sh
# Build binaries for all targets and create a GitHub release.
#
# Prerequisites:
#   rustup target add x86_64-unknown-linux-musl
#   rustup target add aarch64-unknown-linux-musl
#   rustup target add x86_64-apple-darwin  (macOS only)
#   rustup target add aarch64-apple-darwin (macOS only)
#   gh auth login

set -euo pipefail

VERSION="${1:-v0.1.0}"
REPO="KJ-AIML/axga-harness-agent-rs"

echo "=== Building axga ${VERSION} ==="

TARGETS=(
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
)

for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "--- Building ${TARGET} ---"
    cargo build --release --target "${TARGET}" -p axga-cli

    BINARY="target/${TARGET}/release/axga"
    ARCHIVE="axga-${VERSION}-${TARGET}.tar.gz"

    tar -czf "${ARCHIVE}" -C "target/${TARGET}/release" axga
    echo "  Created: ${ARCHIVE} ($(du -h ${ARCHIVE} | cut -f1))"
done

echo ""
echo "=== Creating GitHub Release ==="
gh release create "${VERSION}" \
    --repo "${REPO}" \
    --title "axga ${VERSION}" \
    --notes "Release ${VERSION}

## Binary sizes
- x86_64-unknown-linux-musl: $(du -h axga-${VERSION}-x86_64-unknown-linux-musl.tar.gz | cut -f1)
- aarch64-unknown-linux-musl: $(du -h axga-${VERSION}-aarch64-unknown-linux-musl.tar.gz | cut -f1)

## Install
\`\`\`sh
curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sh
\`\`\`" \
    axga-*.tar.gz

echo ""
echo "=== Done ==="
echo "Install: curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sh"
