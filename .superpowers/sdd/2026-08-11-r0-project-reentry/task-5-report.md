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
