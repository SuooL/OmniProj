## Task 4: Deterministic R0 review reasons

Code commit: `60f0a412adcf59875276b474c8931e82588398f5`
(`feat(core): derive deterministic project review reasons`)

### TDD evidence

- RED: `cargo test -p omniproj-core --test review_reasons -- --nocapture` failed with the
  expected unresolved public review API imports before `review.rs` existed.
- GREEN: the same focused command passed 12/12 review-semantic tests after the pure derivation
  was added.

### Semantics delivered

- Exposes `REVIEW_RULE_VERSION = "r0-v1"`, a seven-day default interval, the five typed reason
  codes, and their evidence-bearing public result type.
- Emits reasons in the specified priority: source unavailable, setup incomplete, missing current
  commitment, action review, then scheduled review. Source failure suppresses setup, inactivity,
  and scheduled-review inference, but not the independently actionable missing commitment.
- Replays an effective transition history before checking commitment age. A correction and the
  transition it compensates are excluded from replay, so corrections cannot become clock anchors.
  Effective Set/Replace establish the set and review clocks; only a later effective Confirmed for
  the current WorkItem advances the review clock.
- Consequently, corrected confirmations do not reset review age, replacements start a new clock,
  and corrections of Complete/Replace/Clear restore the prior pointer and its original clock.
- Exact boundaries use `>=` for the configured commitment interval and `<= now` for scheduled
  review. The derivation accepts only state/source/history/now/interval inputs and does not read
  WorkItem update time, repository activity, dirty state, cache age, or any `Actual changed`
  signal.

### Fresh verification

- `cargo test -p omniproj-core --test review_reasons -- --nocapture`: PASS, 12 tests.
- `cargo test -p omniproj-core --lib --tests`: PASS (library 90, source registry 16, lifecycle
  48, schema migration 51, review reasons 12).
- `cargo check --workspace`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Focused self-review

- Reviewed the module and test diff against the Task 4 truth table. No task-scoped defects
  found. Changes are limited to the two requested core source files and the requested integration
  test; no ledger or plan file was changed.
