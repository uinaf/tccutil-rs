#!/usr/bin/env bash
# scripts/verify.sh — canonical verification gate.
# Pre-push / default local: fmt + clippy + test.
# CI Verify job calls: scripts/verify.sh --full
#
# Usage:
#   scripts/verify.sh          # fmt + clippy + test (default / pre-push)
#   scripts/verify.sh --full   # coverage (75%, runs the test suite) + release build
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

full=0
for arg in "$@"; do
  case "$arg" in
    --full) full=1 ;;
    -h|--help)
      echo "Usage: scripts/verify.sh [--full]"
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      echo "Usage: scripts/verify.sh [--full]" >&2
      exit 2
      ;;
  esac
done

echo "→ cargo fmt --check"
cargo fmt --check

echo "→ cargo clippy -- -D warnings"
cargo clippy --quiet -- -D warnings

if [[ "$full" -eq 1 ]]; then
  # llvm-cov executes the test suite; skip a redundant plain `cargo test`.
  echo "→ cargo llvm-cov --fail-under-lines 75"
  rustup component add llvm-tools-preview
  if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
    cargo install cargo-llvm-cov --locked
  fi
  cargo llvm-cov --fail-under-lines 75

  echo "→ cargo build --release"
  cargo build --release --quiet
else
  echo "→ cargo test"
  cargo test --quiet
fi

echo "✓ verify passed"
