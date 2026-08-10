# Changelog

All notable changes to OmniProj are recorded here. Versions follow [SemVer](https://semver.org/).
Pre-1.0: the public surface (CLI commands, `~/.omniproj` layout) may still change.

## [Unreleased]

### Removed

- **Desktop-pivot teardown** (`docs/desktop-design.md` §6) — OmniProj is pivoting from the
  CLI product to a Tauri desktop "project advancer", so the layers the desktop app replaces
  were removed rather than carried as dead weight:
  - **Crates** — `omniproj-api` (the axum web dashboard + embedded SPA), `omniproj-daemon`
    (the background watcher / floor-timer / refresh orchestration), and `omniproj-ipc`
    (the daemon⇄CLI Unix-socket protocol) are deleted, along with the `web-build` CI job.
  - **CLI commands** — `briefing`, `refresh`, `status`, `daemon`, `opinion`, `dashboard`,
    `curate`, `eval`, `doctor`, `model`, `correct`, `reconcile`, `install-service`,
    `uninstall-service`, and `mcp` are removed. The CLI now keeps `add`/`list`/`remove`,
    `note`/`next`/`clarify`, `recall`, `search`, `digest`, `stats`, and `providers`/`init`.
  - **`omniproj-distill` modules** — `opinion`, `deep` (the deep reasoning pipeline),
    `eval`, `doctor`, `curate`, and `learn` are removed; the crate is now the provider
    adapters + the verify gate + `clarify` (the grounding foundation the desktop Advance
    layer will reuse). The base `distill()`/`verify_output()` pipeline is retained as
    library code.

  Everything removed remains in git history and can be restored. `omniproj-core`,
  `omniproj-capture`, and `omniproj-index` are retained unchanged.

### Fixed

- **Test isolation** — `omniproj-index`'s tests wrote their sqlite index into the
  *user's real* `~/.omniproj` (the cache dir derives from `OMNIPROJ_HOME`, which they never
  set), leaving three stray `projects/<tag><pid>/` dirs behind on every `cargo test`
  run. They now point `OMNIPROJ_HOME` at a throwaway temp store, serialized on a local
  guard because the env var is process-global, with a regression test asserting the
  index path stays inside the sandbox.
- **Stale `mnemo-desktop` build artifacts** — the desktop crate's earlier name (`mnemo-desktop`,
  from before the Mnemo → OmniProj rename) left cached Tauri build output under `target/`
  that pinned an absolute `…/git/Mnemo/…` permissions path, breaking `omniproj-desktop`'s
  build script. Documented here; the fix is a local `cargo clean` of the stale artifacts
  (`target/` is not tracked).

## [0.3.1] — 2026-07-13

Patch release. Follows the initial public open-source release; adds the reconcile
flow and a dogfood-found eval fix, and repairs the release pipeline so all three
platform binaries are produced.

### Added

- **Reconcile flow** — a distill no longer silently overwrites hand-edits to
  `auto/*.md`: an uncommitted user edit parks the AI's version as `<file>.incoming`
  and `omniproj reconcile [--keep-mine|--take-ai]` shows the diff and resolves it
  (charter §5 原则4). `omniproj recall` now also surfaces user-authored `notes/`.

### Fixed

- **Release pipeline** — the `x86_64-apple-darwin` job is now cross-compiled on a
  `macos-14` arm64 runner instead of the scarce/deprecated `macos-13` runners, which
  had left the Intel-Mac binary un-built on the v0.3.0 releases.
- **`omniproj eval` robustness** — the judge-JSON parser tolerates unescaped quotes in a
  model's `rationale` string (found by dogfooding against a live model): it falls back
  to extracting the integer scores instead of erroring on the whole response.

## [0.3.0] — 2026-07-13

Public-launch-readiness release: the feature surface was already complete, so this
release hardens the "trustworthy engineering" dimension the differentiator (code-checked
trust) depends on. The core contract is unchanged — state stays local in `~/.omniproj`,
user repositories are never written to, and LLM outputs are grounded by deterministic
capture plus post-distill verification — but the digest sent to a provider is now
scrubbed by default, the trust foundations are tested, CI/release automation is back,
and the daemon and on-disk state are hardened for daily use.

### Added

- **Outbound-digest privacy** — sensitive paths (`.env*`, `*.key`, `*.pem`, `id_rsa*`,
  `secrets/`, `credentials*`, …) are dropped and secret shapes (`sk-…`, `AKIA…`,
  `Bearer …`, `KEY=value`) masked before the digest reaches the provider, both on by
  default. `omniproj digest` previews the exact outbound text; `--no-redact` opts out of
  masking (deny-list still applies); a one-time consent notice precedes the first
  remote distill; `[privacy]` config (`deny_globs`/`redact`/`send_consent`).
- **`omniproj install-service` / `uninstall-service`** — install the daemon as a macOS
  LaunchAgent (`KeepAlive`) or Linux systemd user unit (`Restart=always`) so it
  auto-starts at login and restarts on crash. Documented in a new README section.
- **`~/.omniproj` schema versioning** — `SCHEMA_VERSION` with a stepwise migration
  skeleton; existing stores adopt v1 non-destructively, and a store written by a newer
  OmniProj is refused rather than corrupted.
- **`omniproj doctor`** — read-only diagnostics for store/config/model-key/provider
  connectivity, plus an actionable first-run missing-key error.
- **CI + release automation restored** — `ci.yml` (fmt → clippy `-D warnings` → build →
  test on push/PR to main+dev) and `release.yml` (tag `v*` → macOS arm64/x86_64 +
  Linux x86_64 binaries with SHA-256 → GitHub Release).
- **Demo + top-of-README banner** — a console-based re-entry demo in the README plus
  platform-support and privacy-boundary notes on the first screen.
- **OSS hygiene** — `CONTRIBUTING.md`, `SECURITY.md` (states the LLM-provider data
  boundary), `CODE_OF_CONDUCT.md`, issue/PR templates, `rust-toolchain.toml`,
  `rustfmt.toml`, and crates.io metadata (internal crates marked `publish = false`).

### Changed

- **Test foundations** — the capture layer (`git.rs`/`claude.rs`/`codex.rs`) that feeds
  the verify-gate whitelist now has real assertion tests; a mock `LlmProvider` enables
  an end-to-end capture→distill→verify→commit test without a live LLM; an eval baseline
  placeholder with documented thresholds is recorded. Test count rose from 63 to 101.
- **Daemon crash recovery** — the shared-status mutex is poison-tolerant and each distill
  job is panic-isolated, so one bad job can no longer take down the daemon.

### Fixed

- Restored the CI/release workflows removed in the 0.2.0 cycle; the earlier
  `startup_failure` was private-repo Actions-minute exhaustion, not a workflow-file bug.

## [0.2.0] — 2026-06-13

Second functional release: OmniProj graduates from the thin loop into a usable local
project-memory cockpit for AI coding workflows. This release keeps the core contract:
state stays local in `~/.omniproj`, user repositories are never written to, and LLM
outputs are grounded by deterministic capture plus post-distill verification.

### Added

- **MCP integration** — `omniproj mcp` serves read-only project memory over stdio with
  `project_recall`, `project_search`, and `briefing` / `decisions` / `open` /
  `opinion` / `learned` resources for agent session start.
- **Claude Code hook guide** — documented SessionStart recall and SessionEnd refresh
  recipes in the README's "Agent integration (MCP + hooks)" section.
- **Session-root watching** — the daemon now watches Claude Code and Codex transcript
  roots, then refreshes after a quiet window so conversation-only progress does not
  wait for the 24h floor.
- **Raw-session search** — new `omniproj-index` crate builds a disposable SQLite/FTS5
  index over normalized session text; exposed through `omniproj search`, dashboard
  search, and MCP `project_search`.
- **Dashboard cockpit** — the local dashboard now shows trust, user-model, and state
  panels, fixed state tabs, search, and explicit second-opinion generation.
- **Second-opinion persistence** — CLI and dashboard use the same grounded
  orchestration and write `opinion.md` as a revertable store commit.
- **User-model visibility** — dashboard exposes active/disabled/over-budget profile
  dimensions without rewriting the user-owned model file.
- **Curator completion** — `omniproj curate` now handles `decisions.md`, `open.md`, and
  oversized `learned.md`; `omniproj stats` reports state-file sizes and store history.
- **Gold-eval harness** — `omniproj eval --gold <file>` scores factual consistency,
  coverage, and concision against a user-supplied handoff and stores cache reports.

### Fixed

- Provider calls now have HTTP timeout, connect timeout, and retry/backoff for
  transient transport / 429 / 5xx failures; daemon jobs have a per-project timeout.
- Verify reports are persisted to `cache/verify-report.json` for dashboard and
  quality observation.
- The verify gate now checks repo-relative path tokens in addition to commit hashes.
- Store writes use a transaction lock so concurrent CLI/daemon distills do not smear
  multiple updates into one `~/.omniproj` commit.
- Refresh gating now includes a stable digest of the full dirty worktree status
  (`git status --porcelain`) in addition to `HEAD` and newest session mtime, so
  uncommitted/staged/untracked changes are not mistaken for "up to date".
- `omniproj add` / `omniproj remove` now best-effort notify a running daemon to reload the
  project registry immediately instead of waiting for the 24h floor sweep.

### Changed

- README and v1 spec now state the actual trust boundary: deterministic verification
  covers commit hashes and repo-relative paths; numbers, tool exit codes, and
  semantic claims require source/provenance handling and remain v2 work.
- README now documents the privacy boundary: persistent state is local, but
  distillation sends captured digests to the configured provider unless a local
  endpoint is selected.

## [0.1.0] — 2026-06-07

First functional release: the **thin loop** (capture → ground → distill → verify) plus a
fully autonomous **background daemon**. Note this is an early milestone, not the complete
product "v1" — user model, second opinion, and the dashboard remain out of scope for now
(see `docs/omniproj-charter.md`).

### Added

- **Capture (`omniproj-capture`)** — passively gather git activity + Claude Code / Codex
  sessions for a project, normalize to a unified `Session`, and render a recency-aware
  substrate digest. No LLM.
- **Grounding + verify gate** — capture produces a deterministic `FactSheet` (real
  git log / HEAD / commit hashes); distillation is grounded on it and a code-level
  verify gate flags any commit hash the FactSheet can't vouch for. Trust is enforced
  by code, not prompt wishes (goal #1, spec §5.2).
- **Distillation (`omniproj-distill`)** — turn a digest into `briefing.md` / `decisions.md`
  / `open.md` via a provider-neutral `LlmProvider` (Anthropic + OpenAI-compatible
  adapters covering OpenRouter/Groq/DeepSeek/Together/xAI/Gemini-compat/Ollama and
  custom endpoints), selectable with `--model provider/model`.
- **Registry + self-versioning** — `omniproj add/list/remove`; `~/.omniproj` is a git repo so
  every distill/curate lands as an independent, revertable commit.
- **Self-iteration** — `omniproj correct` distills user corrections into per-project
  `learned.md` (injected into future distills); `omniproj curate` consolidates the
  append-only `decisions.md` (goal #3, spec §5.3).
- **Staleness floor** — a deterministic change fingerprint (git HEAD + newest session
  mtime) gates re-distillation: `omniproj refresh [--all] [--force]` distills only on
  change, silent otherwise.
- **Daemon (`omniproj daemon`)** — `notify` watcher over registered worktrees + a 24h floor
  timer feed a single off-loop worker (dedup queue, per-project error isolation);
  single-instance `flock`; graceful SIGTERM.
- **IPC + status** — the daemon serves a Unix socket (`~/.omniproj/daemon.sock`);
  `omniproj status` lazy-starts the daemon and reports pid / uptime / in-flight /
  per-project watch + last-distill.
- Supporting commands: `omniproj briefing`, `digest`, `providers`, `init`.

### Notes

- IPC ships as length-delimited JSON over the Unix socket rather than the spec's
  tonic gRPC (no `protoc` on the dev toolchain; tonic 0.14 codegen churn). Surface is
  equivalent and the `omniproj-ipc` crate boundary keeps a later gRPC swap localized.
  See `docs/omniproj-charter.md`.

[Unreleased]: https://github.com/SuooL/OmniProj/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/SuooL/OmniProj/releases/tag/v0.3.1
[0.3.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.3.0
[0.2.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.2.0
[0.1.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.1.0
