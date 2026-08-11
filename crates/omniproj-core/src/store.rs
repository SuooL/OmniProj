//! `~/.omniproj` self-versioning (spec §5 provenance). This is the ONE place the core
//! shells out to `git` — to version the tool's OWN state store, so every distill /
//! curate lands as an independent, revertable commit. (Reading the *user's* repos
//! lives in `omniproj-capture`; this only ever touches `~/.omniproj`.)

use std::fmt;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::omniproj_home;

/// Current on-disk schema version for `~/.omniproj`. The v1 baseline == the layout that
/// existed before versioning was introduced (spec §4.1), so a pre-versioning store is
/// adopted as v1 without any conversion. Bump this **and** add a stepwise migration
/// (see `migrate`) whenever the on-disk layout changes.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// File under `~/.omniproj` recording the on-disk schema version (plain integer + newline).
pub const SCHEMA_VERSION_FILE: &str = "SCHEMA_VERSION";

/// Failures from the checked store APIs.
#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    AuditCommit(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::AuditCommit(error) => write!(f, "audit commit failed: {error}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::AuditCommit(_) => None,
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Tool-authored commits use a `omniproj` identity (honest provenance: distinguishes
/// auto-distills from the user's own commits, and never fails for lack of a global
/// git identity).
fn commit(home: &Path, message: &str) {
    git(
        home,
        &[
            "-c",
            "user.name=omniproj",
            "-c",
            "user.email=omniproj@local",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

/// Ensure `~/.omniproj` exists and is a git repo. Idempotent; safe to call on every run.
///
/// Also enforces the on-disk schema contract (spec §4.1, W2-2):
/// - **fresh store** → records `SCHEMA_VERSION = CURRENT_SCHEMA_VERSION`;
/// - **existing store, no version file** (pre-versioning) → adopted as v1, non-destructively;
/// - **on-disk < CURRENT** → stepwise migration then version bump;
/// - **on-disk > CURRENT** → refuses to run (never silently downgrades a newer store);
/// - **malformed version file** → hard error (never silently overwritten).
pub fn ensure_home() -> std::io::Result<PathBuf> {
    let home = omniproj_home();
    std::fs::create_dir_all(&home)?;
    let fresh = !home.join(".git").exists();
    if fresh {
        git(&home, &["init", "-q"]);
        let gitignore = home.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(
                &gitignore,
                "# derived, regenerable — not versioned (spec §4.1/§4.6)\nprojects/*/cache/\n",
            )?;
        }
        // Stamp the schema version so it lands in the very first commit.
        std::fs::write(
            home.join(SCHEMA_VERSION_FILE),
            format!("{CURRENT_SCHEMA_VERSION}\n"),
        )?;
        git(&home, &["add", "-A"]);
        commit(&home, "init omniproj store");
    } else {
        ensure_schema_version(&home)?;
    }
    Ok(home)
}

/// Reconcile the on-disk `SCHEMA_VERSION` of an existing store with `CURRENT_SCHEMA_VERSION`.
/// See `ensure_home` for the full decision table.
fn ensure_schema_version(home: &Path) -> std::io::Result<()> {
    let vpath = home.join(SCHEMA_VERSION_FILE);
    // Missing file == a pre-versioning store; its layout IS v1, so adopt it as such
    // (non-destructive — do NOT migrate a store that is already at the current layout).
    let on_disk = if vpath.exists() {
        read_schema_version(&vpath)?
    } else {
        1
    };

    if on_disk > CURRENT_SCHEMA_VERSION {
        return Err(std::io::Error::other(format!(
            "{} was written by a newer OmniProj (schema v{on_disk}); this binary only \
                 understands v{CURRENT_SCHEMA_VERSION}. Refusing to touch it to avoid \
                 corruption — upgrade the binary (cargo install --git \
                 https://github.com/SuooL/OmniProj, or grab a newer release) and retry.",
            home.join(SCHEMA_VERSION_FILE).display()
        )));
    }

    if on_disk < CURRENT_SCHEMA_VERSION {
        migrate(on_disk, CURRENT_SCHEMA_VERSION, home)?;
    }

    // Record the (possibly newly-adopted or bumped) version. Skip only when the file
    // already states the current version.
    if !vpath.exists() || on_disk != CURRENT_SCHEMA_VERSION {
        std::fs::write(&vpath, format!("{CURRENT_SCHEMA_VERSION}\n"))?;
        // Commit the version stamp on its own so a bad migration is revertable
        // (targeted `add` — never smears unrelated uncommitted store changes).
        git(home, &["add", SCHEMA_VERSION_FILE]);
        let msg = if on_disk == CURRENT_SCHEMA_VERSION {
            format!("schema: adopt existing store as v{CURRENT_SCHEMA_VERSION}")
        } else {
            format!("schema: migrate store v{on_disk} -> v{CURRENT_SCHEMA_VERSION}")
        };
        commit(home, &msg);
    }
    Ok(())
}

/// Parse a `SCHEMA_VERSION` file into a `u32`. A malformed value is a hard error
/// (never silently overwritten — that could mask a corrupted or newer store).
fn read_schema_version(path: &Path) -> std::io::Result<u32> {
    let raw = std::fs::read_to_string(path)?;
    raw.trim().parse::<u32>().map_err(|_| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "{} is malformed ({:?}); refusing to run. Restore it to a plain integer, \
                 or remove ~/.omniproj to reinitialize (this discards local state).",
                path.display(),
                raw.trim()
            ),
        )
    })
}

/// Apply stepwise, versioned migrations from `from` to `to` (`from < to`).
///
/// Each step upgrades the store by exactly one version and should ideally land as its
/// own store git commit so a bad migration is revertable (`CURRENT_SCHEMA_VERSION`
/// stamping is already a separate commit; see `ensure_schema_version`).
///
/// `CURRENT_SCHEMA_VERSION == 1` today, so this range is always empty and the loop is
/// a documented no-op placeholder. Add real steps in `apply_migration_step`.
fn migrate(from: u32, to: u32, home: &Path) -> std::io::Result<()> {
    for v in from..to {
        apply_migration_step(v, home)?;
    }
    Ok(())
}

/// Migrate the store from schema `v` to `v + 1`. Add one branch per schema bump, in
/// ascending order, e.g.:
///
/// ```ignore
/// if v == 1 { return migrate_v1_to_v2(home); } // when SCHEMA v2 ships
/// ```
///
/// Reaching the fallthrough means a caller requested an undefined jump — a bug, since
/// callers only migrate when `v < CURRENT_SCHEMA_VERSION` and every gap below CURRENT
/// must have a defined step.
fn apply_migration_step(v: u32, _home: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other(format!(
        "no migration defined for schema v{v} -> v{}",
        v + 1
    )))
}

/// Diff a path under `~/.omniproj` against the last commit (`git diff HEAD -- <rel>`).
/// Returns `None` when there's no repo or no uncommitted change. Used to capture a
/// user's in-place edit to `auto/briefing.md` as a correction signal (spec §5.3).
pub fn worktree_diff(relpath: &str) -> Option<String> {
    let home = omniproj_home();
    if !home.join(".git").exists() {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(&home)
        .args(["diff", "HEAD", "--", relpath])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Run `f` while holding the exclusive store lock, or return the lock/opening failure.
///
/// Unlike [`store_txn`], this API never runs its closure without an acquired lock.
pub fn with_store_txn<T>(f: impl FnOnce() -> Result<T, StoreError>) -> Result<T, StoreError> {
    use fs2::FileExt;

    let home = omniproj_home();
    std::fs::create_dir_all(&home)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("store.lock"))?;
    lock.try_lock_exclusive()?;
    f()
}

/// Replace a file durably without exposing a partially written destination.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "atomic write target has no UTF-8 filename: {}",
                    path.display()
                ),
            )
        })?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{filename}.{}.{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ));

    let result = (|| -> Result<(), StoreError> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();

    if result.is_err() {
        std::fs::remove_file(&temporary).ok();
    }
    result
}

fn audit_error(output: std::process::Output) -> StoreError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    StoreError::AuditCommit(if stderr.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        stderr
    })
}

fn validate_relative_audit_path(path: &Path) -> Result<(), StoreError> {
    let mut has_normal_component = false;
    let safe = !path.is_absolute()
        && path.components().all(|component| match component {
            std::path::Component::Normal(_) => {
                has_normal_component = true;
                true
            }
            std::path::Component::CurDir => true,
            _ => false,
        })
        && has_normal_component;
    if safe {
        Ok(())
    } else {
        Err(StoreError::AuditCommit(format!(
            "audit path must be relative and stay below the store root: {}",
            path.display()
        )))
    }
}

/// Stage and commit only the given store-relative paths.
///
/// Returns `Ok(false)` when none of those paths differs from `HEAD` after staging.
pub fn commit_paths_checked(message: &str, relative_paths: &[PathBuf]) -> Result<bool, StoreError> {
    if relative_paths.is_empty() {
        return Ok(false);
    }
    for path in relative_paths {
        validate_relative_audit_path(path)?;
    }

    let home = omniproj_home();
    let add = Command::new("git")
        .arg("-C")
        .arg(&home)
        .arg("add")
        .arg("--")
        .args(relative_paths)
        .output()
        .map_err(StoreError::Io)?;
    if !add.status.success() {
        return Err(audit_error(add));
    }

    let diff = Command::new("git")
        .arg("-C")
        .arg(&home)
        .args(["diff", "--cached", "--quiet", "--"])
        .args(relative_paths)
        .output()
        .map_err(StoreError::Io)?;
    match diff.status.code() {
        Some(0) => return Ok(false),
        Some(1) => {}
        _ => return Err(audit_error(diff)),
    }

    let commit = Command::new("git")
        .arg("-C")
        .arg(&home)
        .args([
            "-c",
            "user.name=omniproj",
            "-c",
            "user.email=omniproj@local",
            "commit",
            "-q",
            "--only",
            "-m",
            message,
            "--",
        ])
        .args(relative_paths)
        .output()
        .map_err(StoreError::Io)?;
    if !commit.status.success() {
        return Err(audit_error(commit));
    }
    Ok(true)
}

/// Run `f` while holding the exclusive store lock (`~/.omniproj/store.lock`).
///
/// Provenance contract (spec §5): every distill/curate/learn is an INDEPENDENT,
/// revertable commit. `commit_all` stages everything, so two processes (e.g. the
/// daemon and a CLI `briefing`) interleaving write→commit would smear both updates
/// into one commit. All write-then-commit sequences must go through here.
///
/// Blocking flock: contention is rare (two simultaneous distill completions) and
/// the critical section is small (file writes + one git commit). The lock releases
/// on drop, crash included.
#[deprecated(note = "use with_store_txn so lock failures are returned to the caller")]
pub fn store_txn<T>(f: impl FnOnce() -> T) -> T {
    use fs2::FileExt;
    let _guard = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(omniproj_home().join("store.lock"))
        .and_then(|file| {
            file.lock_exclusive()?;
            Ok(file)
        })
        .ok(); // best-effort: if the lock can't be taken, still run (matches commit_all's spirit)
    f()
}

/// Stage everything under `~/.omniproj` and commit — but only if something changed.
/// Best-effort: a missing git repo or git error is silently ignored.
#[deprecated(note = "use commit_paths_checked to avoid staging unrelated store changes")]
pub fn commit_all(message: &str) {
    let home = omniproj_home();
    if !home.join(".git").exists() {
        return;
    }
    git(&home, &["add", "-A"]);
    // `diff --cached --quiet` exits 0 (success) when nothing is staged.
    let nothing_staged = git(&home, &["diff", "--cached", "--quiet"]);
    if !nothing_staged {
        commit(&home, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique tempdir per call (no tempfile dep; core tests run as parallel threads of
    /// one process, so the pid alone isn't unique). Removed at the end of each test.
    fn unique_home(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "omniproj-store-test-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ))
    }

    fn read_version(home: &Path) -> String {
        std::fs::read_to_string(home.join(SCHEMA_VERSION_FILE))
            .unwrap()
            .trim()
            .to_string()
    }

    fn git_output(home: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn atomic_write_creates_parents_and_replaces_contents_without_temp_files() {
        let home = unique_home("atomic-write");
        let path = home.join("projects/project-2026/notes/project.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "old contents").unwrap();

        atomic_write(&path, b"replacement contents").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "replacement contents"
        );
        let entries = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(entries, 1, "only the destination file should remain");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn atomic_write_propagates_an_unwritable_target_error() {
        let home = unique_home("atomic-error");
        let blocked_parent = home.join("blocked");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(&blocked_parent, "not a directory").unwrap();

        let result = atomic_write(&blocked_parent.join("state.md"), b"never written");

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(&blocked_parent).unwrap(),
            "not a directory"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn atomic_write_removes_its_temporary_file_when_replacement_fails() {
        let home = unique_home("atomic-cleanup");
        std::fs::create_dir_all(&home).unwrap();
        let destination = home.join("existing-directory");
        std::fs::create_dir(&destination).unwrap();

        let result = atomic_write(&destination, b"never replaces a directory");

        assert!(result.is_err());
        let entries: Vec<_> = std::fs::read_dir(&home)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec!["existing-directory"]);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn with_store_txn_returns_an_error_when_another_holder_has_the_lock() {
        use fs2::FileExt;

        let _g = crate::env_guard();
        let home = unique_home("lock-error");
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::fs::create_dir_all(&home).unwrap();
        let held_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(home.join("store.lock"))
            .unwrap();
        held_lock.lock_exclusive().unwrap();

        let result = with_store_txn(|| Ok::<_, StoreError>(()));

        assert!(result.is_err());
        drop(held_lock);
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn checked_commit_returns_false_when_its_audit_paths_are_unchanged() {
        let _g = crate::env_guard();
        let home = unique_home("checked-commit-clean");
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();

        let changed =
            commit_paths_checked("nothing to audit", &[PathBuf::from("SCHEMA_VERSION")]).unwrap();

        assert!(!changed);
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn checked_commit_returns_git_stderr_as_an_audit_commit_error() {
        let _g = crate::env_guard();
        let home = unique_home("checked-commit-error");
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::fs::create_dir_all(&home).unwrap();

        let error =
            commit_paths_checked("cannot commit", &[PathBuf::from("state.md")]).unwrap_err();

        match error {
            StoreError::AuditCommit(stderr) => assert!(stderr.contains("not a git repository")),
            StoreError::Io(error) => panic!("expected git stderr, got I/O error: {error}"),
        }
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn checked_commit_rejects_root_equivalent_paths_without_staging_human_edits() {
        let _g = crate::env_guard();
        let home = unique_home("checked-commit-root");
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();
        let human = home.join("projects/project-2026/Human.md");

        atomic_write(&human, b"original human text").unwrap();
        git_output(&home, &["add", "--", "projects/project-2026/Human.md"]);
        git_output(
            &home,
            &[
                "-c",
                "user.name=omniproj-test",
                "-c",
                "user.email=omniproj-test@local",
                "commit",
                "-q",
                "-m",
                "seed human document",
            ],
        );
        atomic_write(&human, b"uncommitted Human edit").unwrap();

        for root_equivalent in [PathBuf::from("."), PathBuf::new()] {
            let error = commit_paths_checked("must not stage all", &[root_equivalent]).unwrap_err();
            match error {
                StoreError::AuditCommit(message) => assert!(message.contains("audit path")),
                StoreError::Io(error) => {
                    panic!("expected validation error, got I/O error: {error}")
                }
            }
            assert_eq!(
                git_output(
                    &home,
                    &["status", "--short", "--", "projects/project-2026/Human.md"]
                ),
                " M projects/project-2026/Human.md\n"
            );
            assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
        }

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn checked_commit_stages_only_its_tracked_write() {
        let _g = crate::env_guard();
        let home = unique_home("checked-commit-targeted");
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();
        let human = home.join("projects/project-2026/Human.md");
        let audited_relative = PathBuf::from("projects/project-2026/notes/project.md");
        let audited = home.join(&audited_relative);

        atomic_write(&human, b"original human text").unwrap();
        git_output(&home, &["add", "--", "projects/project-2026/Human.md"]);
        git_output(
            &home,
            &[
                "-c",
                "user.name=omniproj-test",
                "-c",
                "user.email=omniproj-test@local",
                "commit",
                "-q",
                "-m",
                "seed human document",
            ],
        );
        atomic_write(&audited, b"initial tracked state").unwrap();
        git_output(
            &home,
            &["add", "--", "projects/project-2026/notes/project.md"],
        );
        git_output(
            &home,
            &[
                "-c",
                "user.name=omniproj-test",
                "-c",
                "user.email=omniproj-test@local",
                "commit",
                "-q",
                "-m",
                "seed audited state",
            ],
        );
        atomic_write(&human, b"user changed Human text").unwrap();
        atomic_write(&audited, b"updated audited state").unwrap();

        assert!(commit_paths_checked("update audited state", &[audited_relative]).unwrap());

        assert_eq!(
            git_output(
                &home,
                &["status", "--short", "--", "projects/project-2026/Human.md"]
            ),
            " M projects/project-2026/Human.md\n"
        );
        let committed_paths = git_output(&home, &["show", "--format=", "--name-only", "HEAD"]);
        assert_eq!(committed_paths, "projects/project-2026/notes/project.md\n");
        assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// A fresh store records the current schema version in its first commit.
    #[test]
    fn fresh_store_writes_current_version() {
        let _g = crate::env_guard();
        let home = unique_home("fresh");
        std::env::set_var("OMNIPROJ_HOME", &home);
        let got = ensure_home().unwrap();
        assert_eq!(got, home);
        assert_eq!(read_version(&home), CURRENT_SCHEMA_VERSION.to_string());
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A pre-versioning store (has `.git`, no version file) is adopted as v1 WITHOUT
    /// touching existing state.
    #[test]
    fn existing_store_missing_version_adopts_v1() {
        let _g = crate::env_guard();
        let home = unique_home("adopt");
        std::env::set_var("OMNIPROJ_HOME", &home);
        // Simulate an existing store: `.git` present, a user-authored sentinel, no
        // version file. (Fake `.git` dir suffices — `ensure_home` only checks it exists;
        // the best-effort commit no-ops without a real repo.)
        std::fs::create_dir_all(home.join(".git")).unwrap();
        let sentinel = home.join("projects/abc/auto/briefing.md");
        std::fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
        std::fs::write(&sentinel, "original briefing").unwrap();

        ensure_home().unwrap();

        assert_eq!(read_version(&home), "1");
        // Existing state must survive untouched.
        assert_eq!(
            std::fs::read_to_string(&sentinel).unwrap(),
            "original briefing"
        );

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A store written by a newer OmniProj (schema > CURRENT) must refuse to run.
    #[test]
    fn newer_store_is_refused() {
        let _g = crate::env_guard();
        let home = unique_home("newer");
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::fs::create_dir_all(home.join(".git")).unwrap();
        std::fs::write(
            home.join(SCHEMA_VERSION_FILE),
            format!("{}\n", CURRENT_SCHEMA_VERSION + 1),
        )
        .unwrap();

        let err = ensure_home().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("newer OmniProj"), "unexpected message: {msg}");
        assert!(
            msg.contains("upgrade the binary"),
            "unexpected message: {msg}"
        );

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A malformed version file is a hard error, never silently overwritten.
    #[test]
    fn malformed_version_is_refused() {
        let _g = crate::env_guard();
        let home = unique_home("malformed");
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::fs::create_dir_all(home.join(".git")).unwrap();
        std::fs::write(home.join(SCHEMA_VERSION_FILE), "not-a-number\n").unwrap();

        let err = ensure_home().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("malformed"));
        // The bad file is left as-is (not clobbered).
        assert_eq!(read_version(&home), "not-a-number");

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&home).ok();
    }
}
