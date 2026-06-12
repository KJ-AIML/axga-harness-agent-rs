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
DEFAULT_VERSION="latest"
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
            echo "  --version   Version to install (default: latest GitHub release)"
            echo "  --dir       Installation directory (default: /usr/local/bin)"
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

resolve_version() {
    if [ "$VERSION" != "latest" ]; then
        case "$VERSION" in
            v*) ;;
            *) echo "${RED}Version must start with v, for example v0.1.0.${NC}"; exit 1 ;;
        esac
        return
    fi

    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
            | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
            | head -1)
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
            | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
            | head -1)
    fi

    if [ -z "$VERSION" ]; then
        echo "${RED}Could not resolve latest release for ${REPO}.${NC}"
        exit 1
    fi
}

download_asset() {
    url="$1"
    dest="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$dest"
    else
        echo "${RED}Need curl or wget to download.${NC}"
        return 1
    fi
}

resolve_version

# ── Detect platform ──
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux)  PLATFORM="linux" ;;
    Darwin) PLATFORM="apple-darwin" ;;
    *)      echo "${RED}Unsupported OS: $OS${NC}"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH_TARGET="x86_64" ;;
    aarch64|arm64) ARCH_TARGET="aarch64" ;;
    *) echo "${RED}Unsupported arch: $ARCH${NC}"; exit 1 ;;
esac

TARGET="${ARCH_TARGET}-${PLATFORM}-musl"

# Build up a list of fallback targets to try if the primary asset is not found.
FALLBACKS=""
if [ "$PLATFORM" = "linux" ]; then
    FALLBACKS="${ARCH_TARGET}-unknown-linux-musl ${ARCH_TARGET}-linux-gnu"
fi
ARCHIVE="axga-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}.sha256"

echo "${CYAN}axga installer${NC}"
echo "  Platform: ${TARGET}"
echo "  Version:  ${VERSION}"
echo "  Install:  ${INSTALL_DIR}/${BINARY_NAME}"
echo ""

# ── Download ──
echo "Downloading ${DOWNLOAD_URL}..."
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

if ! download_asset "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE"; then
    FOUND=0
    for fb in $FALLBACKS; do
        FB_ARCHIVE="axga-${VERSION}-${fb}.tar.gz"
        FB_URL="https://github.com/${REPO}/releases/download/${VERSION}/${FB_ARCHIVE}"
        FB_CHECKSUM_URL="https://github.com/${REPO}/releases/download/${VERSION}/${FB_ARCHIVE}.sha256"

        echo "  Primary asset not found; trying ${fb}..."
        if download_asset "$FB_URL" "$TMP_DIR/$FB_ARCHIVE"; then
            ARCHIVE="$FB_ARCHIVE"
            DOWNLOAD_URL="$FB_URL"
            CHECKSUM_URL="$FB_CHECKSUM_URL"
            FOUND=1
            break
        fi
    done
    if [ "$FOUND" -eq 0 ]; then
        echo "${RED}Failed to download any asset for ${TARGET}. Tried: $TARGET $FALLBACKS${NC}"
        exit 1
    fi
fi

# ── Checksum verification (on archive, before extraction) ──
verify_checksum() {
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
