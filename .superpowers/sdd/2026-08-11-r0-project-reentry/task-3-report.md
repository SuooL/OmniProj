# Task 3 Report: Human project state and commitment lifecycle

## Result

Implemented the one-file Human project-state aggregate and its auditable command lifecycle.
Only `projects/<id>/notes/project.md` is mutated. The implementation reuses Task 2's
recoverable exact-path audit journal and does not modify `store.rs`.

Code commit: `2680c327304242da76df63dc34782c8f93ac1e72` (`feat(core): add auditable commitment state machine`)

## Implementation

- Extended strict parsing with aggregate project-ID consistency, time ordering, transition
  reference/shape checks, correction-target rules, and stored-pointer/effective-history replay.
- Added the single `apply_project_command` mutation boundary with revision compare-and-swap,
  in-memory pre-state cloning, one accepted-command revision increment, atomic state replacement,
  and exact-path audit recovery.
- Added framing/setup, commitment Set/Confirm/Complete/Replace/Clear, project status changes,
  and append-only compensating Undo.
- Kept the project pointer separate from WorkItem status: Replace and Clear do not alter the
  prior item; Complete changes it to Done; compensating Undo restores the R0 status semantics.
- Exported only the core command, mutation, and state-machine types needed by later tasks.

## TDD evidence

### Parser and aggregate invariants

- RED: `cargo test -p omniproj-core --test project_state_lifecycle parse_ -- --nocapture`
  produced 3 passed / 2 failed because pointer/history mismatch and invalid correction targets
  were accepted.
- A second boundary RED produced 6 passed / 2 failed because mixed embedded project IDs,
  transition times after `updated_at`, and load-path/project-ID mismatch were accepted.
- GREEN: the same focused command produced 8 passed / 0 failed.

### Lifecycle matrix

- RED: `cargo test -p omniproj-core --test project_state_lifecycle lifecycle_ -- --nocapture`
  failed compilation with 10 expected missing-API/typed-error errors before command
  implementation.
- GREEN: the same focused command produced 15 passed / 0 failed.
- The audit-hook case proves a failed Git audit returns
  `AuditCommitFailed { durable_revision: 1, .. }` while revision 1 is readable from disk;
  after removing the hook, `ensure_home()` completes the pending exact-path audit.

### Undo

- RED: after removing the provisional Undo branch, `cargo test -p omniproj-core --test
  project_state_lifecycle undo_ -- --nocapture` produced 0 passed / 7 failed because Undo was
  unimplemented.
- GREEN: the same command produced 7 passed / 0 failed for Set, Confirmed, Completed, Replaced,
  Cleared, older-transition conflict, and correction-not-undoable behavior.

### Legacy preservation

- GREEN: `cargo test -p omniproj-core --test project_state_lifecycle
  preserves_legacy_documents -- --nocapture` produced 1 passed / 0 failed after executing all
  nine R0 command variants. Hand-authored `next.md` and `plan.md` stayed byte-identical.
- A separate mutation test preserves project Markdown body CRLF, trailing spaces, and unknown
  content byte-for-byte.

## Fresh verification

- `cargo test -p omniproj-core --lib --tests`: PASS
  - library: 89 passed
  - project source registry: 16 passed
  - project state lifecycle: 32 passed
  - schema v2 migration: 51 passed
- `cargo check --workspace`: PASS; only pre-existing deprecated API warnings in CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Self-review and concerns

- Confirmed only the three Task 3 code/test files are in the code commit; no legacy document,
  plan, ledger, or `store.rs` change is present.
- A post-write audit error intentionally does not roll back Human state; the error reports the
  durable revision and the Task 2 pending-audit journal makes the exact commit recoverable.
- The pre-commit-hook audit failure test is Unix-only because it requires executable hook
  permissions; all non-hook lifecycle semantics remain cross-platform.
- Archived-project Index exclusion belongs to the later Index assembler; this task persists the
  typed Archived status and tests that boundary only.
- R0 Undo restores Completed items to Doing and marks items introduced by an undone Set/Replace
  as Abandoned. If a future release adds independent WorkItem-status commands, the transition
  schema will need enough prior-status data to restore states beyond the R0 command model.

