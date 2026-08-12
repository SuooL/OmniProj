# Task 7 Report: Typed R0 Desktop Service and Command Allowlist

## Scope

Implemented Task 7 only, entirely inside `crates/omniproj-desktop` plus the root
`Cargo.lock` (adding `tauri-plugin-dialog`). No `omniproj-core`/`-capture`/`-index`/
`-cli` source was changed. Source repositories are inspected read-only through the
existing typed `observe_repository`/`count_commits_since`; all writes remain under
`OMNIPROJ_HOME` (metadata via core CAS, and the gitignored `cache/r0-observation.json`).

BASE `dc3b663`, HEAD `107cb07`.

## RED → GREEN Evidence

The behavior-level test file `tests/r0_commands.rs` was written first and failed to
compile because none of the new types/commands existed (missing `DesktopService`,
`CommandError`, DTOs, `r0_invoke_handler`). After the modules were implemented the
suite compiled and passed 19/19. The single intermediate compile error surfaced during
GREEN (`RefreshOutcome` not `Copy`) was fixed by deriving `Copy`.

## GREEN Implementation

- **Boundary extraction (Step 1).** The pre-R0 backend is archived verbatim in
  `src/legacy.rs`, which is intentionally NOT declared as a module. `src/main.rs` is
  reduced to `omniproj_desktop::run()`; `run()` lives in `src/lib.rs`.
- **Fixed error contract (Step 2).** `error.rs` defines the closed `ErrorCode`
  snake_case set (all 20 required codes) and one `CommandError { code, message,
  retryable, state_applied, field?, project_id?, existing_project_id?,
  durable_revision? }`. Optional fields are omitted when absent. `audit_commit_failed`
  is the only error with `state_applied: true` and carries `durable_revision`;
  `store_write_failed` is `state_applied: false` + retryable. Typed `From`
  conversions map `ProjectStateError`, `ProjectStoreError`, `StoreError`, and
  `RepositoryReadError`; `CurrentCommitmentMismatch` splits into
  `no_current_commitment` (actual `None`) vs `current_commitment_changed`.
- **DTOs + pure assemblers (Step 3).** `dto.rs` defines the index/overview/source/
  commitment/observed-actual/transition/review DTOs and input types. Index rows exclude
  the full source path; Overview includes it. Both carry Human-state `revision` and the
  fixed `review_policy { commitment_review_days: 7, rule_version: "r0-v1" }`; sources
  carry a separate `revision`. Overview carries `last_transition` and
  `undoable_transition_id` (mirroring the core Undo guard). `commits_since_commitment`
  is present only when the cache was computed against the current commitment. Review
  reasons and Index order are sorted in core by fixed priority.
- **Last-successful observation cache (Step 4).** `repository_cache.rs` persists
  `cache/r0-observation.json` atomically (`project_id`, `source_id`, `observed_at`,
  facts, commitment-relative count). A cache belonging to a different source id (after a
  relink) is treated as absent. Success records the observation via core CAS *then*
  replaces the cache; failure records only status/error/attempt and leaves the cache
  bytes untouched, so cached facts survive. `state.rs` holds
  `refreshes: tokio::sync::Mutex<HashSet<ProjectId>>` with an RAII guard that never
  leaks a slot; a concurrent same-project refresh returns `refresh_in_progress` with the
  current cached row. A stale relink race (`LocationConflict`/`RevisionConflict` from
  `record_source_observation`) discards the stale result as `stale` without clobbering.
- **Service + clock seam (Step 5).** `service.rs` defines `Clock`, `SystemClock`,
  `DesktopService<C>`, and the exact `R0Service` trait. All source Git inspection runs
  through `tokio::task::spawn_blocking`. Archived projects are absent from the default
  Index but remain directly addressable via `get_project_overview`. A partial
  multi-project refresh returns exactly one result per project; one source failure never
  rejects the batch.
- **Command allowlist (Step 6).** `commands.rs` + `lib.rs::r0_invoke_handler()` register
  exactly the 15 approved commands, each taking one snake-case `input`.
  `complete_project_setup` calls the single core `CompleteSetup`; `save_project_framing`
  uses `SaveFraming` with no hidden activation. The notification plugin, reminder worker,
  attention count, and notification capability are removed; a neutral Open/Quit tray
  remains. `tauri-plugin-dialog` is initialized with `dialog:allow-open`, and the window
  minimum width is `640`. The behavior-level IPC test proves the deferred names
  (`advance_task`, `get_graph`, `get_plan`, `get_attention`, `test_reminder`) are
  rejected as unregistered while all 15 R0 commands are accepted at the handler boundary.

## Verification (fresh command output)

```text
cargo test -p omniproj-desktop            # 21 passed (r0_commands), lib/bin/doc 0
cargo check -p omniproj-desktop           # Finished, no warnings
cargo fmt --all --check                   # clean
cargo build --workspace --locked          # Finished
cargo test --workspace --locked           # capture 25+13, core 90+16+48+12+51,
                                          # desktop 19, distill 36 (+2 ignored), index 4+1
```

The desktop crate is clippy-clean under `cargo clippy -p omniproj-desktop --all-targets`
(no warnings reference `crates/omniproj-desktop`).

## Independent review (round 1) and fixes

An independent task-scoped review (BASE..`107cb07`) returned one Important and four Minor
findings. Resolved in `29b224f`:

- **Important — stale observed-actual after relink.** `relink_primary_git_source` keeps
  the same `source_id` but changes the location and sets `status=Available` without a fresh
  observation. The cache was keyed only on `source_id`, so the previous repository's facts
  could be shown as the current observed-actual until the next refresh. Fixed by keying the
  cache on the source `location` too: `CachedObservation` now persists `source_location`
  and `repository_cache::load` treats a location mismatch as absent. New test
  `refresh_relink_to_a_new_repo_invalidates_the_stale_cache`.
- **Minor — `transition_not_found` / `undo_not_available` never emitted.** Core collapses
  all undo failures into one `UndoConflict`. The Undo path now refines it into the distinct
  wire codes (unknown id → `transition_not_found`; nothing undoable → `undo_not_available`;
  otherwise `undo_conflict`). New test `service_undo_error_codes_are_distinguished`.
- Minor (3) `StoreError::AuditCommit` → `audit_commit_failed` without `durable_revision`
  on registry ops, Minor (4) store commits running inline on the async executor, and
  Minor (5) the theoretical `RefreshGuard::Drop` contended-path defer were judged
  acceptable-by-design for R0 and left as-is (the Human-mutation audit path carries the
  revision; registry ops have no document revision; the guard's fast path is effectively
  always taken and has no deadlock/panic risk).

The desktop suite is now 21 tests, all passing.

## Pre-existing CI clippy debt (fixed separately, user-authorized)

`cargo clippy --workspace --all-targets -- -D warnings` (the CI gate) failed at BASE
`dc3b663` inside frozen Tasks 1–4 / legacy CLI code under stable clippy 1.97.1
(`new_without_default` ×4 in the `typed_id!` macro, `collapsible_if` in `review.rs`, and
`deprecated` usage in the staged-migration CLI/distill surfaces). With the Human's explicit
go-ahead, these were fixed as a dedicated non-feature commit `d420d25` (allow-attributes +
one `if` collapse + a needless-borrow), leaving the full CI gate green. No behavior changed.

## Commits

- `107cb07` `feat(desktop): expose the R0 project re-entry API`
- `d420d25` `style: satisfy stable clippy across the workspace` (pre-existing lint debt)
- `29b224f` `fix(desktop): invalidate the observation cache on relink; distinguish undo errors`
