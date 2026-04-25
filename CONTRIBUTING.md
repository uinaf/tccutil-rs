# contributing

thanks for sending changes.

## setup

the rust toolchain is pinned in `rust-toolchain.toml`. rustup auto-installs it on the first `cargo` invocation in this directory — no manual step required.

```sh
git clone git@github.com:uinaf/tccutil.git
cd tccutil
cargo build --release
```

binary lands at `target/release/tccutil-rs`.

## run locally

invoke the binary directly while iterating:

```sh
cargo run -- list --user
cargo run -- info
```

read commands work without privileges. write commands (`grant`, `revoke`, `enable`, `disable`, `reset`) need either the user db (no sudo) or `sudo` for the system db. see the [sip limitations](README.md#sip-limitations) section in the readme.

## validation

one entrypoint runs everything ci runs:

```sh
scripts/verify.sh
```

it runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` — same gates as the ci `verify` job.

optional pre-push gate that calls the same script:

```sh
scripts/setup-hooks.sh         # one-time, points git at .git-hooks/
```

after install, every `git push` runs `scripts/verify.sh` and fails the push if anything goes red.

## development notes

- conventional commits — `feat:`, `fix:`, `test:`, `docs:`, `chore:`. ci does not enforce; reviewers do.
- no `unsafe` outside the single `libc::geteuid()` call in `src/tcc.rs`.
- errors return `Result<_, TccError>` — never panic in library code. add a new variant when an error doesn't fit the existing kinds.
- table output in `src/main.rs` does manual ansi-aware padding. if you touch it, run `tccutil-rs list` against a real tcc.db to eyeball alignment.
- integration tests in `tests/integration.rs` exec the real binary via `CARGO_BIN_EXE_tccutil-rs`. unit tests in `src/tcc.rs` round-trip real sqlite via `tempfile`. no mocks.

## pull requests

- keep changes focused. a single concern per pr.
- add or update tests when behavior changes. mock-only tests don't count.
- run `scripts/verify.sh` before pushing.
- include the most useful evidence for the kind of change:
  - command output for new flags or subcommands
  - before-and-after for output formatting changes
  - sqlite schema notes when the digest set in `KNOWN_DIGESTS` changes
  - rollout notes when touching write paths or root checks
