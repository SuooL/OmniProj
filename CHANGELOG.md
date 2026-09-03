# Changelog

All notable changes to OmniProj are recorded here. Versions follow [SemVer](https://semver.org/).
Pre-1.0: the public surface (CLI commands, `~/.omniproj` layout) may still change.

## [Unreleased]

Everything below has landed on `dev` since the `v0.0.1` tag and is **not yet released**:
`main` still points at the initial public release and the workspace version is still `0.0.1`.
The MVP dogfood threshold in `docs/requirements.md` §6 (2–4 weeks of daily use, ≥5 real
projects, ≥20 recorded re-entry events) has **not** been met, so no release is cut.

### Changed

- **Interaction audit sweep** — rather than fixing only the reported examples, every surface
  and state was walked (index, project setup/active, all three workspace tabs, all three task
  views, settings, dialogs) against measurable rules: pointer-target size, accessible name,
  placeholder-as-label, unexplained disabled controls, text-size floor, horizontal overflow,
  clipped-without-title, duplicate names, nested interactives, and label→control travel. The
  fixes below all came out of that sweep.
  - **Forms keep a readable measure.** `space-between` on a full-width settings row was
    harmless in the old 760px column; in the wide detail pane it flung each control ~1000px
    from its own label, and pinned 「启用提醒」's checkbox to the far left with its text at
    the far right. Form rows now cap at a 560px measure, and a checkbox row sits left-aligned
    so the box is beside the words it toggles.
  - **Pointer targets** — checkboxes were 13×13 and the rail toggle 23×27; controls now meet
    the 28px minimum, and the whole label is the checkbox target.
  - **Every disabled control explains itself** — 添加任务, 记录决策, 保存 Agent 设置, and
    测试连接 were disabled with no stated reason; each now carries one.
  - **Chinese-first gaps** — 「模型」 and 「Provider（服务商）」 were shipping the English
    strings inside the Chinese table.
  - **Text-size floor** — `small` rendered at 10.4px; small text now has an 11px floor.
- **The shell is a two-pane desktop layout, not a centered web column** — reviewing the built
  app in a real window showed it behaved like a responsive web page inside a native frame:
  content was locked to a 760px centered column in an 1100px window, switching projects was a
  full page transition, and the task list cost a click on every entry because its accordion
  defaulted closed and reset on navigation. A permanent, searchable, keyboard-navigable
  **project rail** (↑/↓/Home/End, resizable and collapsible with the width remembered) now sits
  beside a detail pane that fills the window. `⌘F` targets the rail from every screen, so the
  shortcut has one unambiguous target instead of depending on the open route. The contract
  clauses this withdraws are recorded in `docs/product-reset-r0.md` §R2 amendment; the project
  page still has exactly one visual endpoint and the rail carries navigation only.
- **Workspace sections are tabs, not accordions** — planning / observed change / project
  management are a segmented control that remembers the chosen pane per session and still
  mounts only the selected panel.
- **Every form control looks like a control** — the control style (border, background, height,
  focus ring) was scoped to `.op-field` / `.op-form-section` / `.op-dialog`, so fields outside
  those containers — new task, decision title, decision rationale, rail search — rendered as
  bare text on the page background and read as body copy. The affordance is now the default for
  every input, select, and textarea; toggles keep their native appearance.
- **Task rows are read-only until opened** — the planning list previously kept four
  borderless inputs, a status select, and Save/Delete on *every* row, so ten tasks meant
  forty always-live form fields and edits were lost unless Save was clicked. A row is now one
  scannable line (text, due signal, tags, status) that expands into a labelled two-column
  edit panel and **autosaves when focus leaves it**; the explicit Save button is gone.
  Status stays on the collapsed row as a single decisive control that commits immediately.
  Nothing is sent when the draft matches what is stored.

### Fixed

- **A finished task is no longer shown as overdue** — the list, board, and time views ran
  the due signal without the task's status, so a completed item with a past date still
  displayed 「逾期 N 天」, contradicting core, where only Planned/Doing/Blocked work produces
  the overdue review reason.
- **Board columns align** — the three status columns now share a height and an empty column
  says 「暂无」 instead of collapsing into a hollow bar.
- **The project heading no longer keeps a focus ring** on entry: focus is still moved there
  for AT/keyboard orientation, but the ring is suppressed for that programmatic move and
  restored on the first real keyboard interaction.
- **Backend error strings no longer reach the user** — a failed task write surfaced raw text
  such as `unhandled update_task`; failures now show the typed, localized message.
- **The Projects Index no longer prints its count twice** (「12  12 个项目」): the numeral is
  the visual anchor and the label carries the unit, with the full phrase as the accessible name.
- **The new-task field reads as a control** (border and padding) instead of blending into the
  paragraph above it, and the tags placeholder is no longer clipped mid-sentence.
- **Vertical rhythm** — section padding and the overview page's top padding were tightened so
  the re-entry page shows real content instead of whitespace on a short window.
- **E2E coverage gap** — the browser harness had no `update_task`/`remove_task` branch, so
  editing a task's due date, tags, or status was never exercised end to end. Both are now
  mocked (with core's tag normalization) and covered by new specs.

### Added

- **R1e cross-project focus strip (FR-A5)** — a collapsible「今日聚焦」strip above the
  Projects Index aggregates overdue + due-today tasks across Active projects, grouped by
  project (oldest debt first). Collapsed it is one line ("N 个项目共 M 条任务逾期或今日到
  期"); expanded, every project name is a jump link into that project — the strip is
  **read-only** (editing stays inside the project) and renders nothing at all when nothing
  is due. Waiting/Parked projects are excluded; an unreadable state document skips its
  project rather than failing the strip. Served by the new read-only `get_focus_agenda`
  IPC command.
- **R1d time-grouped task view (FR-R6)** — a third `按时间` task view groups undone tasks
  by due date against the local calendar: 逾期 / 今天 / 本周 / 下周 / 以后 / 未排期
  (ISO weeks, Monday start). Done tasks are hidden — the view answers "what comes due
  next", not a retrospective — and empty groups are omitted. Pure derivation, zero new
  data; cards reuse the board renderer including the move control and commitment lock.
- **R1c planning board view (FR-R6)** — the task list gains a `list / board` toggle
  (persisted locally) inside the Planning disclosure. The board shows three status columns
  (open/doing/done) with deterministic ordering — oldest overdue first, then dated ascending,
  then undated by recency; cards carry the `?` marker, tags, a due signal (overdue = danger
  with "逾期 N 天" text, ≤7 days = warning; never color-only), and the commitment marker.
  Moves use a keyboard-accessible select — no drag requirement; commitment-bound cards show
  guidance instead of a control (their lifecycle status belongs to commitment actions). The
  done column folds to the newest five with an explicit expand. Task rows now expose
  `updated_at` for the ordering.
- **R1b task tags (FR-R5)** — work items carry 0..8 user classification tags (each ≤24
  chars; trimmed, case-insensitively unique keeping the user's casing and order). Entry is a
  comma-separated field with in-project datalist autocomplete; saved tags render as chips
  and an AND-semantics tag filter joins the task list. The project state document schema is
  now **v2**: v1 documents load unchanged (tags default empty, upgraded in memory) and are
  rewritten as v2 only when next saved; a newer-versioned document is refused with a clear
  version error (checked before field-level parsing). Store-migration provenance checks now
  compare historical state bytes against every canonical rendering (v1 and v2), so stores
  migrated by older versions keep recovering byte-for-byte. Verified against a copy of the
  real `~/.omniproj` store: both v1 projects load, a tagged write persists v2, untouched
  documents stay v1.
- **R1a overdue→Attend (FR-A4)** — a work item whose user-set expected date has passed
  (judged against the user's **local** calendar date; `due == today` is not yet overdue) now
  produces the deterministic review reason `overdue_work` on Active projects, entering the
  needs-decision group, the menu-bar attention count, and the daily reminder. Priority sits
  between `needs_commitment` and `review_action`; evidence names the three oldest overdue
  items (60-char truncation) and folds the rest into a count. Waiting/Parked projects are
  excluded (suspension is an explicit deferral; `scheduled_review` covers their return).
  `REVIEW_RULE_VERSION` is now `r1-v1`. Design: `docs/superpowers/specs/2026-09-02-r1-project-management.md`.
- **M1 menu-bar attention (FR-A3)** — a Tauri tray icon carries the native title
  「N 个待关注」 (hidden at zero) plus a matching tooltip, synced at startup and on a periodic
  refresh as well as after the actions that change the count.
- **M2 human-led task model (FR-R1)** — tasks with tri-state status (`open`/`doing`/`done`),
  an `?`-unformed marker, an expected completion date, and a free-text problem note
  (问题备注); one task can be explicitly promoted to the project's single Current Commitment,
  after which its effective state derives from the commitment lifecycle.
- **M2 git reconciliation (FR-R2)** — a commit timeline in the project page, with one or more
  commits attributable to a single task (many-to-one), plus unbind/rebind.
- **M3 Advance breakdown (FR-V1)** — an agent turns one task or idea into 3–6 concrete
  candidate subtasks that the human adopts item by item; provider/model are configured in-app,
  the API key lives only in the system keychain (service `app.omniproj.desktop.llm`), a remote
  call requires explicit send consent, and a malformed response gets exactly one bounded retry
  before erroring **without** writing a proposal.
- **M4 record deepening** — a branch-aware git flow graph (a compact reconciliation canvas,
  not a full gitk-style history browser) and `plan.md`, a per-project append-only decision log
  that can record 「决定不做」 as `abandoned` rather than deleting it (charter §7).
- **M5 Advance extensions** — `clarify` (FR-V3) and refine-to-spec (FR-V2) wired into the
  desktop Advance layer.
- **R0 project re-entry** (`docs/superpowers/plans/2026-08-11-r0-project-reentry.md`) — the
  bulk of this cycle:
  - **Core** — typed ids and atomic store writes; projects separated from their repository
    sources; an auditable commitment state machine with undo receipts; deterministic project
    review reasons (no health score, no priority ranking).
  - **Capture** — typed repository observations with canonicalized paths and validated
    `git status --porcelain` states.
  - **Desktop** — a typed R0 IPC service over stable project ids, with an observation cache
    invalidated on relink.
  - **Web** — canonical project routes and an AppShell; the dense semantic Projects Index;
    Project Peek and Overview with commitment interactions and focus recovery; project
    registration and moved-source recovery.
- **R0 product reset** (`docs/product-reset-r0.md`) — the re-entry surface rebuilt around the
  actual job: `WorkItem` becomes the canonical task/commitment object (with a one-time
  `notes/next.md` import), the Projects queue splits into "needs a decision" and the rest,
  the project page has one visual endpoint (the current next step) with planning, observed
  change, and project management as progressive disclosures, and language / reminder / Agent
  configuration moves to global Settings. The loop ends inside OmniProj: no editor, terminal,
  Finder, or Codex jump action is a primary call to action.
- **Chinese-first interface** — `zh-CN` is the default locale with English available and the
  choice persisted; status, review-reason, transition, and error labels are all localized.
- **Configurable daily reminders (FR-A2)** — a daily digest by default, adjustable and
  switchable off, with delivery state in `cache/reminder-delivery.toml` so a day fires once.
- **Dogfood instrumentation** — the re-entry timer appends events to
  `~/.omniproj/dogfood/reentry-events.jsonl` as store commits; interpretation in
  `docs/dogfood.md`. This is instrumentation, never a primary user feature.
- **Desktop delivery** — `release.yml` builds and publishes macOS `.dmg` bundles with SHA-256
  sums. Signing, notarization, auto-update, and Homebrew distribution are still out of scope.
- **CI + a pre-PR gate** — three CI jobs (Rust workspace, Frontend unit + build, Frontend e2e)
  and `scripts/pre-pr-check.sh`, an 8-step local gate (fmt → clippy `-D warnings` → build →
  test → npm ci → frontend build → unit tests → Playwright E2E) that must pass before a PR.
- **Interaction and accessibility gates** — Playwright coverage for the core loop plus axe
  (no critical/serious violations), ≥4.5:1 text and ≥3:1 control-boundary contrast in both
  themes, grayscale/forced-colors survival, reduced-motion actually collapsing transitions,
  200% text without horizontal overflow, and responsive behaviour from 1280px down to 640px.

### Changed

- **Desktop design system** — the interface visual system, shell, and navigation were
  reworked several times and finally consolidated into a single "instrument identity"; the
  project workspace was flattened and the permanent project sidebar removed (search moved back
  onto the Projects surface).
- **App icon and situation board** — a dedicated app icon plus the situation-board overview
  redesign.
- **Store recovery and migration** — `~/.omniproj` initialization, migration, and recovery were
  hardened across many rounds: recoverable project mutations, a validated recovery journal with
  legacy-format compatibility, recovery writes checked against trusted paths, preserved legacy
  canonical human state with persisted proofs, a derived migration state-provenance partition,
  and fail-closed behaviour when provenance has no snapshot.

### Fixed

- **Core** — root paths are rejected as audit targets; concurrent in-process store writers are
  serialized, which removes the spurious multi-project refresh failures.
- **Capture** — invalid deleted-status pairs in porcelain output are rejected; the read-only
  proof is scoped to the working tree instead of `.git`, which was the CI-only flaky diff.
- **Desktop** — repository state now refreshes reliably; startup migration no longer breaks the
  first launch; native window dragging works again.
- **Web** — project navigation is trustworthy again (route builder, Escape pop, filter state,
  Index-background Peek, re-entrancy, Peek focus steal); a reserved scrollbar gutter stops
  CI's classic scrollbars from tripping the horizontal-overflow gate.

## [0.0.1] — 2026-08-10

**Desktop-pivot baseline.** OmniProj **resets its version to `0.0.1`** to mark the fresh
start of the Tauri desktop "project advancer". The prior `0.1.0`–`0.3.1` line was the
now-pivoted CLI product; this is a deliberate restart, not a SemVer decrement of the same
product (see `docs/desktop-design.md`). Everything from those releases remains in git
history and the CHANGELOG below.

### Added

- **M0 desktop shell** — `omniproj-desktop` (Tauri) renders the Attend-layer project
  overview: registered projects from `~/.omniproj` with git-derived facts (branch,
  uncommitted-line count, 16-week commit sparkline), read over the `get_projects` Tauri
  IPC command (reusing `omniproj-core` + `omniproj-capture`, no HTTP layer). The React
  frontend (portfolio + sparkline) moved from the removed `omniproj-api` into the desktop
  crate; refresh is a pull, never an auto-poll (charter §8).

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
- **Desktop blank-white window** — `beforeDevCommand` used `npm --prefix web`, which Tauri
  runs from the `web/` dir already, so it resolved to `web/web/package.json` and the Vite
  dev server never started; the dev build then loaded an empty `devUrl` and rendered white.
  Fixed with Tauri's object-form command (`{ script, cwd: "web" }`). Run the app via
  `cargo tauri dev`, not a bare `cargo run` (see CONTRIBUTING).

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

[Unreleased]: https://github.com/SuooL/OmniProj/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/SuooL/OmniProj/releases/tag/v0.0.1
[0.3.1]: https://github.com/SuooL/OmniProj/releases/tag/v0.3.1
[0.3.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.3.0
[0.2.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.2.0
[0.1.0]: https://github.com/SuooL/OmniProj/releases/tag/v0.1.0
