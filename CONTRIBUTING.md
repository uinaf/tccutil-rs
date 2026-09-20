# Contributing

Thanks for sending changes.

## Setup

```sh
git clone git@github.com:uinaf/tccutil-rs.git
cd tccutil-rs
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

One entrypoint runs the core gates:

```sh
make verify
```

It runs stale `cargo fmt --check`, `cargo clippy -- -D warnings`, and
`cargo test` lanes concurrently, the same set the optional pre-push hook runs.
Use the exhaustive command after deleting or renaming Rust sources because
timestamp freshness cannot represent those changes reliably.

CI runs the full gate (`cargo llvm-cov` for coverage at 75% line threshold, which also runs the test suite, plus a release build):

```sh
make verify-full
```

Locally you can run the same command before opening a PR if you want parity with the CI Verify job.
Optional pre-push gate that calls the same verification target:

```sh
scripts/setup-hooks.sh         # one-time, points git at .git-hooks/
```

After install, every `git push` runs `make verify` and fails the push if anything goes red.

## Development notes

- Conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `chore:`. CI does not enforce; reviewers do.
- No `unsafe` outside the single `libc::geteuid()` call in `src/tcc.rs`.
- Errors return `Result<_, TccError>`. Add a new variant when an error doesn't fit the existing kinds.
- Table output in `src/main.rs` does manual ANSI-aware padding. If you touch it, run `tccutil-rs list` against a real TCC.db to eyeball alignment.
- Integration tests in `tests/integration.rs` exec the real binary via `CARGO_BIN_EXE_tccutil-rs`. Unit tests in `src/tcc.rs` round-trip real SQLite via `tempfile`. No mocks.

## Releases

Use Conventional Commits (`feat:`, `fix:`, `docs:`, …); they drive versions.

Successful pushes to protected `main` evaluate commits after `verify` passes.
The release job mints a short-lived `uinaf-ci` token inside the
`release` Environment, publishes the GitHub Release and darwin archives, then
updates the Homebrew formula. See [Releases](docs/RELEASES.md).

## Pull requests

- Keep changes focused: a single concern per PR.
- Add or update behavior-covering tests when behavior changes.
- Run `make verify` before pushing.
- Include the most useful evidence for the kind of change:
  - Command output for new flags or subcommands
  - Before-and-after for output formatting changes
  - SQLite schema notes when the digest set in `KNOWN_DIGESTS` changes
  - Rollout notes when touching write paths or root checks
