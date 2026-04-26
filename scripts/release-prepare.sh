#!/usr/bin/env bash
# Bumps the version in Cargo.toml + Cargo.lock to the version semantic-release
# computed for the upcoming release. Invoked by @semantic-release/exec via
# `prepareCmd` in .releaserc.json.
#
# Runs in CI only — you should not need to run this locally.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <version>" >&2
  exit 2
fi

version="$1"

# Bump only the [package] version line, not any dependency version specs.
# awk replaces the first matching `^version = ` line and leaves the rest of
# the file alone; this is portable across BSD awk (macOS) and GNU awk.
tmp="$(mktemp)"
awk -v v="$version" '
  /^version = / && !done { print "version = \"" v "\""; done=1; next }
  { print }
' Cargo.toml > "$tmp"
mv "$tmp" Cargo.toml

# Refresh Cargo.lock so the local-crate entry matches the new version.
# `cargo check` updates Cargo.lock when Cargo.toml's version changes.
cargo check --quiet

echo "Bumped Cargo.toml + Cargo.lock to version $version"
