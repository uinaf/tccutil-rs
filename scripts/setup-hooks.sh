#!/usr/bin/env bash
# Point git at the repo-tracked hooks under .git-hooks/.
# Idempotent — safe to re-run.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

git config core.hooksPath .git-hooks
chmod +x .git-hooks/*

echo "Hooks installed: $(git config core.hooksPath)"
