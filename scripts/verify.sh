#!/usr/bin/env bash
# scripts/verify.sh — single canonical gate. CI runs this. The pre-push hook runs this. Run it locally before opening a PR.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

echo "→ cargo fmt --check"
cargo fmt --check

echo "→ cargo clippy -- -D warnings"
cargo clippy --quiet -- -D warnings

echo "→ cargo test"
cargo test --quiet

echo "✓ verify passed"
