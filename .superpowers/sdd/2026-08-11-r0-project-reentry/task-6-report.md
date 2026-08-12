# Task 6 Report: Stable Project IDs for Capture, Index, and CLI

## Scope

Implemented Task 6 only in the assigned capture, index, CLI, and index integration-test files.
No package `Cargo.toml` changes were necessary. Source repositories are observed read-only;
all persistent index writes remain under `OMNIPROJ_HOME/projects/<ProjectId>/cache/`.

## RED Evidence

Before implementation, the pre-existing integration test was run:

```text
cargo test -p omniproj-index --test stable_project_id -- --nocapture
```

It failed at compile time because `ensure_index_for` and `search_for` did not exist,
`index_path` was private and accepted `&str`, and the production index still used
`Substrate.hash` / path-derived cache identity. This confirmed the required RED state.

## GREEN Implementation

- `capture_source(&ProjectId, &ProjectSource)` constructs a substrate with permanent
  `project_id` and observed `location`; the former `capture(&Path)` is retained only
  as a deprecated, path-derived legacy adapter.
- `ensure_index_for`, `search_for`, and public `index_path` now use `ProjectId` and
  `cache_dir_for`. Legacy `ensure_index`, `search`, and `search_project` wrappers are
  explicitly deprecated.
- Index search row decoding now uses `collect::<rusqlite::Result<Vec<_>>>()`, so a
  malformed row is returned as an error instead of being silently discarded.
- The CLI uses `register_project`, `list_project_records`, and `find_project_by_cwd`.
  `list` prints `ID`, while digest/search resolve the registered primary source and
  capture/index under `project.id`. Legacy note/plan-style document calls pass
  `project.id.as_str()`.
- `stable_project_id` registers a temporary project, indexes a session, moves and
  relinks the source, then proves that the index path and result are preserved and
  that no cache directory keyed from the moved source path is created.

## Verification

Passed:

```text
cargo test -p omniproj-index --test stable_project_id -- --nocapture
cargo test -p omniproj-capture --lib
cargo test -p omniproj-index
cargo test -p omniproj-cli
cargo fmt --all --check
cargo check --workspace --locked
git diff --check
```

`cargo check --workspace --locked` completed successfully. It emits existing staged-
migration deprecation warnings in CLI/desktop code; no warnings indicate a failed
Task 6 identity migration.

## Commits

- `e7fee8d1e9718be3c52aca10adb6f427d1a225d2` `refactor: use stable project ids across local services`

## Concerns

- The compatibility wrappers necessarily preserve path-derived behavior for deferred
  callers. R0 callers touched here use only explicit `ProjectId` identity.
- The CLI still uses pre-existing deprecated store transaction/commit helpers outside
  the identity API transition; this task did not widen scope to replace them.
