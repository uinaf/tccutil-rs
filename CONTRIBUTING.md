# Contributing

Thanks for sending changes.

## Setup

```sh
git clone git@github.com:uinaf/tccutil-rs.git
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

## Releases

Push-to-main, semantic-release driven. Mirrors the [`uinaf/react-json-logic`](https://github.com/uinaf/react-json-logic) setup.

When a `feat:` or `fix:` lands on `main`, the `release` job in [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs after `verify` passes and:

1. **`semantic-release`** analyzes commits since the last `v*` tag and decides the next version.
2. **`scripts/release-prepare.sh`** bumps `Cargo.toml` + `Cargo.lock` to the new version (via `@semantic-release/exec`).
3. **`@semantic-release/git`** commits those files back to `main` as `chore(release): <version> [skip ci]` (the `[skip ci]` keeps the bump from re-triggering the pipeline).
4. **`@semantic-release/github`** creates the `v<version>` tag and the GitHub Release with the changelog as the body.
5. **macOS dual-arch build** runs in the same job, attaching tarballs + `checksums.txt` to the new Release.
6. **`dawidd6/action-homebrew-bump-formula`** opens a PR against [`uinaf/homebrew-tap`](https://github.com/uinaf/homebrew-tap) bumping `Formula/tccutil-rs.rb`.

Bot identity is `glitch418x` (set inside the semantic-release step's `env:`).

Required secrets on this repo:

- `GITHUB_TOKEN` — provided automatically. Used by semantic-release for the bump-back commit, tag, and Release.
- `TAP_GITHUB_TOKEN` — fine-grained PAT for `glitch418x` with `contents: write` and `pull-requests: write` on `uinaf/homebrew-tap`. The default `GITHUB_TOKEN` only has scope on this repo.

`chore:` / `docs:` / `refactor:` commits do not bump the version on their own — land them alongside a `feat:` or `fix:` if you want them in a release. `feat!:` / `BREAKING CHANGE:` bumps the major.

## Pull requests

- Keep changes focused — a single concern per PR.
- Add or update tests when behavior changes. Mock-only tests don't count.
- Run `scripts/verify.sh` before pushing.
- Include the most useful evidence for the kind of change:
  - Command output for new flags or subcommands
  - Before-and-after for output formatting changes
  - SQLite schema notes when the digest set in `KNOWN_DIGESTS` changes
  - Rollout notes when touching write paths or root checks