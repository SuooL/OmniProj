# Task 8 Report: React Contract, Test Harness, and Query Boundary

## Scope

Task 8 only, entirely under `crates/omniproj-desktop/web`. Establishes the frontend's typed
contract with the Task 7 backend, the pull-only query boundary, and a Vitest test harness;
archives the pre-R0 UI out of the build graph. No Rust changed.

BASE `ef0227a`, HEAD `c70a188`.

## RED → GREEN Evidence

`src/api.test.ts` and `src/domain/projectPresentation.test.ts` were written against the new
contract before the source existed and failed to compile/resolve; after the domain + api +
presentation modules were implemented, `npm test` passed 32/32 and `npm run build`
(tsc + vite) enforced the types at compile time.

## GREEN Implementation

- **Wire types once (`domain/project.ts`).** Every Task 7 DTO mirrored with exact snake_case
  fields and snake_case enum values: `ProjectIndexResponse/Item`, `ProjectOverview`,
  `ProjectSource`, `CurrentCommitment`, `ObservedActual`, `CommitmentTransition` (with the
  `type` discriminant), tagged `HeadState` (`kind`) and `SourceValidation` (`state`),
  `RefreshResult`/`RefreshOutcome`, and all command input DTOs. `ProjectId`, `WorkItemId`,
  and `TransitionId` (plus `ProjectSourceId`) are branded strings so unrelated ids cannot be
  passed by accident. `SourceValidation` models only the valid preview + the typed states;
  missing/unreadable/non-Git/bare/duplicate failures arrive as `CommandError` (duplicate
  carrying `existing_project_id`).
- **Typed errors (`domain/errors.ts`).** The 20-code `ErrorCode`, the `CommandError` wire
  shape, and an `AppError` with a derived `recovery`: `audit_commit_failed`
  (`state_applied`) → `refetch` (never resend), `revision_conflict` → `refetch`, otherwise
  `retryable` → `retry`, else `none`. `classifyError` turns a structured rejection into a
  typed `AppError` and flattens any unknown rejection to a safe generic message with no
  leaked internals.
- **R0 api surface (`api.ts`).** Exactly the 15 commands, each invoked with one top-level
  snake_case `input`; `list_project_index` takes no args. Every deferred wrapper
  (Agent/Decision/Graph/Task/Attention/Settings) is gone. Rejections are routed through
  `classifyError`.
- **Legacy archive (`web/legacy-src/`).** `ProjectCard`, `ProjectDetail`, `GitGraph`,
  `Decisions`, `Settings`, `Sparkline`, `staleness.ts`, `App.tsx`, and a copy of the pre-R0
  `api.ts` moved via `git mv` (history preserved) into `legacy-src`, which is excluded from
  `tsconfig.json`'s `src` include and imported by nothing in `src/` — so it is never
  type-checked or bundled (the TS analogue of `legacy.rs`).
- **Pull-only query boundary.** `queryClient.ts` sets `staleTime: Infinity`,
  `refetchOnWindowFocus/Reconnect/Mount: false`, `retry: 1`, `mutations.retry: 0`, and
  provides `applyOverviewToCaches` to fold a returned Overview into the Overview cache and
  patch the Index row in place — no refetch. `queryKeys.ts` holds stable keys for Index,
  Overview, validation, and refresh.
- **Presentation (`domain/projectPresentation.ts`).** Immutable text/`all|needs_review|
  waiting|parked` filters, defensive archived exclusion, a transparent non-ranking order
  label, backend-priority primary reason + `+N` accessible name enumerating hidden reasons,
  and relative-time with an exact `title`. No function takes commit counts (or any observed
  activity) as a priority/health/order input; a test asserts activity cannot change reason
  display or filter membership.
- **Harness/config.** `react-router-dom` + `@tauri-apps/plugin-dialog` runtime deps;
  Vitest/jsdom/Testing-Library/Playwright/axe dev deps; Vitest configured in `vite.config.ts`
  (jsdom, setup file, css, mock reset) with the obsolete `/api` proxy removed; scripts
  `test`, `test:watch`, `test:e2e`, `check`.

## Verification (fresh command output)

```text
cd crates/omniproj-desktop/web
npm test          # Test Files 2 passed, Tests 32 passed
npm run build     # tsc -b clean; vite build ✓ (78 modules)
npm run check     # build + test, all green
cargo check -p omniproj-desktop   # Finished (rebuilt dist embeds cleanly)
```

## Independent review

An independent contract review cross-checked every TS type field-by-field against
`dto.rs`/`error.rs`/`commands.rs` and the core enums. Verdict: **the wire contract matches
the Rust source of truth exactly — no Critical or Important defects.** Four Minor
observations; addressed in `c8483d2`:

- Added `queryClient.test.ts` covering `applyOverviewToCaches` (matched-row patch,
  untouched-array identity return, and the `source == null` keep-prior path) — the review's
  top Minor (the most bug-prone function previously had no direct test).
- Corrected the stale `SourceValidation` doc comment (recoverable rejections are typed
  states, not `CommandError`).
- Left as-is with rationale: recovery `none` for `current_commitment_changed` /
  `no_current_commitment` / `undo_conflict` / `transition_not_found` conforms to the
  acceptance list ("else none") and the `expected_revision` guard makes `revision_conflict`
  fire first; the `project_id` string-vs-branded cosmetic; the post-mutation index re-sort
  is Task 9's render-layer concern (Task 8 ships a placeholder `App`).

Frontend suite is now 35 tests across 3 files.

## Commits

- `c70a188` `test(web): establish the R0 frontend contract`
- `c8483d2` `test(web): cover the overview cache-fold; clarify source validation doc`
