## Task 5: Typed, read-only repository observations

Code commit: `5a1d8dc46b814c58a947f128d740c54f30777ab5`
(`feat(capture): report typed repository observations`)

### TDD evidence

- New observation API RED: `cargo test -p omniproj-capture --test git_observation --
  --nocapture` failed with the expected E0432 unresolved imports for `observe_repository`,
  `count_commits_since`, `HeadState`, and `RepositoryReadErrorKind`.
- Observation GREEN: the same focused command passed 8/8 hermetic integration tests.
- Commit timestamp RED: the focused inline test failed with E0609 because `CommitEntry` did not
  yet expose `committed_at`; it passed after the single log format parsed `%cI` and `%cs`.
- User-config isolation RED: setting `status.showUntrackedFiles=no` in the fixture reduced the
  observed counts from five changed/one untracked to four changed/zero untracked. Explicit
  `--untracked-files=all` restored the expected counts and the focused test passed.

### Semantics delivered

- Added typed observation results and typed read failures for missing paths, permission denial,
  non-repositories, bare repositories, unavailable Git, failed commands, and invalid output.
- Empty repositories report `Unborn`, attached repositories include their branch, and detached
  repositories remain successful observations. `count_commits_since` returns zero for an unborn
  repository.
- Every new source-repository Git command uses `git --no-optional-locks -C <repo> ...` with
  `GIT_OPTIONAL_LOCKS=0`. Command status and stderr are retained for classification, and Git
  stdout is parsed as strict UTF-8 rather than lossily accepted.
- Porcelain counting is path-based: the fixture proves `changed=5`, `staged=3`, `unstaged=2`, and
  `untracked=1` with a staged rename and a file that is both staged and unstaged. The latter is
  one changed path, not two.
- Last-commit observation returns full/short SHA, subject, and fixed RFC3339 committer timestamp.
  Legacy `CommitEntry.date` remains available while `committed_at` preserves `%cI`.
- Existing `collect`, `commit_log`, and `commit_graph` public behavior remains fail-soft; only the
  requested additional timestamp field and its one-command parsing format changed for
  `commit_log`.

### Read-only proof

- The read-only fixture snapshots the complete repository tree, including relative paths, entry
  types, contents or symlink targets, modes, and mtimes, before and after observation and commit
  counting.
- It separately compares `.git/index` byte digest and mtime. Fixture mutation commands all run
  before the snapshot; no ordinary test-side `git status` runs after it, so the test cannot refresh
  the index it is trying to protect.
- Git unavailable, arbitrary command failure, and invalid successful output use a child test
  process with a private `PATH`; they do not mutate the parent test environment or add a
  production test injection seam.

### Fresh verification

- `cargo test -p omniproj-capture --test git_observation -- --nocapture`: PASS, 8 tests.
- `cargo test -p omniproj-capture --locked`: PASS, 24 unit tests plus 8 integration tests.
- `cargo check --workspace --locked`: PASS; only pre-existing deprecated API warnings in
  CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Focused self-review and platform boundary

- Reviewed the final diff against every Task 5 fixture/error/state/count/timestamp requirement.
  Code changes are limited to `git.rs` and the requested integration test; no ledger or plan file
  changed.
- The unreadable-directory assertion is Unix-only because it uses mode bits. It runs normally
  when permissions are enforced and reports an explicit skip only for privileged processes that
  can bypass mode `000`; all other typed-error fixtures remain cross-platform or child-process
  isolated as annotated in the test.

## Fix Round 1

Code commit: `5f6bcef93a3fd4664ea58aa059babc8d3ffbc338`
(`fix(capture): canonicalize repository observations`)

### Reviewer findings and TDD evidence

- Porcelain config-drift RED: the focused fixture set `status.renames=false` and expected one
  logical staged rename. The old line parser reported `changed=6` instead of `5` because Git
  emitted delete plus add records. The fixture also carries the ordinary path
  `space -> café\nline.txt` and switches `core.quotePath` in both directions.
- Porcelain GREEN: the focused config/special-path test passed 1/1 after command-level config,
  NUL parsing, and semantic digest canonicalization. Both config states now report exactly
  `changed=5`, `staged=3`, `unstaged=2`, `untracked=1`, and the same digest.
- Timestamp-boundary RED: an isolated child-process fake Git was spawned for invalid input and
  the API returned `CommandFailed` instead of the selected `InvalidOutput`; the marker proved
  validation occurred too late.
- Timestamp-boundary GREEN: the focused no-spawn marker test passed 1/1 for `tomorrow`, another
  malformed value, and a naive timestamp in both public APIs. Legal offset timestamps remain
  accepted, and `observed_at` is retained exactly as supplied.

### Fix implementation

- Every typed read now invokes Git with global options ordered before `-C`:
  `--no-optional-locks -c status.renames=true -c core.quotePath=false -C <repo>`. The defensive
  `GIT_OPTIONAL_LOCKS=0` environment setting remains on every command.
- Status uses `--porcelain=v1 -z --untracked-files=all`. The byte parser treats NUL as the only
  record/path delimiter, so spaces, literal arrows, non-ASCII bytes, and newlines do not alter
  record boundaries. Rename/copy records consume their second path but contribute one logical
  changed path.
- `status_digest` is now computed from sorted semantic records. Each record encodes `XY` plus
  byte lengths and hexadecimal path bytes, including both rename/copy paths; raw quoting,
  repository config, and record order do not enter the digest.
- `observe_repository` and `count_commits_since` strictly parse their timestamp input with
  `chrono::DateTime::parse_from_rfc3339` before filesystem/Git inspection. Invalid input uses the
  existing `InvalidOutput` kind with a field-specific RFC3339 message rather than expanding the
  plan's fixed error enum.

### Fresh verification

- `cargo test -p omniproj-capture --test git_observation -- --nocapture`: PASS, 10 tests.
- `cargo test -p omniproj-capture --locked`: PASS, 24 unit tests plus 10 integration tests.
- `cargo check --workspace --locked`: PASS; only pre-existing deprecated API warnings in
  CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Fix Round 1 boundaries

- Full-tree and `.git/index` byte-digest/mtime snapshots pass around both config variants; test
  fixture config writes occur before each snapshot, and observation itself remains read-only.
- Legacy `collect`, `commit_log`, and `commit_graph` remain outside the typed reader and retain
  their prior behavior.

## Fix Round 2

Code commit: `a4a423adc3fee6660911d82ea65a48f2fc0ad14c`
(`fix(capture): validate porcelain status states`)

### Reviewer finding and TDD evidence

- RED: the public-API fake-Git probe successfully supplied bare-repository, symbolic HEAD,
  verified HEAD, and last-commit outputs, then returned a blank/blank status record. The parser
  incorrectly returned a successful observation with `changed=1`, zero category counts, and a
  digest instead of `InvalidOutput`.
- GREEN: the malformed-status matrix passed 1/1 after XY validation. Nine child-process cases now
  return typed `InvalidOutput` without a production panic: blank/blank, single-sided `?`,
  single-sided `!`, a legal-character but illegal `AC` pair, an unknown code, a truncated normal
  record, a rename without its second NUL terminator, a copy with an empty second path, and valid
  status followed by trailing non-NUL bytes.
- A real-Git state matrix passed before and after the fix, demonstrating that the validator did
  not reject the supported ordinary states. It covers unstaged/staged/both-modified, staged add,
  added-then-modified, staged/worktree delete, intent-to-add, staged rename, staged/worktree type
  changes, and untracked files.

### Fix implementation

- Porcelain special states are limited to the exact `??` and `!!` pairs.
- Unmerged states are limited to Git's exact `DD`, `AU`, `UD`, `UA`, `DU`, `AA`, and `UU` pairs;
  `U` cannot enter an ordinary state.
- Ordinary states follow the porcelain-v1 XY table while retaining `T`, staged-plus-unstaged
  combinations, and worktree `A` for intent-to-add. Blank/blank, single-sided special markers,
  unknown codes, and invalid legal-code combinations are rejected.
- Rename/copy codes are accepted only on their valid side/pair combinations and still require a
  second non-empty NUL-delimited path. Canonical semantic encoding and all count logic remain
  unchanged after successful validation.

### Fresh verification

- `cargo test -p omniproj-capture --test git_observation -- --nocapture`: PASS, 13 tests.
- `cargo test -p omniproj-capture --locked`: PASS, 24 unit tests plus 13 integration tests.
- `cargo check --workspace --locked`: PASS; only pre-existing deprecated API warnings in
  CLI/Desktop.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

### Fix Round 2 boundaries

- The real state matrix reports exactly 12 changed, 7 staged, 6 unstaged, and 1 untracked file;
  its full-tree and `.git/index` byte-digest/mtime snapshot remains unchanged by observation.
- Fake Git is isolated in exact child-test processes and mutates neither the source fixture nor
  the parent test environment. Legacy Git APIs and the Task 5 public types remain unchanged.
