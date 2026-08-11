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

## Fix Round 1

Code commit: `501d0a22637291120610d2eabf973562b5d7591d`
(`fix(core): enforce fresh commitment undo receipts`)

### Reviewer findings and TDD evidence

- Undo freshness RED: `cargo test -p omniproj-core --test project_state_lifecycle
  undo_freshness_ -- --nocapture` produced 0 passed / 5 failed because Set, Confirm,
  Complete, Replace, and Clear receipts remained undoable after a later framing revision.
  GREEN: 5 passed / 0 failed after persisting each transition's accepted
  `document_revision` and requiring the Undo target revision to equal the current document
  revision.
- Correction validation RED: the focused `parse_` suite produced 8 passed / 2 failed because
  transition document revisions were not represented and non-adjacent/forged corrections were
  accepted. GREEN: 10 passed / 0 failed after strict revision alignment, static validation of
  every original transition, direct-adjacency rules, and exact compensation-pointer replay.
- Setup gate RED: the focused setup-status test produced 0 passed / 1 failed because
  `SetStatus` allowed Setup to transition directly to Active. GREEN: 1 passed / 0 failed;
  `CompleteSetup` is now the only Setup-to-Active path, and Setup cannot be re-entered.
- Complete prior-status RED: the focused regression produced 0 passed / 1 failed because a
  Blocked current item could be completed. GREEN: 1 passed / 0 failed; Complete now requires
  the current item to be Doing and rejects other statuses before any write.
- All rejected-command cases assert the project document remains byte-identical.

### Fix implementation

- Transition entries now carry the accepted command's `document_revision`; revisions are
  strictly increasing, bounded by the aggregate revision, and corrections use their own new
  revision.
- Correction validation first checks every persisted transition's reason, IDs, pointer shape,
  and status effects. It then requires the correction to immediately follow an uncorrected,
  reversible target and verifies its before/after pointers by replaying history without that
  target.
- SetStatus cannot leave or target Setup. Complete requires a Doing current item, while Undo of
  Completed restores Doing.
- The five legal Undo round trips remain valid when Undo is immediate; any intervening accepted
  mutation makes the receipt stale and returns `UndoConflict` without changing bytes.

### Fresh verification

- `cargo test -p omniproj-core --test project_state_lifecycle`: PASS, 42 passed.
- `cargo test -p omniproj-core --lib --tests`: PASS: library 89, registry 16, lifecycle 42,
  migration 51.
- `cargo check --workspace`: PASS; only pre-existing deprecated API warnings in CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Fix Round 1 concerns

- Task 2 documents remain wire-compatible because their transition arrays are empty. Persisted
  Task 3 transition fixtures now require `document_revision`; pre-fix documents containing
  transitions without that field are intentionally rejected by the strict parser.
- No change was made to `store.rs`, the ledger, or the implementation plan.

## Fix Round 2

Code commit: `43addd0be2431498e159999b1e0ddd9790944a78`
(`fix(core): validate corrected work item statuses`)

### Status replay TDD evidence

- RED: `cargo test -p omniproj-core --test project_state_lifecycle
  parse_rejects_forged_status_after_undo_ -- --nocapture` produced 0 passed / 5 failed.
  The parser accepted an undone Set item left Doing, an undone Replace item left Doing, an
  undone Completed item left Done, and forged status changes across corrected Confirm and Clear.
- GREEN: `cargo test -p omniproj-core --test project_state_lifecycle
  parse_rejects_forged_status -- --nocapture` produced 7 passed / 0 failed. The final focused
  suite includes the five correction cases plus transition-revision-gap and post-Undo tail
  framing regressions.
- Legal Undo regression: `cargo test -p omniproj-core --test project_state_lifecycle undo_
  -- --nocapture` produced 18 passed / 0 failed, including all five legal Undo round trips and
  stale-receipt cases.

### Fix implementation

- Aggregate replay now returns both the effective pointer and only the WorkItem statuses that
  commitment history can prove. Set and Replace introduce Doing items; Completed produces Done;
  corrections of Set/Replace produce Abandoned; correction of Completed restores Doing.
- Confirm and Clear do not alter status, and Replace/Clear do not alter their previous item.
  Therefore already-known statuses survive these transitions while untouched legacy WorkItems
  with Planned, Blocked, or other statuses remain unconstrained.
- Known status is retained across document-revision gaps and trailing non-transition revisions:
  R0 SaveFraming and SetStatus cannot mutate WorkItem status. A future independent WorkItem
  status command must extend the persisted history rather than weaken current replay.
- The non-Doing Complete gate remains covered by a direct in-memory domain test with exact state
  equality. The former integration fixture directly rewrote a command-produced item without an
  accepted revision or history entry, which is now correctly outside the valid persisted domain.

### Fresh verification

- `cargo test -p omniproj-core --test project_state_lifecycle`: PASS, 48 passed.
- `cargo test -p omniproj-core --lib --tests`: PASS: library 90, registry 16, lifecycle 48,
  migration 51.
- `cargo check --workspace`: PASS; only pre-existing deprecated API warnings in CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Fix Round 2 concerns

- Status inference deliberately covers only WorkItems whose status is established by commitment
  transitions; unrelated legacy WorkItems remain compatible.
- No change was made to `store.rs`, `lib.rs`, the ledger, or the implementation plan.
