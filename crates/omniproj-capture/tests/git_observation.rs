use omniproj_capture::git::{
    count_commits_since, observe_repository, HeadState, RepositoryReadErrorKind,
};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "omniproj-observation-{}-{tag}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fixture directory");
        Self { path }
    }

    fn init(&self) {
        self.git(&["init", "-q", "-b", "main"]);
        self.git(&["config", "user.name", "Observation Test"]);
        self.git(&["config", "user.email", "observation@test.invalid"]);
        self.git(&["config", "commit.gpgsign", "false"]);
        self.git(&["config", "core.excludesFile", "/dev/null"]);
        self.git(&["config", "core.fsmonitor", "false"]);
        self.git(&["config", "core.untrackedCache", "false"]);
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, contents).expect("write fixture file");
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("Git must be available to build repository fixtures");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("fixture Git output is UTF-8")
    }

    fn commit_at(&self, subject: &str, committed_at: &str) {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(["commit", "-q", "-m", subject])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_DATE", committed_at)
            .env("GIT_COMMITTER_DATE", committed_at)
            .output()
            .expect("Git must be available to commit fixtures");
        assert!(
            output.status.success(),
            "fixture commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct EntrySnapshot {
    kind: &'static str,
    contents: Vec<u8>,
    modified: Option<SystemTime>,
    mode: u32,
}

/// Snapshot the working tree — every file, directory and symlink under `root`
/// **except the `.git` control directory**.
///
/// The guarantee under test is OmniProj's promise never to modify your *source*
/// repository: your tracked/untracked working-tree files and their metadata. It
/// is emphatically *not* a promise about Git's own bookkeeping inside `.git/`.
/// Ordinary read-only commands (`git status`, `git log --since`, `git rev-list`)
/// legitimately refresh `.git/` internals — the index stat-cache, the
/// commit-graph, gc/maintenance state — and a newer Git on Linux does so where
/// macOS's did not. Snapshotting `.git/` (with directory mtimes) therefore made
/// this test flaky on CI while proving nothing about the source tree. The
/// staged-state guarantee is asserted separately and precisely via
/// [`index_snapshot`].
fn tree_snapshot(root: &Path) -> BTreeMap<PathBuf, EntrySnapshot> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, EntrySnapshot>) {
        if path.file_name().is_some_and(|name| name == ".git") {
            return;
        }
        let metadata = fs::symlink_metadata(path).expect("snapshot metadata");
        let file_type = metadata.file_type();
        let (kind, contents) = if file_type.is_file() {
            ("file", fs::read(path).expect("snapshot file bytes"))
        } else if file_type.is_dir() {
            ("directory", Vec::new())
        } else if file_type.is_symlink() {
            (
                "symlink",
                fs::read_link(path)
                    .expect("snapshot symlink target")
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec(),
            )
        } else {
            ("other", Vec::new())
        };
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode = u32::from(metadata.permissions().readonly());

        entries.insert(
            path.strip_prefix(root)
                .unwrap_or(Path::new("."))
                .to_path_buf(),
            EntrySnapshot {
                kind,
                contents,
                modified: metadata.modified().ok(),
                mode,
            },
        );
        if file_type.is_dir() {
            let mut children = fs::read_dir(path)
                .expect("snapshot directory")
                .map(|entry| entry.expect("snapshot directory entry").path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                visit(root, &child, entries);
            }
        }
    }

    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

/// Human-readable, field-level description of how two working-tree snapshots
/// differ. Used to pin down exactly which path (and which attribute) a
/// "read-only" operation touched, so a failure is diagnosable from the log alone.
fn diff_tree(
    before: &BTreeMap<PathBuf, EntrySnapshot>,
    after: &BTreeMap<PathBuf, EntrySnapshot>,
) -> Vec<String> {
    let mut diffs = Vec::new();
    let mut keys: Vec<&PathBuf> = before.keys().chain(after.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let rel = key.display();
        match (before.get(key), after.get(key)) {
            (None, Some(_)) => diffs.push(format!("+ added   {rel}")),
            (Some(_), None) => diffs.push(format!("- removed {rel}")),
            (Some(b), Some(a)) if a != b => {
                let mut fields = Vec::new();
                if a.kind != b.kind {
                    fields.push(format!("kind {}→{}", b.kind, a.kind));
                }
                if a.contents != b.contents {
                    fields.push(format!(
                        "contents {}B→{}B",
                        b.contents.len(),
                        a.contents.len()
                    ));
                }
                if a.modified != b.modified {
                    fields.push("mtime".to_string());
                }
                if a.mode != b.mode {
                    fields.push(format!("mode {:o}→{:o}", b.mode, a.mode));
                }
                diffs.push(format!("~ changed {rel} ({})", fields.join(", ")));
            }
            _ => {}
        }
    }
    diffs
}

#[derive(Debug, PartialEq, Eq)]
struct IndexSnapshot {
    digest: u64,
    modified: SystemTime,
}

fn index_snapshot(repo: &Path) -> IndexSnapshot {
    let index = repo.join(".git/index");
    let bytes = fs::read(&index).expect("read repository index");
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    IndexSnapshot {
        digest: hasher.finish(),
        modified: fs::metadata(index)
            .expect("index metadata")
            .modified()
            .expect("index mtime"),
    }
}

fn observe_without_source_writes(repo: &Path) -> omniproj_capture::git::RepositoryObservation {
    let tree_before = tree_snapshot(repo);
    let index_before = index_snapshot(repo);
    let observation = observe_repository(repo, "2026-08-11T12:00:00Z").unwrap();
    assert_eq!(
        tree_snapshot(repo),
        tree_before,
        "observation changed source"
    );
    assert_eq!(
        index_snapshot(repo),
        index_before,
        "observation changed index"
    );
    observation
}

#[test]
fn observation_buckets_sixteen_utc_calendar_weeks_oldest_to_newest() {
    let fixture = Fixture::new("activity-weeks");
    fixture.init();
    for (index, at) in [
        "2026-07-27T08:00:00Z",
        "2026-08-03T08:00:00Z",
        "2026-08-10T08:00:00Z",
    ]
    .into_iter()
    .enumerate()
    {
        fixture.write("activity.txt", &format!("{index}\n"));
        fixture.git(&["add", "activity.txt"]);
        fixture.commit_at(&format!("activity {index}"), at);
    }

    let observed = observe_repository(&fixture.path, "2026-08-11T12:00:00Z").unwrap();
    assert_eq!(observed.commit_activity_weeks.len(), 16);
    assert_eq!(&observed.commit_activity_weeks[13..], &[1, 1, 1]);
    assert_eq!(observed.commit_activity_weeks[..13].iter().sum::<u32>(), 0);
}

fn assert_error_kind(path: &Path, expected: RepositoryReadErrorKind) {
    let error = observe_repository(path, "2026-08-11T12:00:00Z")
        .expect_err("repository inspection must fail");
    assert_eq!(error.kind, expected, "unexpected error: {error:?}");
}

#[test]
fn classifies_missing_non_repository_and_bare_paths() {
    let missing = std::env::temp_dir().join(format!(
        "omniproj-missing-observation-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
    ));
    assert_error_kind(&missing, RepositoryReadErrorKind::PathMissing);

    let plain = Fixture::new("plain");
    assert_error_kind(&plain.path, RepositoryReadErrorKind::NotRepository);

    let bare = Fixture::new("bare");
    bare.git(&["init", "-q", "--bare"]);
    assert_error_kind(&bare.path, RepositoryReadErrorKind::BareRepository);
}

#[cfg(unix)]
#[test]
fn classifies_an_unreadable_directory_when_permissions_are_enforced() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("unreadable");
    let original = fs::metadata(&fixture.path).unwrap().permissions();
    fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o000)).unwrap();
    let permissions_are_enforced = fs::read_dir(&fixture.path).is_err();
    let result = observe_repository(&fixture.path, "2026-08-11T12:00:00Z");
    fs::set_permissions(&fixture.path, original).unwrap();

    if !permissions_are_enforced {
        eprintln!("skipping unreadable-directory assertion: process bypasses mode bits");
        return;
    }
    let error = result.expect_err("unreadable directory must fail");
    assert_eq!(error.kind, RepositoryReadErrorKind::PermissionDenied);
}

#[test]
fn reports_unborn_attached_and_detached_head_states() {
    let fixture = Fixture::new("heads");
    fixture.init();

    let unborn = observe_repository(&fixture.path, "2026-08-11T12:00:00Z").unwrap();
    assert_eq!(
        unborn.head_state,
        HeadState::Unborn {
            branch: Some("main".into())
        }
    );
    assert_eq!(unborn.head_sha, None);
    assert_eq!(unborn.last_commit, None);
    assert_eq!(
        count_commits_since(&fixture.path, "2000-01-01T00:00:00Z").unwrap(),
        0
    );

    fixture.write("README.md", "fixture\n");
    fixture.git(&["add", "README.md"]);
    fixture.commit_at("initial", "2025-03-04T05:06:07+02:00");
    let attached = observe_repository(&fixture.path, "2026-08-11T12:00:00Z").unwrap();
    assert_eq!(
        attached.head_state,
        HeadState::Attached {
            branch: "main".into()
        }
    );
    assert!(attached.head_sha.is_some());

    fixture.git(&["checkout", "-q", "--detach", "HEAD"]);
    let detached = observe_repository(&fixture.path, "2026-08-11T12:00:00Z").unwrap();
    assert_eq!(detached.head_state, HeadState::Detached);
    assert_eq!(detached.head_sha, attached.head_sha);
}

#[test]
fn porcelain_counts_and_digest_ignore_repository_config_and_path_syntax() {
    let fixture = Fixture::new("porcelain");
    fixture.init();
    for name in ["staged.txt", "both.txt", "unstaged.txt", "old.txt"] {
        fixture.write(name, "base\n");
    }
    fixture.git(&["add", "."]);
    fixture.commit_at("base", "2025-01-01T00:00:00Z");

    fixture.write("staged.txt", "staged\n");
    fixture.git(&["add", "staged.txt"]);
    fixture.write("both.txt", "index version\n");
    fixture.git(&["add", "both.txt"]);
    fixture.write("both.txt", "worktree version\n");
    fixture.write("unstaged.txt", "unstaged\n");
    fixture.git(&["mv", "old.txt", "renamed.txt"]);
    fixture.write("space -> café\nline.txt", "untracked\n");
    // Observation semantics must not inherit a user preference that hides
    // untracked files from ordinary status output.
    fixture.git(&["config", "status.showUntrackedFiles", "no"]);
    fixture.git(&["config", "status.renames", "false"]);
    fixture.git(&["config", "core.quotePath", "true"]);

    let quoted = observe_without_source_writes(&fixture.path);
    assert_eq!(quoted.changed_files, 5, "rename is one logical path change");
    assert_eq!(quoted.staged_files, 3, "staged + both + rename");
    assert_eq!(quoted.unstaged_files, 2, "unstaged + both");
    assert_eq!(quoted.untracked_files, 1);
    assert_eq!(quoted.status_digest.len(), 16);
    assert!(quoted
        .status_digest
        .chars()
        .all(|character| character.is_ascii_hexdigit()));

    fixture.git(&["config", "status.renames", "true"]);
    fixture.git(&["config", "core.quotePath", "false"]);
    let unquoted = observe_without_source_writes(&fixture.path);
    assert_eq!(unquoted.changed_files, quoted.changed_files);
    assert_eq!(unquoted.staged_files, quoted.staged_files);
    assert_eq!(unquoted.unstaged_files, quoted.unstaged_files);
    assert_eq!(unquoted.untracked_files, quoted.untracked_files);
    assert_eq!(
        unquoted.status_digest, quoted.status_digest,
        "digest is derived from canonical semantic records"
    );
}

#[test]
fn reports_fixed_rfc3339_commit_metadata_and_since_counts() {
    let fixture = Fixture::new("timestamp");
    fixture.init();
    fixture.write("a.txt", "a\n");
    fixture.git(&["add", "a.txt"]);
    fixture.commit_at("fixed timestamp", "2025-03-04T05:06:07+02:00");

    let observed = observe_repository(&fixture.path, "2026-08-11T12:00:00-04:00").unwrap();
    assert_eq!(observed.observed_at, "2026-08-11T12:00:00-04:00");
    let commit = observed.last_commit.expect("last commit");
    assert_eq!(commit.committed_at, "2025-03-04T05:06:07+02:00");
    assert_eq!(commit.subject, "fixed timestamp");
    assert_eq!(commit.sha.len(), 40);
    assert!(commit.sha.starts_with(&commit.short_sha));
    assert_eq!(
        count_commits_since(&fixture.path, "2025-03-03T00:00:00-04:00").unwrap(),
        1
    );
    assert_eq!(
        count_commits_since(&fixture.path, "2025-03-05T00:00:00Z").unwrap(),
        0
    );
}

#[test]
fn observation_and_commit_count_do_not_modify_the_source_tree_or_index() {
    let fixture = Fixture::new("readonly");
    fixture.init();
    fixture.write("tracked.txt", "base\n");
    fixture.git(&["add", "tracked.txt"]);
    fixture.commit_at("base", "2025-01-01T00:00:00Z");
    fixture.write("tracked.txt", "dirty\n");
    fixture.write("untracked.txt", "untracked\n");

    // Snapshot directly from the filesystem. Running ordinary `git status` here would
    // itself be capable of refreshing the index and invalidate the read-only proof.
    let tree_before = tree_snapshot(&fixture.path);
    let index_before = index_snapshot(&fixture.path);

    observe_repository(&fixture.path, "2026-08-11T12:00:00Z").unwrap();
    count_commits_since(&fixture.path, "2024-01-01T00:00:00Z").unwrap();

    let tree_after = tree_snapshot(&fixture.path);
    let index_after = index_snapshot(&fixture.path);
    let tree_diff = diff_tree(&tree_before, &tree_after);
    assert!(
        tree_diff.is_empty(),
        "repository tree changed during reads:\n{}",
        tree_diff.join("\n")
    );
    assert_eq!(index_after, index_before, "Git index hash or mtime changed");
}

#[cfg(unix)]
fn run_error_probe(expected: &str, script: Option<&str>) {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("probe");
    let bin = fixture.path.join("bin");
    fs::create_dir(&bin).unwrap();
    if let Some(script) = script {
        let git = bin.join("git");
        fs::write(&git, script).unwrap();
        fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_git_error_probe", "--nocapture"])
        .env("OMNIPROJ_GIT_ERROR_PROBE", expected)
        .env("OMNIPROJ_GIT_ERROR_PATH", &fixture.path)
        .env("PATH", &bin)
        .output()
        .expect("run isolated probe process");
    assert!(
        output.status.success(),
        "probe {expected} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn classifies_git_unavailable_command_failure_and_invalid_output() {
    run_error_probe("GitUnavailable", None);
    run_error_probe(
        "CommandFailed",
        Some("#!/bin/sh\necho forced failure >&2\nexit 23\n"),
    );
    run_error_probe(
        "InvalidOutput",
        Some("#!/bin/sh\necho malformed-bare-value\nexit 0\n"),
    );
}

#[test]
fn child_git_error_probe() {
    let Ok(expected) = std::env::var("OMNIPROJ_GIT_ERROR_PROBE") else {
        return;
    };
    let path = PathBuf::from(std::env::var_os("OMNIPROJ_GIT_ERROR_PATH").unwrap());
    let error = observe_repository(&path, "2026-08-11T12:00:00Z")
        .expect_err("probe must produce a typed error");
    assert_eq!(format!("{:?}", error.kind), expected);
}

#[cfg(unix)]
#[test]
fn invalid_rfc3339_inputs_do_not_spawn_git() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("invalid-input-probe");
    let source = fixture.path.join("source");
    let bin = fixture.path.join("bin");
    let marker = fixture.path.join("git-was-spawned");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&bin).unwrap();
    let git = bin.join("git");
    fs::write(
        &git,
        "#!/bin/sh\n: > \"$OMNIPROJ_GIT_SPAWN_MARKER\"\nexit 71\n",
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_invalid_rfc3339_probe", "--nocapture"])
        .env("OMNIPROJ_INVALID_INPUT_SOURCE", source)
        .env("OMNIPROJ_GIT_SPAWN_MARKER", &marker)
        .env("PATH", bin)
        .output()
        .expect("run isolated invalid-input probe");
    assert!(
        output.status.success(),
        "invalid-input probe failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "invalid input spawned Git");
}

#[test]
fn child_invalid_rfc3339_probe() {
    let Some(source) = std::env::var_os("OMNIPROJ_INVALID_INPUT_SOURCE") else {
        return;
    };
    let source = PathBuf::from(source);
    for invalid in ["tomorrow", "invalid", "2026-08-11T12:00:00"] {
        let observed = observe_repository(&source, invalid).expect_err("invalid observed_at");
        assert_eq!(observed.kind, RepositoryReadErrorKind::InvalidOutput);
        assert!(observed.message.contains("observed_at"));
        assert!(observed.message.contains("RFC3339"));

        let since = count_commits_since(&source, invalid).expect_err("invalid since_rfc3339");
        assert_eq!(since.kind, RepositoryReadErrorKind::InvalidOutput);
        assert!(since.message.contains("since_rfc3339"));
        assert!(since.message.contains("RFC3339"));
    }
}

#[cfg(unix)]
fn run_malformed_status_probe(case: &str) {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("malformed-status");
    let source = fixture.path.join("source");
    let bin = fixture.path.join("bin");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&bin).unwrap();
    let git = bin.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
case " $* " in
  *" rev-parse --is-bare-repository "*) printf 'false\n' ;;
  *" symbolic-ref --short -q HEAD "*) printf 'main\n' ;;
  *" rev-parse --verify HEAD "*) printf '1111111111111111111111111111111111111111\n' ;;
  *" log -1 "*) printf '1111111111111111111111111111111111111111\0371111111\037subject\0372025-01-01T00:00:00Z' ;;
  *" status --porcelain=v1 -z --untracked-files=all "*)
    case "$OMNIPROJ_MALFORMED_STATUS" in
      blank_blank) printf '   path\000' ;;
      single_question) printf '?  path\000' ;;
      single_bang) printf ' ! path\000' ;;
      illegal_combination) printf 'AC new\000old\000' ;;
      unknown_code) printf 'Z  path\000' ;;
      deleted_modified) printf 'DM path\000' ;;
      deleted_typechanged) printf 'DT path\000' ;;
      deleted_renamed) printf 'DR new\000old\000' ;;
      deleted_copied) printf 'DC new\000old\000' ;;
      truncated_normal) printf 'M  path' ;;
      rename_missing_second_nul) printf 'R  new\000old' ;;
      copy_empty_second_path) printf 'C  new\000\000' ;;
      trailing_non_nul) printf 'M  path\000tail' ;;
      *) exit 73 ;;
    esac
    ;;
  *) exit 74 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_malformed_status_probe", "--nocapture"])
        .env("OMNIPROJ_MALFORMED_STATUS", case)
        .env("OMNIPROJ_MALFORMED_STATUS_SOURCE", source)
        .env("PATH", bin)
        .output()
        .expect("run malformed-status probe");
    assert!(
        output.status.success(),
        "malformed status {case} was not rejected:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn malformed_porcelain_statuses_are_typed_errors_without_panics() {
    for case in [
        "blank_blank",
        "single_question",
        "single_bang",
        "illegal_combination",
        "unknown_code",
        "deleted_modified",
        "deleted_typechanged",
        "deleted_renamed",
        "deleted_copied",
        "truncated_normal",
        "rename_missing_second_nul",
        "copy_empty_second_path",
        "trailing_non_nul",
    ] {
        run_malformed_status_probe(case);
    }
}

#[test]
fn child_malformed_status_probe() {
    let Some(source) = std::env::var_os("OMNIPROJ_MALFORMED_STATUS_SOURCE") else {
        return;
    };
    let result = observe_repository(Path::new(&source), "2026-08-11T12:00:00Z");
    let error = result.expect_err("malformed status must not produce an observation");
    assert_eq!(error.kind, RepositoryReadErrorKind::InvalidOutput);
}

#[cfg(unix)]
#[test]
fn accepts_real_porcelain_v1_state_matrix() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("status-matrix");
    fixture.init();
    for name in [
        "unstaged.txt",
        "staged.txt",
        "both.txt",
        "delete-index.txt",
        "delete-worktree.txt",
        "rename-source.txt",
        "type-unstaged.txt",
        "type-staged.txt",
    ] {
        fixture.write(name, "base\n");
    }
    fixture.git(&["add", "."]);
    fixture.commit_at("status matrix base", "2025-01-01T00:00:00Z");

    fixture.write("unstaged.txt", "worktree\n");
    fixture.write("staged.txt", "index\n");
    fixture.git(&["add", "staged.txt"]);
    fixture.write("both.txt", "index\n");
    fixture.git(&["add", "both.txt"]);
    fixture.write("both.txt", "worktree\n");
    fixture.write("new-staged.txt", "new\n");
    fixture.git(&["add", "new-staged.txt"]);
    fixture.write("added-modified.txt", "index\n");
    fixture.git(&["add", "added-modified.txt"]);
    fixture.write("added-modified.txt", "worktree\n");
    fixture.git(&["rm", "-q", "delete-index.txt"]);
    fs::remove_file(fixture.path.join("delete-worktree.txt")).unwrap();
    fixture.write("intent-to-add.txt", "intent\n");
    fixture.git(&["add", "-N", "intent-to-add.txt"]);
    fixture.git(&["mv", "rename-source.txt", "rename-target.txt"]);
    fixture.write("untracked.txt", "untracked\n");

    fs::remove_file(fixture.path.join("type-unstaged.txt")).unwrap();
    symlink("unstaged.txt", fixture.path.join("type-unstaged.txt")).unwrap();
    fs::remove_file(fixture.path.join("type-staged.txt")).unwrap();
    symlink("staged.txt", fixture.path.join("type-staged.txt")).unwrap();
    fixture.git(&["add", "type-staged.txt"]);

    let observed = observe_without_source_writes(&fixture.path);
    assert_eq!(observed.changed_files, 12);
    assert_eq!(observed.staged_files, 7);
    assert_eq!(observed.unstaged_files, 6);
    assert_eq!(observed.untracked_files, 1);
}
