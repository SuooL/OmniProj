# R0 Trusted Project Re-entry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the approved R0 product loop: a Human-led, pull-based Projects Index with stable project identity, deterministic review reasons, URL-backed Project Peek/full-page detail, explicit current-commitment lifecycle, UI project registration/relink, local atomic persistence, and Undo.

**Architecture:** Keep Tauri and the Rust workspace, but separate durable domain state from repository observations and presentation DTOs. `meta.toml` becomes the registry/source envelope; one atomically replaced, Git-tracked `notes/project.md` with TOML front matter becomes the canonical Human project/commitment document. The desktop crate exposes a typed R0 command allowlist and caches only last-successful repository observations. React consumes that contract through TanStack Query, canonical routes, constrained semantic components, and progressive disclosure from Index to Peek/full page.

**Tech Stack:** Rust 2021, serde/toml/chrono/uuid, Tauri 2, React 19, TypeScript 5.7, TanStack Query 5, React Router, Tailwind CSS 4, Vitest, Testing Library, Playwright, axe-core.

## Global Constraints

- Treat [the approved product and UX specification](../specs/2026-08-10-omniproj-product-ux-redesign.md) as the product source of truth. This plan resolves implementation detail; it does not reopen R0 scope.
- Preserve the source-repository read-only boundary. Tests must prove that registration, observation, refresh, relink, framing, commitment mutations, and Undo never write inside a source repository.
- Preserve existing `notes/next.md`, `plan.md`, `auto/`, `learned.md`, and cache data during migration. Do not infer a current commitment from legacy tasks, even when exactly one task is marked `Doing`.
- A migrated project keeps its existing `projects/<legacy-hash>/` directory name as its permanent opaque `ProjectId`. A relink changes only its primary `ProjectSource.location`.
- New project and source IDs use UUID v7. Typed ID parsers must also accept legacy hexadecimal IDs.
- All R0 Human state—framing, lifecycle, WorkItems, current pointer, transitions, and document revision—lives in one `notes/project.md` front matter block and is replaced atomically. This is the transaction boundary for setup, replace, complete, clear, and correction.
- `notes/project.md` uses `+++` TOML front matter followed by preserved Markdown prose. Unknown Markdown body text must round-trip byte-for-byte.
- `Confirm` appends an event and resets review age, but does not change the original `set_at`. `updated_at` never drives review age.
- Undo is limited to the newest reversible transition. It appends a compensating `correction`; it never deletes history. A later mutation makes an older receipt non-undoable.
- Every Human-state mutation carries `expected_revision`. Relink carries `expected_source_revision`. Stale writes return `revision_conflict` and preserve the caller's draft in the UI.
- Repository failure suppresses inactivity conclusions and never overwrites the last successful observation cache.
- `Actual changed` is informational in Project detail; it is not an R0 ReviewReason and cannot change Index review order.
- R0 registers no Agent, reminder, Attention, full Work, Decision, Git graph, attribution, or settings commands/routes. Legacy files and compatible data remain in the repository but are disconnected from the shipped R0 surface.
- Do not introduce a generic color-selectable `Badge`, Redux/Zustand, a general component library, or an inferred health/priority score.
- Use explicit save controls for framing and commitment creation/replacement. Blur never persists.
- Run the focused test after every implementation step. Do not continue from red tests unless the step explicitly expects red.

---

## File and Ownership Map

### Core domain and persistence

- Modify `Cargo.toml`: add the workspace UUID dependency.
- Modify `crates/omniproj-core/Cargo.toml`: enable UUID support used by typed IDs.
- Create `crates/omniproj-core/src/ids.rs`: typed opaque IDs supporting legacy values and UUID v7.
- Create in Task 2, then extend in Task 3: `crates/omniproj-core/src/project_state.rs`: `notes/project.md` front matter, WorkItems, transitions, revision checks, and state-machine mutations.
- Create `crates/omniproj-core/src/review.rs`: pure deterministic R0 ReviewReason derivation.
- Modify `crates/omniproj-core/src/project.rs`: v2 Project/ProjectSource registry and relink APIs.
- Modify `crates/omniproj-core/src/store.rs`: checked lock, atomic replacement, checked audit commit, and v1→v2 migration.
- Modify `crates/omniproj-core/src/paths.rs`: typed-ID paths and legacy hash compatibility.
- Modify `crates/omniproj-core/src/lib.rs`: new exports.
- Create `crates/omniproj-core/tests/schema_v2_migration.rs`.
- Create `crates/omniproj-core/tests/project_source_registry.rs`.
- Create `crates/omniproj-core/tests/project_state_lifecycle.rs`.
- Create `crates/omniproj-core/tests/review_reasons.rs`.

### Observation and stable-ID consumers

- Modify `crates/omniproj-capture/src/git.rs`: typed repository inspection and exact commit timestamps while retaining legacy readers temporarily.
- Modify `crates/omniproj-capture/src/lib.rs`: accept explicit ProjectId/ProjectSource for new capture paths.
- Create `crates/omniproj-capture/tests/git_observation.rs`.
- Modify `crates/omniproj-index/src/lib.rs`: index/cache by ProjectId rather than recomputed path hash.
- Create `crates/omniproj-index/tests/stable_project_id.rs`.
- Modify `crates/omniproj-cli/src/main.rs`: register/find/remove/list through stable IDs and source locations.

### Tauri application boundary

- Create `crates/omniproj-desktop/src/lib.rs`: builder and R0 command registration.
- Replace `crates/omniproj-desktop/src/main.rs` with a binary shim.
- Create `crates/omniproj-desktop/src/error.rs`: serializable command errors.
- Create `crates/omniproj-desktop/src/dto.rs`: R0 request/response DTOs.
- Create `crates/omniproj-desktop/src/repository_cache.rs`: last-successful observation cache.
- Create `crates/omniproj-desktop/src/state.rs`: per-project in-flight refresh coordination; no canonical product state.
- Create `crates/omniproj-desktop/src/service.rs`: application orchestration, clock seam, revision enforcement, refresh/relink logic.
- Create `crates/omniproj-desktop/src/commands.rs`: thin Tauri commands.
- Move old command implementation to `crates/omniproj-desktop/src/legacy.rs`; keep it out of the R0 handler allowlist.
- Create `crates/omniproj-desktop/tests/r0_commands.rs`.
- Modify `crates/omniproj-desktop/Cargo.toml`, `capabilities/default.json`, and `tauri.conf.json` for the dialog plugin and R0 window floor.

### React application

- Modify `crates/omniproj-desktop/web/package.json`, `package-lock.json`, `vite.config.ts`, `src/main.tsx`, `src/App.tsx`, `src/api.ts`, and `src/index.css`.
- Create `src/domain/project.ts`, `errors.ts`, `routes.ts`, `projectPresentation.ts`, `navigationSession.ts`, and their focused tests.
- Create `src/queryClient.ts`, `src/queryKeys.ts`, `src/platform/dialog.ts`, `src/test/setup.ts`, and `src/test/fixtures.ts`.
- Create routes `ProjectsIndexPage.tsx`, `ProjectOverviewPage.tsx`, and `NotFoundPage.tsx`.
- Create application components `AppShell.tsx`, `LiveStatus.tsx`, `AddProjectDialog.tsx`, and hooks `useAppShortcuts.ts`, `useMediaQuery.ts`.
- Create project components `ProjectsIndex.tsx`, `ProjectRow.tsx`, `ProjectOverview.tsx`, `ProjectPeek.tsx`, `ProjectFramingForm.tsx`, `CurrentCommitment.tsx`, `CommitmentHistory.tsx`, `ObservedActual.tsx`, `ReviewReasons.tsx`, and `SourceRecovery.tsx`.
- Create `ProjectLifecycleControl.tsx` for explicit active/waiting/parked/archived changes, required reasons, and review dates.
- Create constrained semantic components `ProjectStateTag.tsx`, `ReviewSignalBadge.tsx`, `CommitmentStateTag.tsx`, `FactLabel.tsx`, `ActivityStamp.tsx`, and `FilterChip.tsx`.
- Move old `ProjectCard.tsx`, `ProjectDetail.tsx`, `GitGraph.tsx`, `Decisions.tsx`, `Settings.tsx`, `Sparkline.tsx`, and `staleness.ts` to `crates/omniproj-desktop/web/legacy-src/`, outside the TypeScript build graph.
- Create `playwright.config.ts`, `e2e/r0-core.spec.ts`, `e2e/responsive.spec.ts`, and `e2e/accessibility.spec.ts`.

### CI and documentation

- Modify `.github/workflows/ci.yml`: install Node, run web unit/build checks, and retain Rust gates.
- Modify `README.md`: document the R0 routes, source recovery, local state files, and manual smoke checklist.

---

## Task 1: Establish typed IDs and checked atomic store primitives

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/omniproj-core/Cargo.toml`
- Create: `crates/omniproj-core/src/ids.rs`
- Modify: `crates/omniproj-core/src/store.rs`
- Modify: `crates/omniproj-core/src/lib.rs`
- Test: inline unit tests in `ids.rs` and `store.rs`

- [ ] **Step 1: Write failing typed-ID tests**

Add tests that prove a legacy `16`-hex project ID parses unchanged, a legacy `4`-hex WorkItem ID parses unchanged, generated IDs are UUID v7 strings, separators/path traversal are rejected, and serde round-trips transparently.

```rust
#[test]
fn project_id_accepts_legacy_and_rejects_paths() {
    assert_eq!(ProjectId::parse("b8a9e19ef3c91245").unwrap().as_str(), "b8a9e19ef3c91245");
    assert!(ProjectId::parse("../projects/other").is_err());
    assert!(ProjectId::parse("").is_err());
}

#[test]
fn generated_project_id_is_uuid_v7() {
    let id = ProjectId::new();
    assert_eq!(uuid::Uuid::parse_str(id.as_str()).unwrap().get_version_num(), 7);
}
```

Run: `cargo test -p omniproj-core ids::tests -- --nocapture`

Expected: FAIL because `ids` does not exist.

- [ ] **Step 2: Add UUID and implement typed IDs**

Add to workspace dependencies:

```toml
uuid = { version = "1", features = ["v7", "serde"] }
```

Implement `ProjectId`, `ProjectSourceId`, `WorkItemId`, and `CommitmentTransitionId` as `#[serde(transparent)]` newtypes with `new`, `parse`, `as_str`, `Display`, and `FromStr`. Permit ASCII alphanumeric plus `-`, require `4..=64` characters, and reject `.` and path separators.

Run: `cargo test -p omniproj-core ids::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Write failing atomic-store tests**

Add tests for these exact signatures:

```rust
pub fn with_store_txn<T>(f: impl FnOnce() -> Result<T, StoreError>) -> Result<T, StoreError>;
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), StoreError>;
pub fn commit_paths_checked(message: &str, relative_paths: &[PathBuf]) -> Result<bool, StoreError>;
```

The tests must verify parent creation, replacement without a partial temp file, error propagation for an unwritable target, lock acquisition failure, `Ok(false)` when no audit changes exist, and `Ok(true)` after a tracked write. Seed an unrelated modified Human file and assert it remains unstaged and absent from the new commit.

Run: `cargo test -p omniproj-core store::tests -- --nocapture`

Expected: FAIL because the checked APIs are absent.

- [ ] **Step 4: Implement checked storage without changing legacy callers yet**

Implement `atomic_write` as same-directory temporary write → `sync_all` → rename → parent-directory sync on Unix. Name the temporary file from the destination filename plus process ID and a UUID. Always remove a leftover temp file on error. Make `with_store_txn` fail when the lock cannot be acquired; do not preserve the existing best-effort fallback in the new API. Make `commit_paths_checked` run targeted `git add -- <paths>` and return command stderr through `StoreError::AuditCommit`; it must never use `git add -A`.

Keep `store_txn` and `commit_all` as deprecated wrappers until Tasks 2–7 migrate all R0 callers. No new R0 code may call either wrapper.

Run: `cargo test -p omniproj-core store::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Export the new primitives and run the core crate**

Run: `cargo test -p omniproj-core --lib`

Expected: PASS with no existing test regression.

- [ ] **Step 6: Commit the foundation**

```bash
git add Cargo.toml Cargo.lock crates/omniproj-core/Cargo.toml crates/omniproj-core/src/ids.rs crates/omniproj-core/src/store.rs crates/omniproj-core/src/lib.rs
git commit -m "feat(core): add typed ids and atomic store writes"
```

---

## Task 2: Define the schema-v2 registry and project-state persistence foundation

**Files:**

- Modify: `crates/omniproj-core/src/project.rs`
- Modify: `crates/omniproj-core/src/paths.rs`
- Modify: `crates/omniproj-core/src/store.rs`
- Modify: `crates/omniproj-core/src/lib.rs`
- Create: `crates/omniproj-core/src/project_state.rs` (minimal valid setup-document encoder; Task 3 adds the full codec and state machine)
- Create: `crates/omniproj-core/tests/schema_v2_migration.rs`
- Create: `crates/omniproj-core/tests/project_source_registry.rs`

- [ ] **Step 1: Write the v1→v2 migration fixture first**

Build a temporary v1 store containing one exact legacy `meta.toml`, `notes/next.md`, `plan.md`, and `auto/briefing.md`. After `ensure_home`, assert:

```rust
assert_eq!(std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(), "2\n");
assert!(home.join("projects/b8a9e19ef3c91245").exists());
assert_eq!(project.id.as_str(), "b8a9e19ef3c91245");
assert_eq!(project.primary_git_source().unwrap().location, legacy_path);
assert_eq!(std::fs::read_to_string(next_path).unwrap(), legacy_next_bytes);
assert_eq!(std::fs::read_to_string(plan_path).unwrap(), legacy_plan_bytes);
assert_eq!(std::fs::read_to_string(briefing_path).unwrap(), legacy_briefing_bytes);
```

Also rerun `ensure_home` and assert byte-identical v2 files to prove idempotence. Add failpoint cases after journal creation, project-state write, metadata write, project-path audit commit, schema stamp, and schema audit commit; each rerun must converge to the same v2 tree. Seed a pre-existing unrecognized `notes/project.md` and assert migration stops with `MigrationConflict` without overwriting it or stamping schema 2.

Run: `cargo test -p omniproj-core --test schema_v2_migration -- --nocapture`

Expected: FAIL because schema remains v1.

- [ ] **Step 2: Define the v2 registry envelope**

Use these public types and serialized snake-case values:

```rust
pub enum ProjectSourceKind { GitRepo, Session, DocumentPath }
pub enum ProjectSourceStatus { Available, Moved, Unreadable, Missing }

pub struct ProjectSource {
    pub id: ProjectSourceId,
    pub project_id: ProjectId,
    pub kind: ProjectSourceKind,
    pub location: String,
    pub is_primary: bool,
    pub status: ProjectSourceStatus,
    pub created_at: String,
    pub last_observed_at: Option<String>,
    pub last_successful_refresh_at: Option<String>,
    pub last_error_category: Option<String>,
    pub revision: u64,
}

pub struct ProjectRecord {
    pub id: ProjectId,
    pub name: String,
    pub created_at: String,
    pub sources: Vec<ProjectSource>,
    pub capture_cursor: CaptureCursor,
    pub cadence: Option<Cadence>,
}
```

Keep the current `ProjectMeta { path, hash, ... }`, `load_meta`, and `list_projects` as deprecated adapters projected from `ProjectRecord`, so deferred CLI/desktop/distill code compiles during the staged migration. New R0 code uses only `ProjectRecord`, `load_project`, and `list_project_records`. Expose `storage_id()`, `primary_git_source()`, and `primary_git_source_mut()` helpers. Preserve existing `project_dir(&str)`, `auto_dir(&str)`, `notes_dir(&str)`, and `cache_dir(&str)`; add typed `project_dir_for(&ProjectId)`, `auto_dir_for`, `notes_dir_for`, and `cache_dir_for` for every new R0 caller.

- [ ] **Step 3: Implement the project-state persistence foundation**

In `project_state.rs`, define the persisted `ProjectStateDoc`, `ProjectStatus`, `WorkItem`, `WorkItemStatus`, and `CommitmentTransition` data shapes used in Task 3, plus `new_setup`, strict `parse`, deterministic `render`, `load`, and atomic `save`. At this stage, do not expose lifecycle mutations. Test delimiter errors, unsupported document schema, duplicate IDs, dangling pointers, invalid timestamps, and byte-for-byte Markdown-body preservation.

Run: `cargo test -p omniproj-core project_state::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Implement the migration as an explicit version step**

Set `CURRENT_SCHEMA_VERSION` to `2`. Deserialize v1 files through a private `LegacyProjectMetaV1`; never use defaults on the v2 struct to hide malformed v1 data. Map directory name to `ProjectId`, path to one primary Git source, `added_at` to `created_at`, and cursor/cadence byte-equivalently. Do not inspect or rewrite legacy Human documents.

Use a root `.migration-v2` recovery journal and add `/.migration-v2` to the store `.gitignore`. `ensure_home` checks and resumes this journal before accepting an apparently current schema. For each project, write the setup state only when the path is absent, then write v2 metadata; if an unrecognized `notes/project.md` exists without a matching journal entry, return `MigrationConflict`. A resumed step detects and strictly validates either v1 or already-written v2 metadata before continuing. After all projects are durable, commit only the migrated `meta.toml` and newly created `notes/project.md` paths, atomically stamp schema `2`, commit only `SCHEMA_VERSION`, and remove the journal. A failed audit commit leaves the journal so the next run retries the targeted audit rather than silently accepting unaudited state.

Add `ProjectStateDoc::new_setup(created_at)` and its encoder with this exact fixture timestamp; Task 3 will extend the same module rather than replace it:

```text
+++
schema_version = 1
revision = 0
status = "setup"
status_changed_at = "2026-08-10T12:00:00Z"
created_at = "2026-08-10T12:00:00Z"
updated_at = "2026-08-10T12:00:00Z"
work_items = []
commitment_transitions = []
+++

# Project notes
```

In the fixture, `status_changed_at` is also exactly `2026-08-10T12:00:00Z`. In production migration, all three values come from the parsed legacy `added_at`; no literal date is written to real projects.

Run: `cargo test -p omniproj-core --test schema_v2_migration -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Write failing registration, duplicate, and relink tests**

Cover `Created`, duplicate canonical source returning the existing ProjectId, source-owner lookup that does not mutate the store, relink retaining ProjectId/project directory/Human files, relink collision, and longest-prefix cwd lookup after relink. Inject failures after each staging-file write and immediately before directory rename; assert no partial project appears in `list_projects` and a retry succeeds.

Use these exact APIs:

```rust
pub fn load_project(id: &ProjectId) -> Result<ProjectRecord, ProjectStoreError>;
pub fn list_project_records() -> Result<Vec<ProjectRecord>, ProjectStoreError>;
pub fn canonical_source_owner(location: &Path) -> Result<Option<ProjectId>, ProjectStoreError>;
pub fn register_project(input: RegisterProjectInput<'_>) -> Result<RegisterOutcome, ProjectStoreError>;
pub fn relink_primary_git_source(input: RelinkSourceInput<'_>) -> Result<ProjectRecord, ProjectStoreError>;
pub fn record_source_observation(input: RecordSourceObservationInput<'_>) -> Result<ProjectRecord, ProjectStoreError>;
pub fn find_project_by_cwd(cwd: &Path) -> Result<Option<ProjectRecord>, ProjectStoreError>;
```

Run: `cargo test -p omniproj-core --test project_source_registry -- --nocapture`

Expected: FAIL before implementation.

- [ ] **Step 6: Implement registry operations under the checked store lock**

Core canonicalizes existing readable paths and detects duplicates by canonical primary source location; it does not inspect source Git because `omniproj-capture` owns that boundary. Task 5 and the desktop service distinguish non-Git and bare repositories before calling these mutation APIs. Generate UUID v7 IDs only after duplicate validation and repeat duplicate detection after acquiring the store lock. Build the complete new project under `projects/.staging-<id>/`, fsync its setup state and metadata, then atomically rename the whole directory to `projects/<id>/`; a crash or injected failure before rename leaves no enumerable project, and startup removes only recognized empty staging directories. Relink is called only with an outer-layer validated Git source, checks duplicate ownership again under lock, uses source revision compare-and-swap, atomically updates only the source record, and keeps prior Human state untouched.

Implement `record_source_observation` as the only public source-status writer. Its input contains project/source IDs, expected source revision, expected location, attempt time, and either success timestamps or a typed failure status/category. It reloads under lock, compare-and-swaps revision/location, increments source revision once, atomically writes metadata, and commits only that metadata path. Core stores the supplied outcome but never imports or calls Git code.

Run: `cargo test -p omniproj-core --test project_source_registry -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Verify migration and registry together**

Run: `cargo test -p omniproj-core --test schema_v2_migration --test project_source_registry`

Expected: PASS.

- [ ] **Step 8: Commit the registry and state persistence foundation**

```bash
git add crates/omniproj-core/src/project.rs crates/omniproj-core/src/project_state.rs crates/omniproj-core/src/paths.rs crates/omniproj-core/src/store.rs crates/omniproj-core/src/lib.rs crates/omniproj-core/tests/schema_v2_migration.rs crates/omniproj-core/tests/project_source_registry.rs
git commit -m "feat(core): separate projects from repository sources"
```

---

## Task 3: Implement one-file Human project state and commitment lifecycle

**Files:**

- Modify: `crates/omniproj-core/src/project_state.rs`
- Modify: `crates/omniproj-core/src/lib.rs`
- Create: `crates/omniproj-core/tests/project_state_lifecycle.rs`

- [ ] **Step 1: Extend and rerun parser/round-trip safety tests**

Define a fixture with all statuses, multiline TOML strings, an unknown Markdown body, two WorkItems, and two transitions. Assert parsed values, exact preserved body bytes, and parse(render(parse(input))) equality. A missing document must be a typed `NotFound`, not an empty state.

Use these types:

```rust
pub enum ProjectStatus { Setup, Active, Waiting, Parked, Archived }
pub enum WorkItemStatus { Planned, Doing, Blocked, Done, Abandoned }
pub enum CommitmentTransitionKind { Set, Confirmed, Completed, Replaced, Cleared, Correction }

pub struct ProjectStateDoc {
    pub schema_version: u32,
    pub revision: u64,
    pub status: ProjectStatus,
    pub status_reason: Option<String>,
    pub status_changed_at: String,
    pub objective: Option<String>,
    pub desired_outcome: Option<String>,
    pub phase: Option<String>,
    pub current_next_action_id: Option<WorkItemId>,
    pub review_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub work_items: Vec<WorkItem>,
    pub commitment_transitions: Vec<CommitmentTransition>,
    markdown_body: String,
}
```

Run: `cargo test -p omniproj-core --test project_state_lifecycle parse_ -- --nocapture`

Expected: PASS against Task 2's persistence foundation.

- [ ] **Step 2: Add aggregate invariants to the existing codec**

In addition to Task 2's syntax validation, reject transition references to missing WorkItems, a stored pointer that differs from replayed effective history, and a correction that targets a missing/already-corrected/correction transition. Preserve body starting immediately after the closing delimiter. `save` increments no revision by itself; only accepted domain commands do.

Run: `cargo test -p omniproj-core --test project_state_lifecycle parse_ -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Write the complete commitment state-machine matrix**

Add one test per command and failure:

- set on empty → new `Doing` item, pointer, `Set`, revision +1;
- set on occupied → `CurrentCommitmentExists`, no bytes changed;
- confirm same item → `Confirmed`, unchanged original set time, revision +1;
- complete → item `Done`, empty pointer, `Completed`, revision +1;
- replace with nonempty reason → previous item and its status retained unchanged, new `Doing` item, pointer swap, `Replaced`;
- replace with empty reason → `ReasonRequired`, no bytes changed;
- clear → previous item and its status retained unchanged, empty pointer, `Cleared`;
- status waiting → nonempty reason and review date required;
- status parked → nonempty reason required, review date optional;
- archived → excluded by the later Index assembler;
- full setup → objective, outcome, first commitment, and `Setup -> Active` in one revision;
- incomplete setup → typed field error and byte-identical file;
- wrong expected revision → `RevisionConflict` and byte-identical file.

Run: `cargo test -p omniproj-core --test project_state_lifecycle lifecycle_ -- --nocapture`

Expected: FAIL before command implementation.

- [ ] **Step 4: Implement a single mutation entry point**

Use:

```rust
pub enum ProjectCommand {
    SaveFraming { objective: String, desired_outcome: String, phase: Option<String> },
    CompleteSetup { objective: String, desired_outcome: String, phase: Option<String>, first_commitment: String },
    SetCommitment { text: String },
    ConfirmCommitment { work_item_id: WorkItemId },
    CompleteCommitment { work_item_id: WorkItemId },
    ReplaceCommitment { previous_work_item_id: WorkItemId, text: String, reason: String },
    ClearCommitment { work_item_id: WorkItemId, reason: Option<String> },
    SetStatus { status: ProjectStatus, reason: Option<String>, review_at: Option<String> },
    Undo { transition_id: CommitmentTransitionId },
}

pub fn apply_project_command(
    project_id: &ProjectId,
    expected_revision: u64,
    command: ProjectCommand,
    occurred_at: &str,
) -> Result<ProjectMutation, ProjectStateError>;
```

Load and validate inside `with_store_txn`, clone the pre-state, apply in memory, validate all invariants, increment revision once, atomically replace `notes/project.md`, and then run `commit_paths_checked` with only that project's state path. If the audit commit fails, return `AuditCommitFailed` with the durable mutation revision; do not lie that the Human state was not saved.

Run: `cargo test -p omniproj-core --test project_state_lifecycle lifecycle_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Write and implement compensating Undo tests**

For each newest transition type (`Set`, `Confirmed`, `Completed`, `Replaced`, `Cleared`), assert the resulting pointer/item statuses, an appended `Correction` with `corrects_transition_id`, and preservation of the original event. Assert an older transition returns `UndoConflict`. Assert a correction itself is not undoable in R0.

Run: `cargo test -p omniproj-core --test project_state_lifecycle undo_ -- --nocapture`

Expected: PASS after the implementation.

- [ ] **Step 6: Verify no legacy document was touched**

Add an integration fixture containing hand-authored prose and id-less tasks in `next.md`, then execute all R0 project commands and assert byte-identical `next.md` and `plan.md`.

Run: `cargo test -p omniproj-core --test project_state_lifecycle preserves_legacy_documents -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Commit the Human-state boundary**

```bash
git add crates/omniproj-core/src/project_state.rs crates/omniproj-core/src/lib.rs crates/omniproj-core/tests/project_state_lifecycle.rs
git commit -m "feat(core): add auditable commitment state machine"
```

---

## Task 4: Derive deterministic R0 review reasons

**Files:**

- Create: `crates/omniproj-core/src/review.rs`
- Modify: `crates/omniproj-core/src/lib.rs`
- Create: `crates/omniproj-core/tests/review_reasons.rs`

- [ ] **Step 1: Encode the full truth table as failing tests**

Define `REVIEW_RULE_VERSION = "r0-v1"` and `DEFAULT_COMMITMENT_REVIEW_DAYS = 7`. Cover the five reasons in fixed priority order, every suppression rule, exact boundary at seven days, confirmation resetting review age without altering set age, replacement establishing the new commitment clock, corrections removing the effect of the transition they compensate, and multiple-reason aggregation. Explicitly assert `Actual changed` never appears in the returned reasons.

```rust
pub enum ReviewReasonCode {
    SourceUnavailable,
    CompleteSetup,
    NeedsCommitment,
    ReviewAction,
    ScheduledReview,
}

pub struct ReviewReason {
    pub code: ReviewReasonCode,
    pub label: String,
    pub evidence: Vec<String>,
    pub rule_version: String,
}
```

Run: `cargo test -p omniproj-core --test review_reasons -- --nocapture`

Expected: FAIL because the derivation does not exist.

- [ ] **Step 2: Implement one pure derivation function**

```rust
pub fn derive_review_reasons(
    state: &ProjectStateDoc,
    source: &ProjectSource,
    now: chrono::DateTime<chrono::Utc>,
    commitment_review_days: i64,
) -> Vec<ReviewReason>;
```

Replay transitions into an effective history before deriving time. A `Correction` masks the event it compensates and is never itself a clock anchor. The current commitment's original `set_at` is the effective `Set` or `Replaced` event that assigned its pointer; its review anchor is the later of that event and the newest effective `Confirmed` event for the same WorkItem. Undoing Complete/Replace/Clear restores the prior effective pointer and its prior clock; a corrected confirmation cannot reset review age. Do not consult WorkItem `updated_at`, commit activity, dirty files, or cached observation age. Return reasons already sorted by the specification's fixed priority.

Run: `cargo test -p omniproj-core --test review_reasons -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Commit deterministic review semantics**

```bash
git add crates/omniproj-core/src/review.rs crates/omniproj-core/src/lib.rs crates/omniproj-core/tests/review_reasons.rs
git commit -m "feat(core): derive deterministic project review reasons"
```

---

## Task 5: Add typed, read-only repository observations

**Files:**

- Modify: `crates/omniproj-capture/src/git.rs`
- Create: `crates/omniproj-capture/tests/git_observation.rs`

- [ ] **Step 1: Write hermetic repository-state tests**

Create fixtures for missing path, regular non-repository directory, bare repository, unreadable directory where supported, empty/unborn repository, attached branch, detached HEAD, dirty staged/unstaged/untracked files, Git command failure, and a commit with a fixed RFC3339 author date. Snapshot every file path/content/mtime before and after inspection to prove zero source writes.

Use these types:

```rust
pub enum RepositoryReadErrorKind {
    PathMissing,
    PermissionDenied,
    NotRepository,
    BareRepository,
    GitUnavailable,
    CommandFailed,
    InvalidOutput,
}

pub enum HeadState {
    Attached { branch: String },
    Detached,
    Unborn { branch: Option<String> },
}

pub struct CommitObservation {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub committed_at: String,
}

pub struct RepositoryObservation {
    pub observed_at: String,
    pub head_state: HeadState,
    pub head_sha: Option<String>,
    pub last_commit: Option<CommitObservation>,
    pub changed_files: usize,
    pub staged_files: usize,
    pub unstaged_files: usize,
    pub untracked_files: usize,
    pub status_digest: String,
}
```

Run: `cargo test -p omniproj-capture --test git_observation -- --nocapture`

Expected: FAIL before the new API.

- [ ] **Step 2: Implement `observe_repository` without weakening legacy APIs**

```rust
pub fn observe_repository(
    path: &Path,
    observed_at: &str,
) -> Result<RepositoryObservation, RepositoryReadError>;

pub fn count_commits_since(
    path: &Path,
    since_rfc3339: &str,
) -> Result<u32, RepositoryReadError>;
```

Run every source-repository Git command as `git --no-optional-locks -C <repo> ...` (and set `GIT_OPTIONAL_LOCKS=0` defensively). Use `rev-parse --is-bare-repository`, `symbolic-ref --short -q HEAD`, `rev-parse --verify HEAD`, `status --porcelain=v1`, and one log format containing `%cI`. Capture stderr and exit status. Treat empty/unborn and detached as successful states. Keep existing `collect`, `commit_log`, and `commit_graph` unchanged until deferred callers are migrated.

`count_commits_since` runs `git rev-list --count --since=<RFC3339> HEAD`, returns `0` for an unborn repository, and is used only for the neutral phrase `N repository commits observed since this commitment was set`. It never becomes a ReviewReason, progress score, or proof that the commitment advanced.

The read-only fixture compares worktree bytes plus `.git/index` hash and mtime before/after observation, validation, refresh, and relink validation.

Run: `cargo test -p omniproj-capture --test git_observation -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Strengthen the existing commit timestamp without breaking callers**

Add `committed_at: String` to `CommitEntry`, preserve the existing `date` field for legacy UI, and parse both `%cI` and `%cs` in one command. Update its inline tests to assert RFC3339 parsing.

Run: `cargo test -p omniproj-capture git::tests::commit_log_returns_structured_entries_newest_first -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Commit typed observations**

```bash
git add crates/omniproj-capture/src/git.rs crates/omniproj-capture/tests/git_observation.rs
git commit -m "feat(capture): report typed repository observations"
```

---

## Task 6: Move capture, index, and CLI off path-derived identity

**Files:**

- Modify: `crates/omniproj-capture/src/lib.rs`
- Modify: `crates/omniproj-index/src/lib.rs`
- Modify: `crates/omniproj-cli/src/main.rs`
- Create: `crates/omniproj-index/tests/stable_project_id.rs`

- [ ] **Step 1: Write a failing stable-index test**

Register a project, index one session under its ProjectId, move the source repository, relink it, and assert the same cache database path and search result remain. Assert no second cache directory keyed from the new path appears.

Run: `cargo test -p omniproj-index --test stable_project_id -- --nocapture`

Expected: FAIL because index identity is still `sub.hash`.

- [ ] **Step 2: Introduce explicit capture identity**

Add:

```rust
pub fn capture_source(
    project_id: &ProjectId,
    source: &ProjectSource,
) -> anyhow::Result<Substrate>;
```

Rename new-path fields to `project_id` and `location`. Retain `capture(dir)` only as a deprecated legacy wrapper; no R0 caller may use it.

Run: `cargo test -p omniproj-capture --lib`

Expected: PASS.

- [ ] **Step 3: Key new index APIs by ProjectId**

Add `ensure_index_for(project_id, sessions)`, `search_for(project_id, query, limit)`, and `index_path(project_id)`. Make row decoding errors fail the query instead of disappearing through `filter_map(Result::ok)`. Keep the old path-based wrappers deprecated for deferred callers.

Run: `cargo test -p omniproj-index --test stable_project_id -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Adapt CLI registration, listing, lookup, and removal**

The CLI must print `ID`, not `HASH`; `add` must use `register_project`; `remove` must resolve the registered primary source and then remove by ProjectId; `find_by_cwd` must use current ProjectSource locations. Preserve existing note/plan commands by passing `project.id.as_str()` to legacy document wrappers until R1 migration.

Run: `cargo test -p omniproj-cli && cargo check --workspace`

Expected: PASS.

- [ ] **Step 5: Commit stable identity consumers**

```bash
git add crates/omniproj-capture/src/lib.rs crates/omniproj-index/src/lib.rs crates/omniproj-index/tests/stable_project_id.rs crates/omniproj-cli/src/main.rs
git commit -m "refactor: use stable project ids across local services"
```

---

## Task 7: Build the typed R0 desktop service and command allowlist

**Files:**

- Create: `crates/omniproj-desktop/src/lib.rs`
- Replace: `crates/omniproj-desktop/src/main.rs`
- Create: `crates/omniproj-desktop/src/error.rs`
- Create: `crates/omniproj-desktop/src/dto.rs`
- Create: `crates/omniproj-desktop/src/repository_cache.rs`
- Create: `crates/omniproj-desktop/src/state.rs`
- Create: `crates/omniproj-desktop/src/service.rs`
- Create: `crates/omniproj-desktop/src/commands.rs`
- Create: `crates/omniproj-desktop/src/legacy.rs`
- Create: `crates/omniproj-desktop/tests/r0_commands.rs`
- Modify: `crates/omniproj-desktop/Cargo.toml`
- Modify: `crates/omniproj-desktop/capabilities/default.json`
- Modify: `crates/omniproj-desktop/tauri.conf.json`

- [ ] **Step 1: Extract the current backend without changing behavior**

Move the old command bodies to `legacy.rs`, create a library `run()` entry, and reduce `main.rs` to:

```rust
fn main() {
    omniproj_desktop::run();
}
```

Run: `cargo check -p omniproj-desktop`

Expected: PASS before changing the handler allowlist.

- [ ] **Step 2: Write DTO/error serialization contract tests**

Use `#[serde(tag = "code", rename_all = "snake_case")]` or an equivalent fixed wire shape. Snapshot each recoverable error without raw stack traces. Required codes are:

```text
project_not_found, invalid_input, invalid_path, source_missing,
source_unreadable, not_git_repository, bare_repository, duplicate_source,
source_observation_failed, store_read_failed, store_write_failed,
audit_commit_failed, revision_conflict, current_commitment_exists,
no_current_commitment, current_commitment_changed, reason_required,
transition_not_found, undo_not_available, undo_conflict
```

Every error carries `message`, `retryable`, `state_applied`, and optional `field`, `project_id`, `existing_project_id`, and `durable_revision`. `audit_commit_failed` is the only normal R0 error with `state_applied: true`; the UI refetches that revision and must not resend the mutation. `store_write_failed` has `state_applied: false` and may offer Retry.

Run: `cargo test -p omniproj-desktop --test r0_commands error_ -- --nocapture`

Expected: FAIL before the types exist.

- [ ] **Step 3: Define the R0 DTOs and pure assembler**

Implement `ProjectIndexResponseDto`, `ProjectIndexItemDto`, `ProjectOverviewDto`, `ProjectSourceDto`, `CurrentCommitmentDto`, `ObservedActualDto`, `CommitmentTransitionDto`, `ReviewReasonDto`, `ReviewPolicyDto`, `SourceValidationDto`, and `RefreshResultDto`. Index rows exclude full source path; Overview includes it. Index response and Overview carry `review_policy { commitment_review_days: 7, rule_version: "r0-v1" }`. Both item/detail shapes carry Human-state `revision`; `ProjectSourceDto` separately carries `revision` for refresh/relink conflict detection. `ProjectOverviewDto` also carries `last_transition` and `undoable_transition_id` so every successful mutation can return the updated Overview directly. `ObservedActualDto` includes `commits_since_commitment: Option<u32>` only when a current commitment and successful Git observation exist. Sort reasons in core, not in React.

Test exact snake-case JSON field names and enum values.

Run: `cargo test -p omniproj-desktop --test r0_commands dto_ -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Implement last-successful observation caching**

Persist `cache/r0-observation.json` atomically with `project_id`, `source_id`, and observation timestamp. On success, update the source to `available`, set both observation timestamps, clear last error, and replace the cache. On failure, update only source status/error/attempt time; leave the cached fact bytes untouched. Cache absence plus empty Git history must still be a successful `last_commit: null` state.

Add `DesktopState { refreshes: tokio::sync::Mutex<HashSet<ProjectId>> }`. Use skip semantics: a concurrent refresh request for the same ProjectId immediately returns a stable `refresh_in_progress` outcome with the current cached row; it never launches overlapping Git commands or clears visible facts. An RAII guard removes the ID on success, error, or cancellation. This state coordinates work only and is never canonical persistence.

Persist a result through `record_source_observation(project_id, source_id, expected_source_revision, expected_location, outcome)`. That core registry method reloads metadata inside the store lock, verifies source ID/revision/location, increments source revision, atomically replaces metadata, and commits only that metadata path. If relink won the race, return `stale_observation`, do not overwrite the new location/status, and discard the old-path result. `RelinkSourceInput` likewise contains `expected_source_revision`. Add an injected refresh-versus-relink race test.

Run: `cargo test -p omniproj-desktop --test r0_commands refresh_ -- --nocapture`

Expected: PASS for success, missing, unreadable, partial multi-project failure, cached-fact preservation, and detached/unborn fixtures.

- [ ] **Step 5: Implement the service methods with a clock seam**

```rust
pub trait Clock { fn now_rfc3339(&self) -> String; }

pub struct DesktopService<C: Clock> {
    pub clock: C,
    pub state: DesktopState,
}

#[allow(async_fn_in_trait)]
pub trait R0Service {
    fn list_project_index(&self) -> CommandResult<ProjectIndexResponseDto>;
    fn get_project_overview(&self, project_id: ProjectId) -> CommandResult<ProjectOverviewDto>;
    async fn validate_project_source(&self, location: String) -> CommandResult<SourceValidationDto>;
    async fn register_project(&self, input: RegisterProjectInput) -> CommandResult<ProjectOverviewDto>;
    async fn relink_project_source(&self, input: RelinkProjectInput) -> CommandResult<ProjectOverviewDto>;
    async fn refresh_projects(&self, project_ids: Option<Vec<ProjectId>>) -> CommandResult<Vec<RefreshResultDto>>;
    fn apply_project_mutation(&self, input: ProjectMutationInput) -> CommandResult<ProjectOverviewDto>;
}
```

Run all source Git inspection through `tokio::task::spawn_blocking`; do not block the Tauri async command executor. Archived projects are absent from default Index results but remain directly addressable. Refresh returns one result per project; one source failure cannot reject the batch.

Run: `cargo test -p omniproj-desktop --test r0_commands service_ -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Register only the R0 Tauri commands**

The handler list must contain exactly:

```rust
tauri::generate_handler![
    list_project_index,
    get_project_overview,
    validate_project_source,
    register_project,
    relink_project_source,
    refresh_projects,
    complete_project_setup,
    save_project_framing,
    set_project_status,
    set_commitment,
    confirm_commitment,
    complete_commitment,
    replace_commitment,
    clear_commitment,
    undo_commitment_transition,
]
```

`complete_project_setup` accepts `project_id`, `objective`, `desired_outcome`, optional `phase`, `first_commitment`, and `expected_revision`, then calls the single core `CompleteSetup` command. `save_project_framing` edits framing after setup and never performs a hidden activation.

Every command takes one top-level `input` argument. Nested request DTOs and all response DTOs use snake-case JSON fields. The TypeScript call is therefore `invoke("replace_commitment", { input: { project_id, previous_work_item_id, expected_revision, text, reason } })`; do not mix camelCase and snake_case inside request bodies.

Remove notification plugin initialization, reminder worker, attention-count tooltip, and notification capability. Keep the neutral Open/Quit tray. Add `tauri-plugin-dialog` initialization and `dialog:allow-open`. Set the desktop minimum width to `640` so the specified `<800px` full-page path is testable.

Keep `legacy.rs` as source archive but do not declare it as a module from `lib.rs`; this prevents outdated deferred commands from compiling into the shipped binary while retaining their implementation for later redesign. Add a behavior-level IPC integration test that invokes representative deferred command names (`advance_task`, `get_graph`, `get_plan`, `get_attention`, and `test_reminder`) against the built R0 handler and asserts they are rejected as unregistered, while every R0 command is accepted by the handler boundary. Do not use source-text or symbol-presence assertions.

Run: `cargo test -p omniproj-desktop --test r0_commands handler_ -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Verify the desktop crate and commit**

Run: `cargo test -p omniproj-desktop && cargo check -p omniproj-desktop`

Expected: PASS.

```bash
git add crates/omniproj-desktop
git commit -m "feat(desktop): expose the R0 project re-entry API"
```

---

## Task 8: Establish the React contract, test harness, and query boundary

**Files:**

- Modify: `crates/omniproj-desktop/web/package.json`
- Modify: `crates/omniproj-desktop/web/package-lock.json`
- Modify: `crates/omniproj-desktop/web/vite.config.ts`
- Modify: `crates/omniproj-desktop/web/tsconfig.json`
- Modify: `crates/omniproj-desktop/web/src/api.ts`
- Modify: `crates/omniproj-desktop/web/src/main.tsx`
- Move: legacy UI files from `crates/omniproj-desktop/web/src/components/` to `crates/omniproj-desktop/web/legacy-src/`
- Move: `crates/omniproj-desktop/web/src/staleness.ts` to `crates/omniproj-desktop/web/legacy-src/staleness.ts`
- Create: `crates/omniproj-desktop/web/src/domain/project.ts`
- Create: `crates/omniproj-desktop/web/src/domain/errors.ts`
- Create: `crates/omniproj-desktop/web/src/domain/projectPresentation.ts`
- Create: `crates/omniproj-desktop/web/src/domain/projectPresentation.test.ts`
- Create: `crates/omniproj-desktop/web/src/queryClient.ts`
- Create: `crates/omniproj-desktop/web/src/queryKeys.ts`
- Create: `crates/omniproj-desktop/web/src/test/setup.ts`
- Create: `crates/omniproj-desktop/web/src/test/fixtures.ts`
- Create: `crates/omniproj-desktop/web/src/api.test.ts`

- [ ] **Step 1: Install the minimal runtime and test dependencies**

From `crates/omniproj-desktop/web`, run:

```bash
npm install react-router-dom @tauri-apps/plugin-dialog
npm install -D vitest jsdom @testing-library/react @testing-library/user-event @testing-library/jest-dom @playwright/test @axe-core/playwright
```

Add scripts:

```json
{
  "test": "vitest run",
  "test:watch": "vitest",
  "test:e2e": "playwright test",
  "check": "npm run build && npm test"
}
```

Configure Vitest with `environment: "jsdom"`, `setupFiles: ["./src/test/setup.ts"]`, CSS enabled, and automatic mock reset. Remove the obsolete `/api` proxy.

- [ ] **Step 2: Define TypeScript wire types exactly once**

Mirror the Rust snake-case contract in `domain/project.ts`. `SourceValidationDto` represents only a valid preview; missing/unreadable/non-Git/bare/duplicate outcomes arrive as typed `CommandError`, with duplicate carrying `existing_project_id`. Define `ProjectId`, `WorkItemId`, and `TransitionId` as branded strings so unrelated IDs cannot be passed accidentally.

```ts
export type ProjectId = string & { readonly __projectId: unique symbol };
export type ProjectStatus = "setup" | "active" | "waiting" | "parked" | "archived";
export type ReviewReasonCode =
  | "source_unavailable"
  | "complete_setup"
  | "needs_commitment"
  | "review_action"
  | "scheduled_review";
```

The remaining fields must match Task 7's DTO snapshots; do not add browser-derived review semantics.

- [ ] **Step 3: Write failing API adapter tests**

Mock `@tauri-apps/api/core` and assert every command name plus one top-level `input` key; assert nested input fields are snake_case exactly as defined in Task 7. Assert rejected structured errors become typed `AppError` values and an unknown rejection becomes a generic safe message without exposing a stack. For `audit_commit_failed` with `state_applied: true`, assert the adapter exposes `durable_revision` and marks the operation for refetch rather than Retry.

Run: `npm test -- src/api.test.ts`

Expected: FAIL against the legacy API.

- [ ] **Step 4: Replace the API surface with only R0 operations**

Export the exact operations from Task 7, including `completeProjectSetup`. Remove Agent/Decision/Graph/Task/Attention/Settings wrappers from `api.ts`. Move `ProjectCard.tsx`, `ProjectDetail.tsx`, `GitGraph.tsx`, `Decisions.tsx`, `Settings.tsx`, `Sparkline.tsx`, and `staleness.ts` into `web/legacy-src/`, outside `tsconfig.json`'s `src` include. Do not use a nominally unimported `src/legacy/` folder because TypeScript still checks it.

Run: `npm test -- src/api.test.ts`

Expected: PASS.

- [ ] **Step 5: Add presentation pure-function tests**

Test immutable local text filtering, `all|needs_review|waiting|parked` filters, fixed backend reason priority display, archived exclusion, transparent sort labels, relative-time formatting with exact title timestamps, and `+N` accessible names enumerating hidden reasons. Assert no function accepts commit count as health or priority input.

Run: `npm test -- src/domain/projectPresentation.test.ts`

Expected: FAIL before implementation, then PASS after the smallest pure functions are added.

- [ ] **Step 6: Extract the pull-only QueryClient**

Keep `staleTime: Infinity`, `refetchOnWindowFocus: false`, `refetchOnReconnect: false`, and `retry: 1`. Add stable keys for Index, Overview, validation, and refresh. Mutation success updates both row and detail caches from returned DTOs; it does not trigger uncontrolled polling.

Run: `npm test`

Expected: PASS.

- [ ] **Step 7: Commit the web contract**

```bash
git add crates/omniproj-desktop/web/package.json crates/omniproj-desktop/web/package-lock.json crates/omniproj-desktop/web/vite.config.ts crates/omniproj-desktop/web/tsconfig.json crates/omniproj-desktop/web/src/api.ts crates/omniproj-desktop/web/src/main.tsx crates/omniproj-desktop/web/src/domain crates/omniproj-desktop/web/src/queryClient.ts crates/omniproj-desktop/web/src/queryKeys.ts crates/omniproj-desktop/web/src/test crates/omniproj-desktop/web/legacy-src
git commit -m "test(web): establish the R0 frontend contract"
```

---

## Task 9: Implement canonical routes and the AppShell interaction frame

**Files:**

- Modify: `crates/omniproj-desktop/web/src/App.tsx`
- Modify: `crates/omniproj-desktop/web/src/main.tsx`
- Create: `crates/omniproj-desktop/web/src/domain/routes.ts`
- Create: `crates/omniproj-desktop/web/src/domain/navigationSession.ts`
- Create: `crates/omniproj-desktop/web/src/routes/ProjectsIndexPage.tsx`
- Create: `crates/omniproj-desktop/web/src/routes/ProjectOverviewPage.tsx`
- Create: `crates/omniproj-desktop/web/src/routes/NotFoundPage.tsx`
- Create: `crates/omniproj-desktop/web/src/components/AppShell.tsx`
- Create: `crates/omniproj-desktop/web/src/components/LiveStatus.tsx`
- Create: `crates/omniproj-desktop/web/src/hooks/useAppShortcuts.ts`
- Create: `crates/omniproj-desktop/web/src/App.test.tsx`
- Create: `crates/omniproj-desktop/web/src/components/AppShell.test.tsx`

- [ ] **Step 1: Write route behavior tests before the router**

Assert `/` redirects to `/projects`, `/projects/:projectId` replaces to `/projects/:projectId/overview`, a direct overview route renders a full page, an Index-origin navigation renders the same canonical URL as a Peek over the still-mounted Index, `Open as page` clears background state without changing the object URL, Back/Forward restore the prior screen, and unknown routes show `Back to Projects`. Filter and sort live in `/projects` search params. Scroll position and return-focus ID live in history state plus sessionStorage. Simulate app restart at `/` and assert the last canonical `pathname+search` is restored; an explicit incoming non-root deep link always wins over saved session state.

Run: `npm test -- src/App.test.tsx`

Expected: FAIL.

- [ ] **Step 2: Implement background-location routing**

Use one main `<Routes location={backgroundLocation ?? location}>` and a second conditional route for the Peek. `navigationSession.ts` persists only canonical path/search and noncanonical Index view state; it never stores project data. Before BrowserRouter mounts, restore the saved route only when the incoming path is `/`. Route builders live in `domain/routes.ts`; components never concatenate route strings. Do not use `HashRouter`.

Run: `npm test -- src/App.test.tsx`

Expected: PASS.

- [ ] **Step 3: Write AppShell keyboard and navigation tests**

Assert the only primary destination is `Projects`; `Cmd/Ctrl+F` focuses the local filter, `Cmd/Ctrl+N` opens Add Project, and `Cmd/Ctrl+R` prevents default only while the OmniProj window has focus and starts a pull refresh, including while a text input is focused. Assert Escape closes only the topmost Add Project modal when it is stacked over Peek. Assert polite announcements for refresh/save/Undo and assertive announcements for errors.

Run: `npm test -- src/components/AppShell.test.tsx`

Expected: FAIL.

- [ ] **Step 4: Implement AppShell and live status**

Keep shortcuts in one hook and provide visible button equivalents. Ignore only unmodified typing keys from text controls; `Cmd/Ctrl+F`, `Cmd/Ctrl+N`, and `Cmd/Ctrl+R` still work while an input is focused. Escape closes only the topmost active surface: Add Project modal before Peek. Use two persistent visually-hidden `aria-live` nodes so announcements do not remount during mutations.

Run: `npm test -- src/components/AppShell.test.tsx`

Expected: PASS.

- [ ] **Step 5: Commit routing and shell**

```bash
git add crates/omniproj-desktop/web/src/App.tsx crates/omniproj-desktop/web/src/main.tsx crates/omniproj-desktop/web/src/domain/routes.ts crates/omniproj-desktop/web/src/domain/navigationSession.ts crates/omniproj-desktop/web/src/routes crates/omniproj-desktop/web/src/components/AppShell.tsx crates/omniproj-desktop/web/src/components/LiveStatus.tsx crates/omniproj-desktop/web/src/components/AppShell.test.tsx crates/omniproj-desktop/web/src/hooks/useAppShortcuts.ts crates/omniproj-desktop/web/src/App.test.tsx
git commit -m "feat(web): add canonical project routes and app shell"
```

---

## Task 10: Build semantic tokens, constrained labels, and the Dense Projects Index

**Files:**

- Modify: `crates/omniproj-desktop/web/src/index.css`
- Create: `crates/omniproj-desktop/web/src/components/semantic/ProjectStateTag.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/ReviewSignalBadge.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/CommitmentStateTag.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/FactLabel.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/ActivityStamp.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/FilterChip.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/semantic.test.tsx`
- Create: `crates/omniproj-desktop/web/src/components/semantic/semantic.types.test.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectsIndex.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectRow.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectsIndex.test.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectRow.test.tsx`
- Modify: `crates/omniproj-desktop/web/src/routes/ProjectsIndexPage.tsx`

- [ ] **Step 1: Write constrained semantic-component tests**

Assert every state has visible text independent of color, interactive chips expose pressed state, `+N` names hidden reasons, and no semantic component renders emoji. In `semantic.types.test.tsx`, use `@ts-expect-error` cases for arbitrary `tone`, raw hex, and unknown status props; run `npm run build` because Vitest transpilation alone does not typecheck. Run a source scan that fails if component files reference raw hex or old `--color-*` variables.

Run: `npm test -- src/components/semantic/semantic.test.tsx && npm run build`

Expected: FAIL.

- [ ] **Step 2: Replace the old theme with the `--op-*` contract**

Implement the approved Light values at `:root`, Dark values under `prefers-color-scheme: dark`, explicit status foreground/background/border tokens, `prefers-contrast: more`, `forced-colors: active`, and `prefers-reduced-motion: reduce`. Necessary text is at least `12px/16px`; body is `13px/18px`; controls are at least `28px`; icon-only controls are at least `32px`.

Do not reproduce the old grid background, phosphor palette, opacity-disabled state, glow, pulse, or status animation.

Run: `npm test -- src/components/semantic/semantic.test.tsx`

Expected: PASS.

- [ ] **Step 3: Write Index state and row tests**

Cover loading, error, empty, content, refresh-with-stale-facts, semantic project links, filter and sort, at most one ProjectStateTag plus one ReviewSignalBadge, at most three FactLabels, plain `+N`, setup/missing commitment/source failure states, exact timestamp title, detached/unborn/empty history facts, long text growth, and archived exclusion. Assert visible `Project`, `Current commitment`, `Observed actual`, and `Review` headers; stacked rows retain those field labels. The empty state has one focusable primary `Add project` action that opens the modal. Beside `Review order`, display `Commitment review interval: 7 days` from `review_policy`, never a frontend constant.

Assert a row never renders full path, Sparkline, health/current badge, Git graph, Agent control, full task list, or activity-derived ranking.

Run: `npm test -- src/components/projects/ProjectsIndex.test.tsx src/components/projects/ProjectRow.test.tsx`

Expected: FAIL.

- [ ] **Step 4: Implement the four-column semantic list**

Use a `<ul aria-label="Projects">`; each `<li>` contains one canonical project `<Link>`. Render a visible aligned column-header row at wide breakpoints and per-field labels in stacked rows. CSS grid columns are `minmax(180px,1.1fr) minmax(240px,1.5fr) minmax(190px,1fr) minmax(150px,.8fr)`. Use `min-height: 66px`, never fixed height. Target 9–11 rows at 1280×800 with the standard fixture.

The default deterministic order is: source unavailable, setup incomplete, needs commitment, review action, scheduled review, then selected transparent sort. Label the control `Review order`; never call it priority.

Run: `npm test -- src/components/projects/ProjectsIndex.test.tsx src/components/projects/ProjectRow.test.tsx`

Expected: PASS.

- [ ] **Step 5: Commit the visual grammar and Index**

```bash
git add crates/omniproj-desktop/web/src/index.css crates/omniproj-desktop/web/src/components/semantic crates/omniproj-desktop/web/src/components/projects/ProjectsIndex.tsx crates/omniproj-desktop/web/src/components/projects/ProjectRow.tsx crates/omniproj-desktop/web/src/components/projects/ProjectsIndex.test.tsx crates/omniproj-desktop/web/src/components/projects/ProjectRow.test.tsx crates/omniproj-desktop/web/src/routes/ProjectsIndexPage.tsx
git commit -m "feat(web): build the dense semantic projects index"
```

---

## Task 11: Implement Project Overview, Peek, commitment actions, and focus recovery

**Files:**

- Modify: `crates/omniproj-desktop/web/src/App.tsx`
- Create: `crates/omniproj-desktop/web/src/hooks/useMediaQuery.ts`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectOverview.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectPeek.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectFramingForm.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectLifecycleControl.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/CurrentCommitment.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/CommitmentHistory.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ObservedActual.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ReviewReasons.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/SourceRecovery.tsx`
- Create: `crates/omniproj-desktop/web/src/components/projects/ProjectPeek.test.tsx`
- Modify: `crates/omniproj-desktop/web/src/routes/ProjectOverviewPage.tsx`

- [ ] **Step 1: Write the Project Overview content-order tests**

Assert the DOM order is identity/state → all review reasons → current commitment → observed actual definition list → recent transition rail → Open as page. Assert full source path appears only here. `Review action` evidence visibly includes the server-provided seven-day interval and last effective set/confirmed timestamp. Setup focuses objective, then desired outcome, then first commitment. Source failure shows cached facts with timestamp and recovery, never inactivity wording.

Also assert one explicit `Complete setup` submit sends objective, desired outcome, optional phase, first commitment, and expected revision to `complete_project_setup`; success increments revision once, appends one Set transition, and promotes to active. No intermediate framing write is allowed in this path. Lifecycle changes are explicit: `waiting` requires a reason and review date, `parked` requires a reason and allows an optional review date, `archived` requires confirmation, and returning to `active` never fabricates a current commitment. Phase remains an optional Human-authored framing label and never affects review order.

Run: `npm test -- src/components/projects/ProjectPeek.test.tsx -t "content order|setup|source failure"`

Expected: FAIL.

- [ ] **Step 2: Implement shared Overview content**

Use one `ProjectOverview` component for both Peek and full-page route. The inspector wrapper adds navigation/focus behavior only. Render observed data as `<dl>`, review evidence as text rows, and history as an ordered event rail using `ActivityStamp` and `CommitmentStateTag`. Put lifecycle editing in `ProjectLifecycleControl`; send the current revision with each save and retain unsaved reason/review-date input on failure.

Run: `npm test -- src/components/projects/ProjectPeek.test.tsx -t "content order|setup|source failure"`

Expected: PASS.

- [ ] **Step 3: Write lifecycle interaction tests**

Cover explicit Save for set/replace, blur without save, confirm, complete leaving no current commitment, replace reason required, clear, Undo receipt, stale-revision conflict, write failure retaining draft with Retry and Copy text, audit-commit failure with `state_applied: true`, mutation success announcements, and query cache updates. Assert no action auto-creates a replacement after Complete.

Run: `npm test -- src/components/projects/ProjectPeek.test.tsx -t "commitment"`

Expected: FAIL.

- [ ] **Step 4: Implement commitment mutation UI**

Keep drafts local until command success. Disable only the submitted action with semantic disabled tokens; retain readable draft text. On `revision_conflict`, refetch Overview and show a comparison message without discarding the draft. On `audit_commit_failed` with `state_applied: true`, refetch `durable_revision`, announce `State saved; audit commit failed`, and offer audit Retry only—never resend the Human mutation. Show Undo only for the returned newest `undoable_transition_id`.

Run: `npm test -- src/components/projects/ProjectPeek.test.tsx -t "commitment"`

Expected: PASS.

- [ ] **Step 5: Write and implement Peek focus/navigation tests**

At width `>=800`, Index-origin navigation renders a non-modal `aside` sized `clamp(480px, 42vw, 560px)`, focuses its heading or first action, leaves the background navigable, closes on Escape, and restores the originating row by stable DOM ID. In `App.tsx`, compute `effectiveBackgroundLocation = isPeekViewport ? backgroundLocation : undefined`; at `<800`, main Routes use the real detail location and no secondary Peek/Index landmark remains in the DOM or accessibility tree. Direct access always renders full page. At `799px`, Index → detail → Back restores search-param filter/sort, session scroll, and nearest surviving row focus.

Run: `npm test -- src/components/projects/ProjectPeek.test.tsx -t "focus|responsive|direct"`

Expected: PASS.

- [ ] **Step 6: Commit Project detail interactions**

```bash
git add crates/omniproj-desktop/web/src/App.tsx crates/omniproj-desktop/web/src/hooks/useMediaQuery.ts crates/omniproj-desktop/web/src/components/projects/ProjectOverview.tsx crates/omniproj-desktop/web/src/components/projects/ProjectPeek.tsx crates/omniproj-desktop/web/src/components/projects/ProjectFramingForm.tsx crates/omniproj-desktop/web/src/components/projects/ProjectLifecycleControl.tsx crates/omniproj-desktop/web/src/components/projects/CurrentCommitment.tsx crates/omniproj-desktop/web/src/components/projects/CommitmentHistory.tsx crates/omniproj-desktop/web/src/components/projects/ObservedActual.tsx crates/omniproj-desktop/web/src/components/projects/ReviewReasons.tsx crates/omniproj-desktop/web/src/components/projects/SourceRecovery.tsx crates/omniproj-desktop/web/src/components/projects/ProjectPeek.test.tsx crates/omniproj-desktop/web/src/routes/ProjectOverviewPage.tsx
git commit -m "feat(web): add project peek and commitment interactions"
```

---

## Task 12: Implement Add Project and moved-source recovery

**Files:**

- Create: `crates/omniproj-desktop/web/src/platform/dialog.ts`
- Create: `crates/omniproj-desktop/web/src/components/AddProjectDialog.tsx`
- Create: `crates/omniproj-desktop/web/src/components/AddProjectDialog.test.tsx`
- Modify: `crates/omniproj-desktop/web/src/components/AppShell.tsx`
- Modify: `crates/omniproj-desktop/web/src/components/projects/SourceRecovery.tsx`

- [ ] **Step 1: Isolate and test the directory picker**

Wrap `@tauri-apps/plugin-dialog` as:

```ts
export async function chooseProjectDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}
```

Test cancel, one path, and unexpected array defensively.

Run: `npm test -- src/components/AddProjectDialog.test.tsx -t "picker"`

Expected: PASS after the wrapper.

- [ ] **Step 2: Write the complete validation-state tests**

Cover valid preview, duplicate with `Open existing project`, non-Git directory, bare repository, missing path, unreadable path, picker cancellation, retryable observation failure, and register failure. Assert validation never mutates the store and registration remains disabled until a valid preview exists.

Run: `npm test -- src/components/AddProjectDialog.test.tsx -t "validation|duplicate|failure"`

Expected: FAIL.

- [ ] **Step 3: Implement the modal flow**

Use native `<dialog>` with `showModal()` after verifying it in the target Tauri webview. Trap focus, close on Escape, and restore the Add Project trigger. Flow is select → validate → preview name/path → explicit Register. On success, close the dialog, navigate to the canonical Overview URL with Index background state, and focus objective. The project remains `setup` until objective, desired outcome, and first commitment save atomically.

Run: `npm test -- src/components/AddProjectDialog.test.tsx -t "validation|duplicate|failure|success"`

Expected: PASS.

- [ ] **Step 4: Implement relink from SourceRecovery**

Reuse the picker and source validator. Relink requires explicit confirmation, sends `project_id`, new location, and `expected_source_revision`, and returns to the same Overview object. Assert ProjectId, route, framing, current commitment, history, and legacy files remain unchanged. A duplicate new source offers `Open existing project`; it never steals the source.

Run: `npm test -- src/components/AddProjectDialog.test.tsx -t "relink"`

Expected: PASS.

- [ ] **Step 5: Commit onboarding and recovery**

```bash
git add crates/omniproj-desktop/web/src/platform/dialog.ts crates/omniproj-desktop/web/src/components/AddProjectDialog.tsx crates/omniproj-desktop/web/src/components/AddProjectDialog.test.tsx crates/omniproj-desktop/web/src/components/AppShell.tsx crates/omniproj-desktop/web/src/components/projects/SourceRecovery.tsx
git commit -m "feat(web): add project registration and source recovery"
```

---

## Task 13: Add responsive, accessibility, and browser-level R0 gates

**Files:**

- Create: `crates/omniproj-desktop/web/playwright.config.ts`
- Create: `crates/omniproj-desktop/web/e2e/r0-core.spec.ts`
- Create: `crates/omniproj-desktop/web/e2e/responsive.spec.ts`
- Create: `crates/omniproj-desktop/web/e2e/accessibility.spec.ts`
- Modify: `crates/omniproj-desktop/web/src/index.css`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add a deterministic browser test harness**

In Playwright dev mode, inject a mock `window.__TAURI_INTERNALS__` transport backed by the standard 12-project fixture. Reset it before each test. Do not make browser tests depend on the user's real `~/.omniproj` store.

Run: `npx playwright install chromium && npx playwright test --list`

Expected: the three spec files and their named cases are listed.

- [ ] **Step 2: Test the keyboard-only core loop**

Cover Index → filter → project link → Peek → replace with explicit Save → Undo → Escape → restored row; direct deep link; Back/Forward; Add Project modal; relink; refresh partial failure; save failure with preserved draft; and Complete followed by closing without replacement.

Run: `npm run test:e2e -- e2e/r0-core.spec.ts`

Expected: PASS.

- [ ] **Step 3: Test exact responsive boundaries and density**

At `1280×800`, assert 9–11 standard rows are visible. At `1100`, render all four columns and Peek. At `1099` and `800`, compress Observed actual to relative time plus delta and move subject/SHA to detail. At `799` and `640`, render stacked rows and full-page detail. At every width, assert `scrollWidth <= clientWidth`.

Repeat at 200% text zoom and with long project/action/reason fixtures; assert no clipped action or overlapping badge.

Run: `npm run test:e2e -- e2e/responsive.spec.ts`

Expected: PASS.

- [ ] **Step 4: Test accessibility and non-color semantics**

Run axe and fail on critical or serious findings. Capture Light/Dark × normal/high-contrast snapshots, forced-colors, reduced-motion, grayscale, and color-vision emulation screenshots. Assertions must verify visible labels and boundaries remain, not merely store screenshots. Add computed-color contrast checks for every semantic fixture: necessary/small text ≥4.5:1 and focus/control boundaries ≥3:1; record the measured pairs in the test output.

Run: `npm run test:e2e -- e2e/accessibility.spec.ts`

Expected: PASS.

- [ ] **Step 5: Add frontend gates to CI**

Install Node from the repository lockfile, run `npm ci`, `npm test`, and `npm run build` before Rust build. Keep Playwright browser installation and e2e execution in a separate CI job so system dependency failures are distinguishable from unit failures.

Run locally:

```bash
cd crates/omniproj-desktop/web
npm run check
npm run test:e2e
```

Expected: PASS.

- [ ] **Step 6: Commit release gates**

```bash
git add crates/omniproj-desktop/web/playwright.config.ts crates/omniproj-desktop/web/e2e crates/omniproj-desktop/web/src/index.css .github/workflows/ci.yml
git commit -m "test: add R0 interaction and accessibility gates"
```

---

## Task 14: Perform end-to-end migration, production, and product acceptance

**Files:**

- Modify: `README.md`
- Modify only if a verified defect is found: files owned by Tasks 1–13

- [ ] **Step 1: Run the complete automated verification from a clean shell**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
cd crates/omniproj-desktop/web
npm ci
npm test
npm run build
npm run test:e2e
cd ..
cargo tauri build
```

Expected: every command exits `0`. Record exact command outputs in the implementation session handoff; do not summarize unrun checks as passing.

- [ ] **Step 2: Run migration against a copied real-store fixture**

Copy a sanitized `~/.omniproj` fixture to a temporary `OMNIPROJ_HOME`; never test destructive migration against the live store first. Verify project counts, permanent IDs, source paths, and byte hashes of legacy `next.md`, `plan.md`, `auto/`, and `learned.md`. Run migration twice and compare the second tree hash.

Expected: no project/data loss, no legacy Human file changes, and identical second-run tree.

- [ ] **Step 3: Run the production Tauri smoke matrix**

On the built app, verify BrowserRouter navigation under the production asset protocol: `/projects`, direct `/projects/:id/overview`, Back/Forward, app restart with last object URL, Peek/full-page threshold, native directory picker, registration, relink, refresh, all commitment actions, Undo, and source-repository zero writes.

On macOS, run and record a VoiceOver matrix for keyboard-only Index → Peek → commitment save → return, Add Project, source error recovery, and save/Undo live announcements. A visual screenshot or axe result cannot substitute for this manual screen-reader pass.

If production deep-link fallback fails, fix the Tauri navigation fallback while retaining canonical pathnames. Do not replace canonical paths with hash routes.

- [ ] **Step 4: Run the product comprehension fixtures**

With 8–12 realistic project snapshots, run the five-second scan for current commitment/last actual/review reason; run Human vs Observed provenance recognition; and compare pure text against any retained micro-visual. Remove any visualization that does not reduce time or error. Record results without claiming external validity.

- [ ] **Step 5: Document R0 operation and the dogfood gate**

Update README with the canonical routes, local file layout, source read-only guarantee, migration/recovery behavior, keyboard shortcuts, the visible seven-day review rule, and the 2–4 week / five-project / twenty-re-entry dogfood gate. State that R1 is blocked until the approved metrics are observed; do not present the thresholds as scientific universals.

- [ ] **Step 6: Run a final diff and forbidden-surface audit**

```bash
git diff --check
rg -n "advance_task|clarify_task|refine_task|get_graph|get_plan|get_attention|test_reminder|Sparkline|GitGraph|Decisions|Settings" crates/omniproj-desktop/src/lib.rs crates/omniproj-desktop/src/commands.rs crates/omniproj-desktop/web/src/App.tsx crates/omniproj-desktop/web/src/api.ts
rg -n "#[0-9A-Fa-f]{6}|--color-|text-\[(9|10|11)px\]|emoji|health score|priority score" crates/omniproj-desktop/web/src --glob '*.{ts,tsx}'
rg -n "#[0-9A-Fa-f]{6}" crates/omniproj-desktop/web/src/index.css | rg -v '^.*--op-'
```

Expected: `git diff --check` is clean. The TypeScript scan returns no shipped-surface violations. The CSS pipeline permits raw palette values only on `--op-*` token declaration lines and returns no component-rule raw color.

- [ ] **Step 7: Request code review, fix only evidenced issues, and commit docs**

Use `superpowers:requesting-code-review`, then rerun every affected focused test plus the complete automated verification. Commit:

```bash
git add README.md
git commit -m "docs: document the R0 project re-entry loop"
```

---

## Completion Definition

R0 is implementation-complete only when all of the following are evidenced in the same implementation session:

- Existing stores migrate idempotently to schema v2 without changing legacy Human/Agent documents.
- ProjectId survives source relink and every cache/index path remains keyed by ProjectId.
- Setup, set, confirm, complete, replace, clear, status change, and Undo satisfy revision and append-only history tests.
- Source missing/unreadable/non-Git/bare/empty/unborn/detached states remain distinct; failures preserve cached facts and suppress inactivity claims.
- The R0 Tauri allowlist and React entry graph contain no deferred Agent, notification, Attention, Work, Decision, Git graph, attribution, settings, or sparkline surface.
- Index, Peek, full-page detail, Add Project, relink, Back/Forward, focus restoration, and keyboard shortcuts pass unit and browser tests.
- Responsive, 200% text, contrast, forced-colors, reduced-motion, axe, and recorded VoiceOver gates pass.
- Rust workspace tests/clippy/format/build, frontend unit/build/e2e, and production `cargo tauri build` all pass freshly.
- A production smoke confirms canonical routes under Tauri's asset protocol and proves source repository contents remain unchanged.

Passing these engineering gates allows dogfood; it does not authorize R1. R1 begins only after the approved 2–4 week product gate is measured and the R0 interaction loop is demonstrably useful with low maintenance tax.
