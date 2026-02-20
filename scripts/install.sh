#!/bin/sh
set -eu

REPO="uinaf/tccutil"
BINARY_NAME="tccutil-rs"
INSTALL_PATH="/usr/local/bin/${BINARY_NAME}"

usage() {
  cat <<'USAGE'
Install tccutil-rs from GitHub Releases.

Usage:
  scripts/install.sh [VERSION]

Examples:
  scripts/install.sh          # install latest release
  scripts/install.sh v0.1.1   # install a specific release

Notes:
  - macOS only
  - installs to /usr/local/bin/tccutil-rs
USAGE
}

error() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

if [ "$(uname -s)" != "Darwin" ]; then
  error "tccutil-rs is macOS-only"
fi

arch="$(uname -m)"
case "$arch" in
  arm64) platform="darwin-arm64" ;;
  x86_64) platform="darwin-amd64" ;;
  *) error "unsupported architecture: $arch" ;;
esac

command -v curl >/dev/null 2>&1 || error "curl is required"
command -v shasum >/dev/null 2>&1 || error "shasum is required"
command -v tar >/dev/null 2>&1 || error "tar is required"

version_arg="${1:-}"
if [ -z "$version_arg" ]; then
  version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$version" ] || error "failed to resolve latest release version"
else
  case "$version_arg" in
    v*) version="$version_arg" ;;
    *) version="v$version_arg" ;;
  esac
fi

asset="${BINARY_NAME}_${version}_${platform}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/${version}"
asset_url="${base_url}/${asset}"
checksums_url="${base_url}/checksums.txt"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

printf 'Installing %s (%s) from %s\n' "$BINARY_NAME" "$platform" "$version"

curl -fsSL "$asset_url" -o "$tmp_dir/$asset" || error "failed to download asset: $asset_url"
curl -fsSL "$checksums_url" -o "$tmp_dir/checksums.txt" || error "failed to download checksums: $checksums_url"

(
  cd "$tmp_dir"
  grep "  ${asset}$" checksums.txt | shasum -a 256 -c - >/dev/null
) || error "checksum verification failed"

(
  cd "$tmp_dir"
  tar -xzf "$asset"
)

[ -f "$tmp_dir/$BINARY_NAME" ] || error "binary not found in archive"

install_dir="$(dirname "$INSTALL_PATH")"
if [ -w "$install_dir" ]; then
  install -m 0755 "$tmp_dir/$BINARY_NAME" "$INSTALL_PATH"
else
  command -v sudo >/dev/null 2>&1 || error "sudo is required to install to $INSTALL_PATH"
  sudo install -m 0755 "$tmp_dir/$BINARY_NAME" "$INSTALL_PATH"
fi

printf 'Installed %s to %s\n' "$BINARY_NAME" "$INSTALL_PATH"
"$INSTALL_PATH" --version
