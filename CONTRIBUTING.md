# Contributing

Thanks for sending changes.

## Setup

```sh
git clone git@github.com:uinaf/tccutil.git
cd tccutil
cargo build --release
```

Binary lands at `target/release/tccutil-rs`. Cargo auto-installs the pinned toolchain on first run.

## Run locally

Invoke the binary directly while iterating:

```sh
cargo run -- list --user
cargo run -- info
```

Read commands work without privileges. Write commands (`grant`, `revoke`, `enable`, `disable`, `reset`) need either the user database (no sudo) or `sudo` for the system database. See [SIP limitations](README.md#sip-limitations) in the README.

## Validation

One entrypoint runs everything CI runs:

```sh
scripts/verify.sh
```

It runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` — the same gates as the CI `Verify` job.

Optional pre-push gate that calls the same script:

```sh
scripts/setup-hooks.sh         # one-time, points git at .git-hooks/
```

After install, every `git push` runs `scripts/verify.sh` and fails the push if anything goes red.

## Development notes

- Conventional commits — `feat:`, `fix:`, `test:`, `docs:`, `chore:`. CI does not enforce; reviewers do.
- No `unsafe` outside the single `libc::geteuid()` call in `src/tcc.rs`.
- Errors return `Result<_, TccError>` — never panic in library code. Add a new variant when an error doesn't fit the existing kinds.
- Table output in `src/main.rs` does manual ANSI-aware padding. If you touch it, run `tccutil-rs list` against a real TCC.db to eyeball alignment.
- Integration tests in `tests/integration.rs` exec the real binary via `CARGO_BIN_EXE_tccutil-rs`. Unit tests in `src/tcc.rs` round-trip real SQLite via `tempfile`. No mocks.

## Pull requests

- Keep changes focused — a single concern per PR.
- Add or update tests when behavior changes. Mock-only tests don't count.
- Run `scripts/verify.sh` before pushing.
- Include the most useful evidence for the kind of change:
  - Command output for new flags or subcommands
  - Before-and-after for output formatting changes
  - SQLite schema notes when the digest set in `KNOWN_DIGESTS` changes
  - Rollout notes when touching write paths or root checks
