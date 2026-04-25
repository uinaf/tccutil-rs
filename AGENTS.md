# tccutil-rs

CLI helpers for managing macOS TCC permissions. Single static binary, no runtime deps.

This file is the agent navigation map. User-facing usage lives in [README](README.md), contributor setup and validation in [contributing](CONTRIBUTING.md), and vulnerability reporting in [security](SECURITY.md).

## tech stack

- **Rust 2024 edition** (`edition = "2024"` in Cargo.toml; toolchain pinned in `rust-toolchain.toml`)
- **rusqlite** (bundled SQLite) — reads/writes TCC.db directly
- **clap** (derive) — CLI argument parsing
- **colored** — terminal output formatting
- **chrono** — timestamp formatting (CoreData + Unix)
- **sha1_smol** — schema digest verification
- **dirs** — home directory resolution
- **libc** — root/euid check

## architecture

Single binary, two source files. Reads both user (`~/Library/Application Support/com.apple.TCC/TCC.db`) and system (`/Library/Application Support/com.apple.TCC/TCC.db`) databases. System DB writes require `sudo`. SIP may block writes on newer macOS.

## key files

- `src/main.rs` — CLI definition (clap derive), subcommand dispatch, table output formatting
- `src/tcc.rs` — core logic: `TccDb` struct, DB reads/writes, service name mapping (`SERVICE_MAP`), schema validation, timestamp formatting
- `tests/integration.rs` — integration tests; exec the real binary via `CARGO_BIN_EXE_tccutil-rs`
- `scripts/verify.sh` — single canonical gate. CI calls it; the pre-push hook calls it; run it locally before opening a PR
- `Cargo.toml` — dependencies and package metadata
- `rust-toolchain.toml` — pinned toolchain channel

## commands

`list`, `grant`, `revoke`, `enable`, `disable`, `reset`, `services`, `info`. Service names accept both human-readable (`Accessibility`) and internal (`kTCCServiceAccessibility`) forms. See the [readme](README.md#commands) for examples.

## conventions

- conventional commits (`feat:`, `fix:`, `test:`, `docs:`, `chore:`)
- no `unsafe` (except the one `libc::geteuid()` call for root detection)
- errors return `Result<_, TccError>` — typed enum with discrete kinds (`DbOpen`, `NotFound`, `NeedsRoot`, `UnknownService`, `AmbiguousService`, `QueryFailed`, `SchemaInvalid`, `HomeDirNotFound`, `WriteFailed`); no panics in library code
- table output uses manual column-width calculation with ANSI-aware padding

## validation

```sh
scripts/verify.sh
```

Runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` — same gates CI runs.
