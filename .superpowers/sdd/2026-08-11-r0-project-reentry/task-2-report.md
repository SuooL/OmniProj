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

## Correction Round 5

Code commit: `77a8c80` (`fix(core): validate recovery writes against trusted paths`).

### Changed files

- `crates/omniproj-core/src/store.rs`: validates Round-2 expected snapshots against deterministic write bytes and Git-backed v1 project baselines; adds canonical-root, no-follow ancestor validation; checks persisted audit paths during decode; guards store atomic writes before temporary-file creation and destination rename; and rechecks exact commit paths before Git add and commit.
- `crates/omniproj-core/src/project.rs`: applies checked store writes and directory guards to registration staging/final paths and all audited metadata replacements.
- `crates/omniproj-core/src/project_state.rs`: routes store-owned state writes through the checked atomic primitive while preserving the public save contract that creates an absent store root.
- `crates/omniproj-core/tests/schema_v2_migration.rs` and `project.rs` unit tests: cover wrong-but-valid Round-2 expected hashes and ancestor-symlink escapes with byte, target, journal, and index invariants.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration round2_ignore_prepared_rejects_an_expected_hash_that_disagrees_with_its_write_bytes -- --exact --nocapture` failed because recovery appended `/.migration-v2` to `.gitignore` before rejecting the false 64-hex expected hash.
2. `cargo test -p omniproj-core --test schema_v2_migration round2_projects_prepared_rejects_non_authoritative_meta_and_state_hashes_before_writes -- --exact --nocapture` failed because recovery replaced strict v1 metadata with v2 bytes before detecting the corrupted snapshot; the fixture iterates both metadata and migration-created state targets.
3. `cargo test -p omniproj-core --test schema_v2_migration migration_rejects_a_notes_ancestor_symlink_before_writing_external_state -- --exact --nocapture` failed because an external `project.md` was created through the symlinked `notes/` ancestor.
4. `cargo test -p omniproj-core --lib registration_rejects_a_staging_project_root_symlink_before_external_writes -- --nocapture` failed because registration created external `notes/project.md` through a swapped staging-root symlink.

### GREEN evidence and final verification

- Both authoritative-snapshot regressions and all five `round2_` migration tests passed after the decoder validation change.
- Both ancestor-symlink regressions passed with external sentinel bytes unchanged, no external state/metadata target, the symlink intact, and an empty relevant Git index.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 47 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 136 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check`: exit 0.
- `git diff --check`: exit 0 before the code commit and report commit.

### Design decisions and concerns

- Round-2 ignore recovery accepts an expected identity only when it hashes the persisted `pending_ignore_contents`. Project recovery reconstructs strict v1 metadata from Git history, deterministically renders v2 metadata and any migration-created state, and requires persisted prior/expected identities to match that authoritative plan before phase advance, journal rewrite, file write, or staging.
- Store targets are resolved lexically below the configured home and checked from its canonical root with `symlink_metadata`; every existing intermediate component must be a real directory. Mutation leaves remain missing or regular files, while directory operations separately require real directories or an explicitly allowed missing leaf.
- The no-follow guard runs during snapshot/decoder validation, around directory creation, before temporary-file creation and destination rename, before registration directory rename/fsync, and before Git add/commit. The audit journal itself remains under the store's real `.git` directory and is never a worktree audit target.
- The first project-state regression run exposed the prior public behavior of saving below an absent store root. The final implementation restores only that root creation at the public entry point; initialized migration and registration paths retain the stricter guarded contract.

## Correction Round 6

Code commit: `e2aa3b2` (`fix(core): validate all migration journal formats`).

### Changed files

- `crates/omniproj-core/src/store.rs`: routes tagged, Round-2, Round-1, and legacy migration journals through one pure current-representation validation pipeline before return or compatibility-journal rewrite; extends authoritative and phase-state validation to schema phases; and makes project enumeration reject any valid ProjectId entry that is not a real directory.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: covers tagged prepared journals with wrong-but-valid expected hashes, an external v1 project behind a valid-ID symlink root, and a valid-ID regular file discovered by the pre-stamp rescan.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration tagged_ignore_prepared_rejects_a_non_authoritative_expected_hash_before_writes -- --exact --nocapture` failed because recovery appended `/.migration-v2` to `.gitignore` before rejecting the false tagged expected hash.
2. `cargo test -p omniproj-core --test schema_v2_migration tagged_projects_prepared_rejects_non_authoritative_expected_hashes_before_writes -- --exact --nocapture` failed because recovery replaced strict v1 metadata with v2 bytes before rejecting the false tagged expected hash; the fixture iterates metadata and migration-created state targets.
3. `cargo test -p omniproj-core --test schema_v2_migration migration_rejects_a_valid_project_id_symlink_root_without_following_it -- --exact --nocapture` failed because `unwrap_err()` received `Ok(home)`, proving the external legacy project behind the valid-ID symlink did not block migration.
4. `cargo test -p omniproj-core --test schema_v2_migration migration_rescan_rejects_a_valid_project_id_regular_file_before_mutation -- --exact --nocapture` failed because the pre-stamp rescan skipped the project-shaped regular file and completed migration.

### GREEN evidence and final verification

- All four focused regressions passed. Invalid tagged fixtures retain target, journal, schema, and index bytes; the symlink/file fixtures retain external or Human bytes, symlink type, migration journal, schema 1, and an empty index.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 51 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 140 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check`: exit 0.
- `git diff --check`: exit 0 before the code commit and report commit.

### Design decisions and concerns

- Structural validation remains an early pre-normalization gate, but every successfully decoded or upgraded journal then passes one side-effect-free pipeline over the normalized current representation: exact shape/phase/target set, safe target paths and leaf types, deterministic authoritative snapshots, and actual phase state. Compatibility upgrades write their normalized journal only after this pipeline succeeds.
- Authoritative validation now applies uniformly to tagged and flat SHA formats. Ignore targets must hash `pending_ignore_contents`; project prior/expected identities must match strict Git-backed v1 inputs and deterministic v2 bytes; schema prior/expected identities must match Git history and literal `2\n`.
- Project enumeration obtains no data through a symlink. Invalid/tool-reserved entry names retain their prior behavior, while any entry whose name parses as a ProjectId must be a real directory or migration returns a typed conflict. The same function is used for initial discovery and every pre-stamp rescan.

## Correction Round 7

Code commit: `aa653fa8bbafe8f659813c4d3ceb68ede993bbcd` (`fix(core): prove migration audit milestones`).

### Changed files

- `crates/omniproj-core/src/store.rs`: adds a shared, side-effect-free phase-milestone proof after structural, authoritative-snapshot, and snapshot-state validation; proves ignore, project, and schema worktree/HEAD states before any recovery transition; and upgrades no-phase legacy journals to `projects_audited` only when deterministic outputs and `HEAD` prove that milestone.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: covers tagged/Round-2 false `projects_audited` journals over both untouched v1 and an ignore-only audited store, false `ignore_audited`, three false schema-audited states, and legitimate audited-phase crash recovery.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration tagged_and_round2_projects_audited_require_proven_project_outputs -- --exact --nocapture` failed with `tagged: rejected=false, unchanged=false; round2: rejected=false, unchanged=false`; false audited journals advanced and changed an unproved v1 store.
2. `cargo test -p omniproj-core --test schema_v2_migration ignore_audited_requires_the_expected_ignore_bytes_to_be_committed -- --exact --nocapture` failed because `unwrap_err()` received `Ok(home)` when the expected `.gitignore` bytes existed only in the worktree.
3. `cargo test -p omniproj-core --test schema_v2_migration tagged_schema_audited_requires_exact_schema_bytes_in_worktree_and_head -- --exact --nocapture` failed with `uncommitted: rejected=false, unchanged=false; modified: rejected=false, unchanged=false`; the missing-schema table case was already rejected by the existing stamp/phase guard.

### GREEN evidence and final verification

- The three negative regressions pass with target, journal, schema, and index unchanged; the positive project/schema audited failpoint fixtures converge and remove the journal.
- The first full migration run caught one old no-phase compatibility regression (38/39). Its upgrade now promotes an all-expected project state only after ignore and `HEAD` proof; the exact regression and all Round-1/Round-2 fixtures then passed.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 55 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- Final `cargo test -p omniproj-core --tests`: 144 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check` and `git diff --check`: exit 0.

### Design decisions and concerns

- `journal_created` requires Git-backed project priors and the prior schema; `ignore_audited` additionally requires the marker-bearing `.gitignore` to match `HEAD` while allowing each project target to be either prior or expected for partial-loop recovery.
- `projects_audited` requires the exact project set plus every deterministic migrated metadata/migration-created state output in both worktree and `HEAD`; `schema_audited` additionally requires literal `2\n` in both worktree and `HEAD`. Schema prepared/written phases retain their prior-or-expected/expected crash windows.
- Non-created Human project-state files remain outside the migration audit target set and are never staged or rewritten; they must still parse as the canonical pre-existing setup state. Compatibility normalization conflicts on mixed or otherwise unprovable states rather than guessing.

## Fresh Correction Round 8

Code commit: `61ccdeda77394631ebb22eac2c9df1970b8eb3f5` (`fix(core): preserve legacy canonical human state`).

### Changed files

- `crates/omniproj-core/src/store.rs`: classifies an existing project state under an exact two-field/no-phase journal and schema 1 as non-created only when it strictly parses to the deterministic setup document and is byte-identical to its canonical rendering; journal-created recovery no longer requires that Human state to match `HEAD`, while schema 2 and inferred `projects_audited` recovery retain explicit `HEAD` proof.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: covers the real Git schema-1 store with an exact old journal, pre-existing untracked canonical state, and unrelated Human bytes; also tightens the former ambiguous fixture into a parse-equivalent but noncanonical-byte near miss.

### RED evidence

- `cargo test -p omniproj-core --test schema_v2_migration legacy_journal_preserves_preexisting_untracked_canonical_project_state -- --exact --nocapture` failed 0/1 because `ensure_home()` returned `MigrationConflict` for the untracked canonical `notes/project.md`.

### GREEN evidence and final verification

- The focused canonical-state and noncanonical near-miss tests both passed; the full migration suite passed 40/40.
- The canonical state and unrelated Human file remain byte-identical and untracked, and the project audit commit contains only migrated `meta.toml`.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 56 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 145 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check` and `git diff --check`: exit 0.

### Design decisions and concerns

- Exact canonical bytes are the classification boundary: semantic parse equality alone is insufficient, so alternate whitespace or any other byte difference remains a typed `MigrationConflict` and is not rewritten.
- The relaxation applies only while upgrading the no-phase compatibility journal against schema 1 with project metadata still at its prior identity. If metadata proves the project audit already occurred, every project state must still match `HEAD`; schema-2 compatibility recovery is unchanged.
- Non-created canonical state is excluded from `created_state_ids` and all migration audit targets, preserving its original tracking semantics.

## Fresh Correction Round 9

Code commit: `f676075751fb8a80d34c1e9ad07e9e310d1d4dfa` (`fix(core): persist preserved migration state proofs`).

### Changed files

- `crates/omniproj-core/src/store.rs`: persists one strict `preserved_state_proofs` entry for every non-created project state, binding the project id to its deterministic canonical regular-file identity and the compatibility recovery's `HEAD` requirement; validates the exact proof set, worktree bytes, parse result, authoritative deterministic identity, and optional `HEAD` bytes on every retry before mutation; and adds a deny-unknown decoder for the immediately prior tagged format.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: adds two normalization/interruption/retry sequences, safe and ambiguous prior-current compatibility cases, malformed proof-set cases, and updates the exact Round-2 fixture conversion to omit the new current-only field.

### RED evidence

1. `cargo test -p omniproj-core --test schema_v2_migration normalized_legacy_journal_retains_exact_untracked_state_proof_across_retries -- --exact --nocapture` failed because retry returned `Ok(home)` after the normalized journal's Human state was replaced by parse-equivalent but noncanonical bytes.
2. `cargo test -p omniproj-core --test schema_v2_migration normalized_audited_legacy_journal_retains_state_head_proof_across_retries -- --exact --nocapture` failed because retry returned `Ok(home)` after an initially `HEAD`-proven canonical state was removed from tracking and committed out of `HEAD`.

### GREEN evidence and final verification

- Both multi-start regressions pass with typed `MigrationConflict` and byte-identical journal, state, metadata, schema, and index snapshots on retry.
- The prior tagged current journal upgrades only from verifiable state; an audited/later missing-proof journal with an untracked non-created state remains ambiguous and conflicts. Missing, duplicate, wrong-kind, and unknown-field proof fixtures are rejected before rewrite.
- The full migration suite passed 45/45, including tagged, Round-2, Round-1, and two-field/no-phase compatibility.
- `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`: 61 passed, 0 failed.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 150 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check` and `git diff --check`: exit 0.

### Design decisions and concerns

- `preserved_state_proofs` must be unique and exactly cover `project_ids - created_state_ids`; each entry is path-bound through its typed project id, permits only a regular-file identity, and re-derives the authoritative identity from Git-backed v1 metadata plus the deterministic setup renderer.
- Proofs created for schema-1 prior/journal-created Human state require exact worktree bytes but not `HEAD`. Schema 2 and compatibility phases at or beyond `projects_audited` require both exact worktree and `HEAD` bytes, and that requirement survives every normalized retry.
- The immediately prior tagged format is distinguishable only by the absent proof field. Compatibility first validates its old base shape, then derives proofs from current evidence, runs the complete current validation pipeline, and only then rewrites. Audited/later provenance that cannot prove `HEAD` is rejected rather than downgraded.

## Fresh Correction Round 10

Code commit: `eacd19055bd5c4a9d70961b08ae94ff26f0cdc98` (`fix(core): derive migration state proof policy`).

### Changed files

- `crates/omniproj-core/src/store.rs`: treats persisted `head_required` as a claim rather than authority; derives the authoritative policy from canonical state plus strict v1 metadata in Git history before the migration marker boundary; recognizes the migration's own missing-state/v1-meta to canonical-state/v2-meta commit as created state; and applies the same derivation to current validation, prior-current compatibility, legacy no-phase upgrades, new journals, and project rescans.
- `crates/omniproj-core/tests/schema_v2_migration.rs`: adds multi-start tamper regressions for both `true -> false` and `false -> true`, including post-normalization Git history changes and pre-retry snapshots of the journal, state, metadata, schema, and index.

### RED evidence

1. `tampered_false_head_policy_cannot_disable_audited_state_proof` failed because retry returned `Ok(home)` after the journal policy was changed to `false` and the previously tracked state was committed out of `HEAD`.
2. `tampered_true_head_policy_cannot_invent_untracked_state_provenance` failed because retry returned `Ok(home)` after an untracked state's policy was changed to `true` and the state was committed only after normalization.

### GREEN evidence and final verification

- Both tamper regressions now return typed `MigrationConflict` before rewrite and preserve all captured bytes and index state.
- Legitimate untracked `false`, audited tracked `true`, prior-current safe upgrade/ambiguous rejection, and all legacy no-phase interruption recoveries remain green.
- The full migration suite passed 47/47; migration plus registry passed 63/63.
- `cargo test -p omniproj-core --lib project_state -- --nocapture`: 9 passed, 0 failed.
- `cargo test -p omniproj-core --lib`: 89 passed, 0 failed.
- `cargo test -p omniproj-core --tests`: 152 passed, 0 failed across unit and integration suites.
- `cargo check --workspace`: exit 0; only the existing staged-migration deprecation warnings remain.
- `cargo fmt --all --check` and `git diff --check`: exit 0.

### Design decisions and concerns

- The first committed addition of `/.migration-v2` to `.gitignore` is the provenance boundary. Only a strict ancestor containing exact deterministic state bytes alongside matching strict v1 metadata can authorize `head_required = true`; later commits cannot manufacture pre-migration provenance.
- A persisted policy that differs from this derivation conflicts regardless of whether the stronger or weaker value would currently pass. Current `HEAD` is used only to enforce an authoritatively derived `true`, never to derive the policy.
- Multiple marker-addition histories, merge-shaped boundary commits, noncanonical pre-boundary state, and contradictory tracked/created evidence are rejected conservatively as migration conflicts.
