#!/bin/sh
set -euo pipefail

REPO="uinafdev/tccutil"
BINARY_NAME="tccutil-rs"
INSTALL_DIR="/usr/local/bin"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
NC='\033[0m'

error() { printf "${RED}error:${NC} %s\n" "$1" >&2; exit 1; }
info() { printf "${BOLD}%s${NC}\n" "$1"; }

# Check macOS
OS="$(uname -s)"
if [ "$OS" != "Darwin" ]; then
    error "tccutil-rs is macOS-only (TCC is a macOS subsystem).
  Detected OS: $OS
  This tool manages macOS privacy permissions and has no use on other platforms."
fi

# Check curl
if ! command -v curl >/dev/null 2>&1; then
    error "curl is required but not found. Install it with: brew install curl"
fi

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64) ASSET_SUFFIX="aarch64-apple-darwin" ;;
    x86_64)        ASSET_SUFFIX="x86_64-apple-darwin" ;;
    *)             error "Unsupported architecture: $ARCH" ;;
esac

info "Installing tccutil-rs..."
printf "  Architecture: %s\n" "$ARCH"

# Get latest release download URL
RELEASE_URL="https://api.github.com/repos/${REPO}/releases/latest"
info "Fetching latest release..."

DOWNLOAD_URL=$(curl -fsSL "$RELEASE_URL" | grep -o "\"browser_download_url\": *\"[^\"]*${BINARY_NAME}-macos-universal[^\"]*\"" | head -1 | cut -d'"' -f4)

# Fall back to arch-specific binary if universal not found
if [ -z "$DOWNLOAD_URL" ]; then
    DOWNLOAD_URL=$(curl -fsSL "$RELEASE_URL" | grep -o "\"browser_download_url\": *\"[^\"]*${ASSET_SUFFIX}[^\"]*\"" | head -1 | cut -d'"' -f4)
fi

if [ -z "$DOWNLOAD_URL" ]; then
    error "Could not find a release binary for your platform.
  Check https://github.com/${REPO}/releases for available downloads."
fi

printf "  Downloading: %s\n" "$DOWNLOAD_URL"

# Download to temp file
TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT
curl -fsSL -o "$TMPFILE" "$DOWNLOAD_URL"
chmod +x "$TMPFILE"

# Install
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMPFILE" "${INSTALL_DIR}/${BINARY_NAME}"
else
    info "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "$TMPFILE" "${INSTALL_DIR}/${BINARY_NAME}"
fi

printf "\n${GREEN}✓ tccutil-rs installed to ${INSTALL_DIR}/${BINARY_NAME}${NC}\n\n"
printf "Usage:\n"
printf "  tccutil-rs list --compact    # list all TCC permissions\n"
printf "  tccutil-rs services          # list known service names\n"
printf "  tccutil-rs info              # show database info & SIP status\n"
printf "  tccutil-rs --help            # full usage\n\n"
printf "Optional alias (add to ~/.zshrc):\n"
printf "  alias tccutil=\"tccutil-rs\"\n"
