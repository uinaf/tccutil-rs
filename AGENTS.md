# tccutil-rs

Rust CLI for managing macOS TCC (Transparency, Consent, and Control) privacy permissions databases. Replaces Apple's limited `tccutil` and the Python-based `tccutil.py` with a single static binary — no runtime dependencies.

This file is the agent navigation map. Each section is a pointer:

- User-facing usage, install, commands, JSON envelope → [README.md](README.md)
- Contributor setup, validation, conventions, PR expectations → [CONTRIBUTING.md](CONTRIBUTING.md)
- Vulnerability reporting → [SECURITY.md](SECURITY.md)

## Tech stack

- **Rust 2024 edition** (`edition = "2024"` in Cargo.toml; toolchain pinned in `rust-toolchain.toml`)
- **rusqlite** (bundled SQLite) — reads/writes TCC.db directly
- **clap** (derive) — CLI argument parsing
- **colored** — terminal output formatting
- **chrono** — timestamp formatting (CoreData + Unix)
- **sha1_smol** — schema digest verification
- **dirs** — home directory resolution
- **libc** — root/euid check

## Architecture

Single binary, two source files. Reads both user (`~/Library/Application Support/com.apple.TCC/TCC.db`) and system (`/Library/Application Support/com.apple.TCC/TCC.db`) databases. System DB writes require `sudo`. SIP may block writes on newer macOS.

## Key files

- `src/main.rs` — CLI definition (clap derive), subcommand dispatch, table output formatting
- `src/tcc.rs` — Core logic: `TccDb` struct, DB reads/writes, service name mapping (`SERVICE_MAP`), schema validation, timestamp formatting
- `tests/integration.rs` — Integration tests; exec the real binary via `CARGO_BIN_EXE_tccutil-rs`
- `scripts/verify.sh` — Single canonical gate. CI calls it; the pre-push hook calls it; run it locally before opening a PR
- `Cargo.toml` — Dependencies and package metadata
- `rust-toolchain.toml` — Pinned toolchain channel
