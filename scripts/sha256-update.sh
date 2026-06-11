#!/bin/bash
# scripts/sha256-update.sh
# Run after building all release archives to auto-fill SHA256 hashes
# in the Homebrew formula and AUR PKGBUILD.
#
# Usage:
#   ./scripts/sha256-update.sh v0.1.0
#
# Prerequisites: all axga-v<VERSION>-*.tar.gz archives must exist in CWD.

set -euo pipefail

VERSION="${1:?Usage: $0 <version-tag, e.g. v0.1.0>}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Updating SHA256 hashes for axga ${VERSION} ==="

# ── Homebrew Formula ──────────────────────────────────────────────
BREW_FILE="${REPO_ROOT}/scripts/homebrew/axga.rb"

update_brew_sha() {
    local arch_label="$1"   # e.g. "x86_64-unknown-linux-musl"
    local archive="axga-${VERSION}-${arch_label}.tar.gz"

    if [ ! -f "$archive" ]; then
        echo "  WARNING: ${archive} not found — skipping"
        return
    fi

    local sha=$(sha256sum "$archive" | awk '{print $1}')
    echo "  ${arch_label}: ${sha}"

    # Replace the TODO line following the matching URL line
    sed -i "/axga-v.*-${arch_label}\.tar\.gz\"/{
        n
        s/sha256 \".*\"/sha256 \"${sha}\"/
    }" "$BREW_FILE"
}

echo ""
echo "Homebrew formula:"
update_brew_sha "x86_64-unknown-linux-musl"
update_brew_sha "aarch64-unknown-linux-musl"
update_brew_sha "x86_64-apple-darwin"
update_brew_sha "aarch64-apple-darwin"

# ── AUR PKGBUILD ──────────────────────────────────────────────────
PKGBUILD_FILE="${REPO_ROOT}/scripts/aur/PKGBUILD"

# The PKGBUILD uses a _sha256() function that maps $CARCH to the correct hash.
# We update the hash inside that function for each architecture.

echo ""
echo "AUR PKGBUILD:"
for arch_label in "x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl"; do
    ARCHIVE="axga-${VERSION}-${arch_label}.tar.gz"
    if [ -f "$ARCHIVE" ]; then
        SHA=$(sha256sum "$ARCHIVE" | awk '{print $1}')
        echo "  ${arch_label}: ${SHA}"
    else
        echo "  WARNING: ${ARCHIVE} not found — skipping"
        continue
    fi

    case "${arch_label}" in
        x86_64*)
            sed -i "/x86_64)  echo /s/\\(echo \"\\).*\\(\"\\)/\\1${SHA}\\2/" "$PKGBUILD_FILE"
            ;;
        aarch64*)
            sed -i "/aarch64) echo /s/\\(echo \"\\).*\\(\"\\)/\\1${SHA}\\2/" "$PKGBUILD_FILE"
            ;;
    esac
done

echo ""
echo "=== Done ==="
echo "Review changes:"
echo "  git diff scripts/homebrew/axga.rb scripts/aur/PKGBUILD"
