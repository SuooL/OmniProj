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
