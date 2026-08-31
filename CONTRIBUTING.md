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

The `omniproj-cli` crate produces the `omniproj` binary:

```sh
cargo run -p omniproj-cli -- --help
cargo install --git https://github.com/SuooL/OmniProj omniproj-cli   # installs `omniproj`
```

## Running the desktop app (`omniproj-desktop`)

The Tauri desktop app must be run through the Tauri CLI, **not** as a bare
`cargo run -p omniproj-desktop` — a dev build loads the Vite dev server at
`build.devUrl` (`:5173`), so running the plain binary with no dev server yields a
**blank white window**. Use:

```sh
cargo install tauri-cli --version '^2' --locked   # first time (provides `cargo tauri`)
cargo tauri dev                                    # from crates/omniproj-desktop/
```

`cargo tauri dev` starts the Vite dev server (`beforeDevCommand`) and launches the app
pointed at it. For a standalone bundle that embeds the frontend (no dev server needed),
use `cargo tauri build`. The React frontend lives in `crates/omniproj-desktop/web/`.

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

CI runs the same Rust and frontend checks on every PR. Run the repository gate locally first;
all steps must pass:

```sh
./scripts/pre-pr-check.sh
```

The script is the source-of-truth wrapper and runs from the repository root. It also changes
into the frontend directory before invoking npm, preventing a false pass/fail caused by the
wrong working directory. The underlying Rust gates are:

```sh
cargo fmt --all --check                              # formatting
cargo clippy --workspace --all-targets -- -D warnings  # lints (warnings are errors)
cargo build --workspace --locked                     # build
cargo test --workspace --locked                      # tests
```

After committing and pushing, wait for `gh pr checks <number>` to finish. A local pass does not
replace remote CI evidence; if any job fails, inspect that job's log and fix it before claiming
the PR is ready.

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
