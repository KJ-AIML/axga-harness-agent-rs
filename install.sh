#!/bin/sh
# install.sh — one-liner installer for axga
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sh
#
# Or with a specific version:
#   curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/install.sh | sh -s -- --version v0.1.0

set -e

REPO="KJ-AIML/axga-harness-agent-rs"
DEFAULT_VERSION="v0.1.1"
INSTALL_DIR="${AXGA_INSTALL_DIR:-/usr/local/bin}"
BINARY_NAME="axga"

# ── Colors ──
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Parse args ──
VERSION="$DEFAULT_VERSION"
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            if [ $# -lt 2 ]; then
                echo "${RED}--version requires a value${NC}"
                exit 1
            fi
            VERSION="$2"; shift 2 ;;
        --dir)
            if [ $# -lt 2 ]; then
                echo "${RED}--dir requires a value${NC}"
                exit 1
            fi
            INSTALL_DIR="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: install.sh [--version VERSION] [--dir INSTALL_DIR]"
            echo "  --version   Version to install (default: ${DEFAULT_VERSION})"
            echo "  --dir       Installation directory (default: /usr/local/bin)"
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Detect platform ──
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux)  PLATFORM="unknown-linux-musl" ;;
    Darwin) PLATFORM="apple-darwin" ;;
    *)      echo "${RED}Unsupported OS: $OS${NC}"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH_TARGET="x86_64" ;;
    aarch64|arm64) ARCH_TARGET="aarch64" ;;
    *) echo "${RED}Unsupported arch: $ARCH${NC}"; exit 1 ;;
esac

TARGET="${ARCH_TARGET}-${PLATFORM}"
ARCHIVE="axga-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

echo "${CYAN}axga installer${NC}"
echo "  Platform: ${TARGET}"
echo "  Version:  ${VERSION}"
echo "  Install:  ${INSTALL_DIR}/${BINARY_NAME}"
echo ""

# ── Download ──
echo "Downloading ${DOWNLOAD_URL}..."
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/$ARCHIVE"
else
    echo "${RED}Need curl or wget to download.${NC}"
    exit 1
fi

# ── Checksum verification (on archive, before extraction) ──
verify_checksum() {
    CHECKSUM_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}.sha256"
    # The CI publishes .tar.gz.sha256 files containing the SHA256 of the archive,
    # not the binary inside — so we hash the archive file for comparison.
    if command -v curl >/dev/null 2>&1; then
        EXPECTED=$(curl -fsSL "$CHECKSUM_URL" 2>/dev/null | awk '{print $1}')
    elif command -v wget >/dev/null 2>&1; then
        EXPECTED=$(wget -qO- "$CHECKSUM_URL" 2>/dev/null | awk '{print $1}')
    fi
    if [ -z "$EXPECTED" ]; then
        echo "  (no checksum file found; skipping verification)"
        return 0
    fi
    ACTUAL=$(sha256sum "$TMP_DIR/$ARCHIVE" 2>/dev/null | awk '{print $1}' || shasum -a 256 "$TMP_DIR/$ARCHIVE" 2>/dev/null | awk '{print $1}')
    if [ -z "$ACTUAL" ]; then
        echo "  (no sha256sum/shasum available; skipping verification)"
        return 0
    fi
    if [ "$EXPECTED" = "$ACTUAL" ]; then
        echo "${GREEN}Checksum verified.${NC}"
    else
        echo "${RED}Checksum mismatch! Archive may be corrupted or tampered with.${NC}"
        echo "  Expected: $EXPECTED"
        echo "  Got:      $ACTUAL"
        exit 1
    fi
}
verify_checksum

# ── Extract ──
if ! tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR" 2>/dev/null; then
    echo "${RED}Failed to extract archive. It may be corrupted or in an unexpected format.${NC}"
    exit 1
fi

# Find the binary — it may be at the archive root or one directory deep
BINARY_PATH=$(find "$TMP_DIR" -name "$BINARY_NAME" -type f 2>/dev/null | head -1)
if [ -z "$BINARY_PATH" ]; then
    echo "${RED}Binary '$BINARY_NAME' not found in archive.${NC}"
    exit 1
fi

# ── Install ──
if [ ! -d "$INSTALL_DIR" ]; then
    mkdir -p "$INSTALL_DIR"
fi

install -m 755 "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"

echo ""
echo "${GREEN}axga ${VERSION} installed to ${INSTALL_DIR}/${BINARY_NAME}${NC}"

echo ""
echo "Run: ${CYAN}axga --help${NC}"
echo ""

# ── Verify ──
if command -v axga >/dev/null 2>&1; then
    axga --version 2>/dev/null || true
else
    echo "${RED}Warning: ${INSTALL_DIR} may not be in your PATH.${NC}"
    echo "Add it:  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
