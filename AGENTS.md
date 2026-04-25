# tccutil-rs

Rust CLI for managing macOS TCC (Transparency, Consent, and Control) privacy permissions databases. Replaces Apple's limited `tccutil` and the Python-based `tccutil.py` with a single static binary — no runtime dependencies.

macOS hides non-app-bundle clients (CLI tools, scripts) from the Privacy & Security UI, and Apple's `tccutil` only supports `reset`. This tool gives full read/write access to both user and system TCC.db files: list, grant, revoke, enable, disable, and reset individual entries.

## Tech stack

- **Rust 2024 edition** (`edition = "2024"` in Cargo.toml)
- **rusqlite** (bundled SQLite) — reads/writes TCC.db directly
- **clap** (derive) — CLI argument parsing
- **colored** — terminal output formatting
- **chrono** — timestamp formatting (CoreData + Unix)
- **sha1_smol** — schema digest verification
- **dirs** — home directory resolution
- **libc** — root/euid check

## Build / Test / Install

```sh
cargo build --release          # binary at target/release/tccutil-rs
cargo test                     # unit + integration tests
cargo clippy                   # lint
cargo fmt                      # format
install -m 0755 target/release/tccutil-rs /usr/local/bin/tccutil-rs  # install
```

The Rust toolchain is pinned via `rust-toolchain.toml`; rustup auto-installs it on first `cargo` invocation in this directory.

Optional pre-push gate (mirrors CI: fmt + clippy + test):

```sh
scripts/setup-hooks.sh         # one-time, points git at .git-hooks/
```

## Architecture

Single binary, two source files. Reads both user (`~/Library/Application Support/com.apple.TCC/TCC.db`) and system (`/Library/Application Support/com.apple.TCC/TCC.db`) databases. System DB writes require `sudo`. SIP may block writes on newer macOS.

## Key files

- `src/main.rs` — CLI definition (clap derive), subcommand dispatch, table output formatting
- `src/tcc.rs` — Core logic: `TccDb` struct, DB reads/writes, service name mapping (`SERVICE_MAP`), schema validation, timestamp formatting
- `tests/integration.rs` — Integration tests
- `Cargo.toml` — Dependencies and package metadata

## Commands

`list`, `grant`, `revoke`, `enable`, `disable`, `reset`, `services`, `info`

Service names accept both human-readable (`Accessibility`) and internal (`kTCCServiceAccessibility`) forms.

## Conventions

- Conventional commits (`feat:`, `fix:`, `test:`, `docs:`, `chore:`)
- No `unsafe` (except the one `libc::geteuid()` call for root detection)
- Errors return `Result<_, TccError>` — typed enum with discrete kinds (`DbOpen`, `NotFound`, `NeedsRoot`, `UnknownService`, `AmbiguousService`, `QueryFailed`, `SchemaInvalid`, `HomeDirNotFound`, `WriteFailed`); no panics in library code
- Table output uses manual column-width calculation with ANSI-aware padding
