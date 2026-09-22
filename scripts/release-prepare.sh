#!/usr/bin/env bash
# Bumps the version in Cargo.toml + Cargo.lock to the version semantic-release
# computed for the upcoming release. Invoked by @semantic-release/exec via
# `prepareCmd` in .releaserc.json.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <version>" >&2
  exit 2
fi

version="$1"

tmp="$(mktemp)"
awk -v v="$version" '
  /^version = / && !done { print "version = \"" v "\""; done=1; next }
  { print }
' Cargo.toml > "$tmp"
mv "$tmp" Cargo.toml

# `cargo check` updates Cargo.lock when Cargo.toml's version changes.
cargo check --quiet

echo "Bumped Cargo.toml + Cargo.lock to version $version"
