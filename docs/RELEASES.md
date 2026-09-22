# Releases

Pushes to `main` release automatically. Skip a push with `[skip ci]`.

## Versioning

Conventional Commits drive the bump (see `.releaserc.json`):

| Commit type | Release |
|---|---|
| `feat:` | minor |
| `fix:` / `perf:` | patch |
| `feat!:` / breaking change | major |
| `docs:` / `test:` / `chore:` / `refactor:` / `build:` / `ci:` | none |

## Pipeline

1. `verify` runs with read-only credentials
2. Protected `release` Environment mints a short-lived `uinaf-ci` installation token scoped to `tccutil-rs`
3. `semantic-release` prepares the Cargo files, commits them through GitHub's
   signed App commit API, then creates the version tag and a mutable draft
   GitHub Release; exact-tag lookup fails if the expected Release is unavailable
4. The release job builds, uploads, and attests both darwin archives, publishes the draft once, and verifies GitHub's immutable-release attestation
5. A follow-up job mints a Contents-only token scoped to `tccutil-rs` + `homebrew-tap` and updates the Homebrew formula; it runs on ubuntu because Docker actions do not run on macOS

Concurrency is job-level: a workflow-level cancellable group would cancel
`release` mid-flight on a second push to `main`, leaving a tag and Release
without assets or the Homebrew bump. The release job installs the toolchain
with `rustup show` so the cross-compile targets attach to the channel pinned in
`rust-toolchain.toml`; `dtolnay/rust-toolchain@stable` would install them onto a
different toolchain than cargo uses and silently fail the build.

Published releases and their `v*` tags are immutable. Draft assets may be
replaced only while recovering a partial upload; after publication, retries
skip mutation and resume from immutable verification or the Homebrew job.

Sources of truth: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), [`.releaserc.json`](../.releaserc.json).

## Credentials

`release` Environment:

| Name | Kind |
|---|---|
| `UINAF_CI_APP_CLIENT_ID` | variable |
| `UINAF_CI_APP_PRIVATE_KEY` | secret |
