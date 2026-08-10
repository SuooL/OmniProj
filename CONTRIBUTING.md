# Contributing to OmniProj

Thanks for your interest in OmniProj. It's a local-first, provider-neutral cognitive
scaffolding tool for LLM-era knowledge work. This guide covers how to build, test,
and submit changes.

## Building

OmniProj is a Rust workspace. You need a stable Rust toolchain (the repo pins one via
[`rust-toolchain.toml`](rust-toolchain.toml), so `rustup` will fetch the right
components automatically).

```sh
# Standard setup (rustup on PATH):
cargo build --workspace
```

The single binary is produced by the `omniproj-cli` crate:

```sh
cargo run -p omniproj-cli -- --help
cargo install --git https://github.com/SuooL/OmniProj omniproj-cli   # installs `omniproj`
```

> Environment note: some maintainer machines don't have `cargo` on `PATH` and invoke
> it via the rustup toolchain directory (e.g.
> `export PATH="$HOME/.rustup/toolchains/stable-<target>/bin:$PATH"`). That's
> environment-specific — the standard path is plain `rustup` + `cargo`.

## Testing

```sh
cargo test --workspace
```

Tests are hermetic: they never touch your real `~/.omniproj`, `~/.claude`, or
`~/.codex`. Filesystem tests point `OMNIPROJ_HOME` at a tempdir; LLM calls are stubbed
with an in-crate mock provider (no network, no keys). Please keep new tests hermetic.

## Before you open a PR — the CI gates

CI runs the same four checks on every PR. Run them locally first; all must pass:

```sh
cargo fmt --all --check                              # formatting
cargo clippy --workspace --all-targets -- -D warnings  # lints (warnings are errors)
cargo build --workspace --locked                     # build
cargo test --workspace --locked                      # tests
```

`fmt` and `clippy` are **required**, not optional. `-D warnings` means any clippy
warning fails CI.

## Git workflow (trimmed Git Flow)

- `main` — released, stable.
- `dev` — integration branch; **PRs target `dev`**.
- `feature/<name>` — your work branches off `dev`.

**Never commit directly to `main` or `dev`.** Open a feature branch, push it, and
open a pull request against `dev`. `main` only advances via release merges.

## Pull requests

- Fill in the PR template checklist.
- Keep changes focused; update the relevant spec doc under `docs/` (`omniproj-charter.md`,
  `requirements.md`, `desktop-design.md`) and `README.md` when behavior changes.
- Add tests for behavior changes, especially anything touching capture, the verify
  gate, privacy redaction, or state migration.

## Reporting bugs / requesting features

Use the issue templates under [`.github/ISSUE_TEMPLATE`](.github/ISSUE_TEMPLATE).
For anything security- or privacy-related, see [SECURITY.md](SECURITY.md) instead of
filing a public issue.
