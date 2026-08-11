# Task 2 report — schema-v2 registry and project-state persistence foundation

## Status

Implemented the explicit, recoverable schema-v1→v2 migration; strict one-file project-state codec; typed project/source registry; atomic staged registration; relink and source-observation compare-and-swap operations; and deprecated legacy adapters required by the staged workspace migration.

Implementation commit: `6c464420f68055d6302ae2669bf3e0d44786844c` (`feat(core): separate projects from repository sources`).

## Changed files

- `crates/omniproj-core/src/project.rs`: added the v2 project/source envelope, typed registry errors and inputs, strict record loading, canonical owner lookup, locked duplicate-safe registration, atomic staging rename, relink and observation CAS, exact-path audits, cwd lookup, and deprecated v1 adapters.
- `crates/omniproj-core/src/project_state.rs`: added the schema-1 front-matter types, strict parser and invariants, deterministic renderer, exact Markdown-body preservation, typed not-found/errors, and atomic load/save foundation.
- `crates/omniproj-core/src/paths.rs`: added typed project/auto/notes/cache path helpers while preserving string adapters.
- `crates/omniproj-core/src/store.rs`: set store schema 2; added the strict private v1 decoder, `.migration-v2` journal, resumable migration, exact audit commits, failpoints, typed conflicts, generic checked transactions, and conservative empty-staging cleanup.
- `crates/omniproj-core/src/lib.rs`: exported the Task 2 public API and project-state types.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: added real-Git migration, byte preservation, idempotence, six failpoint recovery, conflict, and audit-path coverage.
- `crates/omniproj-core/tests/project_source_registry.rs`: added envelope, registration, duplicate, read-only owner lookup, failpoint, staging cleanup, relink, cwd, collision, observation, CAS, Human-byte preservation, and exact-audit coverage.

No `Cargo.toml` or `Cargo.lock` changes were required.

## TDD evidence

### RED

1. `cargo test -p omniproj-core --test schema_v2_migration -- --nocapture` failed on the first behavior fixture with actual schema bytes `"1\n"` versus expected `"2\n"`.
2. `cargo test -p omniproj-core --test project_source_registry -- --nocapture` failed to compile because `CaptureCursor`, `ProjectRecord`, `ProjectSource`, `ProjectSourceKind`, and `ProjectSourceStatus` did not exist; the minimal envelope then made the test green.
3. `cargo test -p omniproj-core project_state::tests -- --nocapture` failed to compile because `ProjectStateDoc` and `ProjectStateError` did not exist; the strict codec then made six initial behaviors green.
4. The expanded migration suite failed to compile because `load_project` and `StoreError::MigrationConflict` were absent before the recoverable migration implementation.
5. The registry-operation suite failed to compile because registration, owner lookup, relink, observation, cwd lookup, their input/outcome types, and duplicate/revision errors were absent before implementation.
6. The legacy-adapter assertion failed with `list_projects().len() == 0` after v2 registration; projecting adapters from `ProjectRecord` made it green.
7. The startup cleanup test failed first for an empty staging directory and again for a standard empty staging skeleton; recursive file-emptiness checking now removes only recognized empty staging trees and preserves nonempty/unrecognized trees.
8. Full-lib regression initially exposed non-exhaustive Task 1 test matches after adding typed store errors, then a placeholder `.git` fixture that could not support checked migration commits. The minimal fixes made the matches future-safe and changed the fixture to a real Git v1 store.

### GREEN

- `cargo test -p omniproj-core --test schema_v2_migration -- --nocapture`: 3 passed, 0 failed.
- `cargo test -p omniproj-core project_state::tests -- --nocapture`: 8 passed, 0 failed.
- `cargo test -p omniproj-core --test project_source_registry -- --nocapture`: 8 passed, 0 failed.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 11 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 77 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 88 passed, 0 failed across unit and integration suites.
- `cargo fmt --all --check`: exit 0.
- `cargo check --workspace`: exit 0; deferred CLI/desktop callers compile through the explicitly deprecated adapters.
- `git diff --check`: exit 0 before the implementation commit.

## Self-review

- Migration writes the journal before project mutation, resumes it before accepting schema 2, strictly recognizes v1 or v2 metadata, never rewrites legacy `next.md`, `plan.md`, or `auto/briefing.md`, and conflicts on unrecognized `notes/project.md` without stamping schema 2.
- Migration audit commits contain only migrated `meta.toml`/new `notes/project.md` paths and then only `SCHEMA_VERSION`; failed checked commits leave the ignored journal for retry.
- Registration canonicalizes an existing readable directory without Git inspection, repeats duplicate detection under the checked lock, generates UUID v7 IDs only after that check, fsyncs state/metadata in `projects/.staging-<id>`, and exposes the project only through one parent-directory rename.
- Relink and observation both reload under lock and compare source revision plus location. They increment the source revision once, atomically replace metadata, commit only that metadata path, and never touch Human files.
- `project.rs` contains no subprocess or Git-source inspection. Store Git calls remain limited to the existing audit boundary.
- Project-state parsing rejects delimiter/schema/timestamp/duplicate/dangling-reference errors and preserves all Markdown bytes after the closing delimiter.

## Concerns

- Workspace compilation intentionally reports deprecation warnings for CLI/desktop callers that still use `ProjectMeta`, `load_meta`, `list_projects`, `register`, `store_txn`, or `commit_all`; later staged tasks migrate those callers. Compilation succeeds.
- A failure after a staging file is written intentionally leaves a non-enumerable, nonempty staging tree. Startup removes only recognized staging trees with no files, as required; a later operational cleanup policy may address retained forensic partials.

## Fix Round 1

Fix commit: `8f7e36f` (`fix(core): make project mutations recoverable`).

### Changed files

- `crates/omniproj-core/src/store.rs`: validates the schema stamp before considering a migration journal; replaces the one-shot journal with explicit durable phases; retries `.gitignore`, project-path, and schema audits; rescans the project set before stamping; records only migration-created state paths; recovers exact-path pending audits; and runs staging cleanup under the checked store lock.
- `crates/omniproj-core/src/project.rs`: strictly validates v2 identifiers/timestamps/nonempty/coherence invariants; records recoverable audits before registration rename and metadata replacement; keeps CAS mutations non-replayable; and makes the legacy cursor adapter reload/update/audit under the store lock.
- `crates/omniproj-core/src/project_state.rs`: requires persisted `work_items` and `commitment_transitions` fields instead of silently defaulting absent collections.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: adds stale-journal stamp protection, real audit-failure retry, project rescan, malformed-resume, and precise state-audit regressions.
- `crates/omniproj-core/tests/project_source_registry.rs`: adds strict-record and real Git audit-failure recovery regressions for registration, relink, and observation. Unit regressions use one-shot barriers for deterministic cleanup/registration and cursor/relink interleavings.

No `Cargo.toml` or `Cargo.lock` changes were required.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration stale_journal_cannot_downgrade_a_newer_or_malformed_schema_stamp -- --exact`: `unwrap_err()` received `Ok`; a stale journal bypassed the v3/malformed stamp.
2. `cargo test -p omniproj-core --test schema_v2_migration migration_retries_gitignore_audit_after_a_real_commit_failure -- --exact`: failed because `.migration-v2` had not been created before the rejected `.gitignore` commit.
3. `cargo test -p omniproj-core --test schema_v2_migration migration_rescans_projects_added_after_journal_creation -- --exact`: the added project remained v1 after the store stamped v2.
4. `cargo test -p omniproj-core --test schema_v2_migration migration_resume_rejects_malformed_v2_source_metadata -- --exact`: malformed source `created_at` was accepted.
5. `cargo test -p omniproj-core --test schema_v2_migration migration_audits_only_project_state_created_by_that_migration -- --exact`: `HEAD^` incorrectly included pre-existing `notes/project.md`.
6. `cargo test -p omniproj-core --test project_source_registry loading_v2_metadata_rejects_duplicate_sources_bad_timestamps_and_incoherent_fields -- --exact`: duplicate source ID was accepted.
7. `cargo test -p omniproj-core project_state::tests::parser_requires_persisted_collection_fields -- --exact`: a document missing `work_items` was accepted.
8. `cargo test -p omniproj-core --test project_source_registry audit_failure -- --nocapture`: all three real-hook cases failed; no pending registration audit existed and relink/observation retries left the earlier registration commit at `HEAD`.
9. `cargo test -p omniproj-core project::tests::startup_cleanup_cannot_delete_an_active_registration_skeleton -- --exact --nocapture`: startup returned `Ok` while registration held the lock and deleted its empty skeleton.
10. `cargo test -p omniproj-core project::tests::legacy_cursor_update_cannot_overwrite_a_concurrent_source_relink -- --exact --nocapture`: final source revision was `0` instead of `1`, proving stale whole-record overwrite.

### GREEN evidence and final verification

- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 20 passed, 0 failed.
- `cargo test -p omniproj-core project_state::tests -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 80 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 100 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the already-expected staged-migration deprecation warnings remain.
- `cargo fmt --all --check`: exit 0.
- `git diff --check`: exit 0 before the fix commit.

### Design decisions and concerns

- Migration recovery uses a small enumerated phase protocol, including `schema_stamp_pending`, so crashes on either side of the cross-file stamp transition have an explicit legal resume state. A phase/stamp mismatch is rejected rather than guessed.
- The mutation audit journal lives below the store's own `.git` directory, contains only a message and validated store-relative paths, and is removed only after the exact-path commit succeeds. Recovery audits durable state but never replays the mutation, so registration retries return `Existing` and stale relink/observation retries remain CAS conflicts.
- Store-lock contention remains a checked error (`WouldBlock`) rather than implicit waiting. This is existing `with_store_txn` behavior and prevents startup cleanup from crossing an active registration.
- Legacy nonempty staging trees remain intentionally preserved as forensic partials, matching the original Task 2 boundary.

## Fix Round 2

Fix commit: `63741e268acd25d6203631e549f2785f51c58a4d` (`fix(core): harden store recovery protocols`).

### Changed files

- `crates/omniproj-core/src/store.rs`: adds strict legacy/Round-1 migration-journal decoders, verifiable legacy upgrade, SHA-256 prior/expected audit snapshots, typed `AuditConflict`, explicit ignore/project write-prepared phases, pending-audit prepared/applied recovery, one-lock fresh/existing startup, and exact-path initialization commits.
- `crates/omniproj-core/src/project.rs`: supplies desired mutation snapshots before metadata replacement/registration rename, marks applied mutations only after durable exposure, and adds deterministic rename/parent-fsync failpoints.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: covers three verifiable legacy interruption states, ambiguous/malformed legacy journals, same-path project metadata and `.gitignore` Human edits, and snapshot-safe retry.
- `crates/omniproj-core/tests/project_source_registry.rs`: covers same-path registration-state conflicts plus prepared registration recovery before and after directory rename.

No `Cargo.toml` or `Cargo.lock` changes were required.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration legacy_migration_journals_resume_from_verifiable_interruption_states -- --exact --nocapture`: the first old-format journal failed with missing `phase`; the companion ambiguous-state test returned generic `InvalidData` rather than `MigrationConflict`.
2. `cargo test -p omniproj-core --test project_source_registry registration_audit_recovery_rejects_a_same_path_human_edit_before_git_add -- --exact --nocapture`: `ensure_home().unwrap_err()` received `Ok`, proving recovery restaged and committed the Human replacement.
3. `cargo test -p omniproj-core --test schema_v2_migration migration_audit_recovery_rejects_a_same_path_human_edit_before_git_add -- --exact --nocapture`: `ensure_home().unwrap_err()` received `Ok`, proving a valid Human metadata rename was committed.
4. `cargo test -p omniproj-core --test schema_v2_migration migration_gitignore_recovery_rejects_a_same_path_human_edit_before_git_add -- --exact --nocapture`: retry returned `Ok` and included the Human `.gitignore` edit.
5. `cargo test -p omniproj-core store::tests::fresh_initialization_holds_the_store_lock_before_git_becomes_visible -- --exact --nocapture`: the concurrent startup returned `Ok` instead of checked `WouldBlock`.
6. `cargo test -p omniproj-core store::tests::fresh_initialization_commits_only_tool_created_paths -- --exact --nocapture`: initial `HEAD` contained pre-existing `Human.md`.
7. The two exact registry tests for `registration_directory_rename_failure` and `registration_parent_fsync_failure` both failed because registration returned success; the failpoints and prepared/applied distinction did not exist.

### GREEN evidence and final verification

- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 28 passed, 0 failed.
- `cargo test -p omniproj-core project_state::tests -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 82 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 110 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check`: exit 0.
- `git diff --check`: exit 0 before the fix commit.

### Design decisions and concerns

- Old two-field journals are decoded only by an exact `deny_unknown_fields` legacy shape. Recovery compares project metadata/state against `HEAD` and deterministic v2 bytes; a generated state indistinguishable from pre-existing untracked state is deliberately a typed `MigrationConflict` rather than a guess.
- Audit snapshots contain validated relative paths plus prior/expected SHA-256 identities. Recovery validates the worktree before any `git add`; changed targets remain unstaged and produce `AuditConflict`. Migration also validates each phase's exact allowed path set.
- Pending audits use `prepared` and `applied`. A prepared mutation matching all prior identities is cleared without replay; matching all expected identities is promoted and audited; any mixed/unknown state conflicts. Registration rename failure therefore clears safely, while parent-fsync failure recognizes the already-visible complete directory.
- `ensure_home` now acquires the checked store lock before the fresh/existing decision. Its locked helper performs init, schema migration, pending recovery, and cleanup without nested lock acquisition. Initial Git audit stages only tool-created `.gitignore` (when absent) and `SCHEMA_VERSION`.
- The pending journal remains inside `.git`, so it is outside all worktree staging. Existing nonblocking lock semantics and retained nonempty forensic staging trees are unchanged.

## Fix Round 3

Fix commit: `8b4498c748b4fae06bb9b356f93299069311e426` (`fix(core): secure initialization and migration recovery`).

### Changed files

- `crates/omniproj-core/src/store.rs`: puts fresh initialization under the pending-audit prepared/applied protocol; recovers deterministic partial initialization before schema dispatch; adds schema write-prepared/written snapshots; upgrades verifiable Round-1 phases from Git-backed v1 inputs; replaces content-only hashes with tagged missing/regular/directory/symlink identities; and rejects non-regular mutation targets before any replacement or staging.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: adds schema same-path conflict and write/phase crash coverage, Round-1 project/schema phase recovery and Human-v2 conflict coverage, plus dangling symlink, same-bytes symlink, and directory-target regressions.
- `crates/omniproj-core/src/store.rs` unit tests: add a real fresh-init commit-hook failure recovery test and deterministic prepared-window failpoints both after the `.gitignore` write and after the schema write.

No `Cargo.toml` or `Cargo.lock` changes were required.

### RED evidence

1. `cargo test -p omniproj-core --lib fresh_initialization_recovers_an_exact_audit_after_commit_failure -- --nocapture`: failed because `.git/omniproj-pending-audit.toml` did not exist after the rejected initial commit.
2. `cargo test -p omniproj-core --test schema_v2_migration schema_stamp_write_before_phase_advance_is_recoverable -- --exact --nocapture`: the new write-before-phase failpoint did not interrupt migration; `ensure_home()` unexpectedly returned `Ok`.
3. `cargo test -p omniproj-core --test schema_v2_migration schema_audit_recovery_rejects_same_path_human_bytes_before_git_add -- --exact --nocapture`: `ensure_home().unwrap_err()` received `Ok`; parseable Human bytes `"2 \n"` were accepted and committed.
4. `cargo test -p omniproj-core --test schema_v2_migration round1_ -- --nocapture`: `projects_written` failed with an invalid empty target set, while `ignore_audited` plus a valid Human v2 rename unexpectedly returned `Ok` and was audited.
5. `cargo test -p omniproj-core --test schema_v2_migration migration_rejects_ -- --nocapture`: all three file-type regressions failed; dangling and same-bytes symlinks were replaced successfully, and a directory produced a non-typed I/O error.
6. `cargo test -p omniproj-core --lib fresh_initialization_recovers_a_prepared_partial_write -- --nocapture`: first failed because the post-`.gitignore` failpoint did not exist, then failed at the post-schema/pre-applied window until both prepared states used deterministic recovery.

### GREEN evidence and final verification

- `cargo test -p omniproj-core --test schema_v2_migration`: 22 passed, 0 failed.
- `cargo test -p omniproj-core --test project_source_registry`: 15 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 84 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 121 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --check`: exit 0.
- `git diff --check`: exit 0 before the code commit and again before report finalization.

### Design decisions and concerns

- Fresh initialization records its exact tool-created targets before either write. The normal applied path uses the same pending-audit recovery as registry mutations. The narrowly identified fresh-init prepared path can reconstruct only the fixed initialization bytes, validates every target as prior-or-expected, and then exact-commits; it never stages or rewrites pre-existing Human paths.
- Schema stamping is now a one-target `schema_write_prepared`/`schema_written` snapshot transition. Both a crash after writing `2\n` but before phase advance and an audit-hook failure converge; any alternate bytes, even parseable `2`, produce `AuditConflict` before `git add`.
- Round-1 recovery does not derive expected output from current v2 worktree bytes. It finds a strict v1 record in that path's Git history, runs the deterministic converter, validates prior/expected identities, and for already-audited phases also requires `HEAD` to match expected output. A valid Human v2 edit therefore remains unstaged and conflicts.
- Snapshot identity uses `symlink_metadata`: `missing`, `regular_file { sha256 }`, `directory`, or `symlink { target }`. Mutation snapshots allow only missing/regular prior states and regular expected states; special files are typed conflicts, and symlink targets are never read or overwritten.
- Pending journals remain below `.git`; all commits still stage exact validated relative paths. Store startup retains one nonblocking lock with no nested acquisition. No new external dependency or future-facing abstraction was introduced.

## Correction Implementer Round 4

Code commit: `b32ac6d` (`fix(core): preserve recovery journal compatibility`).

### Changed files

- `crates/omniproj-core/src/store.rs`: adds exact `deny_unknown_fields` Round-2 SHA snapshot decoders for migration and pending-audit journals; maps absent/regular SHA priors to tagged identities; validates phase state before recovery; rejects persisted special-file priors; and preflights fresh-init schema/ignore paths before Git initialization.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: covers all four Round-2 snapshot-bearing migration phases, malformed/ambiguous fixtures, symlink-safe compatibility, fresh-init special-file handling through store unit tests, and tampered migration/pending special-prior journals.
- `crates/omniproj-core/tests/project_source_registry.rs`: covers Round-2 pending-audit recovery from both `prepared` and `applied` fixtures.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration round2_snapshot_migration_journals_resume_from_every_write_phase -- --exact --nocapture` failed at `ignore_write_prepared` with `InvalidData(... is malformed or does not match a supported migration journal format)`.
2. `cargo test -p omniproj-core --test project_source_registry round2_pending_audits_resume_from_prepared_and_applied -- --exact --nocapture` failed at `prepared` because `expected_sha256` was unknown to the tagged-only decoder.
3. `cargo test -p omniproj-core --lib fresh_initialization_rejects_a_preexisting_schema_before_git_init -- --nocapture` failed because `unwrap_err()` received `Ok(home)` after a pre-existing `SCHEMA_VERSION = 9` was overwritten.
4. `cargo test -p omniproj-core --test schema_v2_migration persisted_ -- --nocapture` failed all three tests: a schema symlink prior was accepted and replaced, a project directory prior reached later runtime validation instead of decode rejection, and a pending directory prior was accepted and its journal cleared.

### GREEN evidence and final verification

- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 44 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 88 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 132 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check`: exit 0.
- `git diff --check`: exit 0 before the code commit.

### Design decisions and concerns

- Round-2 compatibility accepts only the exact flat `relative_path`/`prior_sha256`/`expected_sha256` shape. A missing prior maps to `Missing`; a valid prior hash maps to `RegularFile`. Hash syntax, duplicate/relative paths, exact migration target sets, phase coherence, and actual prior/expected state are still checked before recovery. Mixed tagged/flat or malformed fixtures remain untouched and unstaged.
- The four Round-2 phases that persisted snapshots (`ignore_write_prepared`, `ignore_written`, `projects_write_prepared`, and `projects_written`) are covered directly. Its schema phases had no snapshot targets and remain handled by the existing strict legacy-phase upgrade.
- Fresh initialization now requires `SCHEMA_VERSION` to be absent before `git init`. A regular pre-existing `.gitignore` is preserved and excluded from the initial audit; symlink/directory schema or ignore paths conflict without following or replacing them. A real commit-hook failure/retry proves the Human `.gitignore` is not included in the tool commit.
- Every persisted mutation/audit target now requires a `Missing` or `RegularFile` prior and a `RegularFile` expected identity. Tagged directory/symlink priors and a Round-2 hash that currently names a symlink are rejected before writes or `git add`. Pending journals remain inside `.git`; the one-lock recovery flow and exact-path audit boundary are unchanged.
