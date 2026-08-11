//! `~/.omniproj` self-versioning (spec §5 provenance). This is the ONE place the core
//! shells out to `git` — to version the tool's OWN state store, so every distill /
//! curate lands as an independent, revertable commit. (Reading the *user's* repos
//! lives in `omniproj-capture`; this only ever touches `~/.omniproj`.)

use std::fmt;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ids::{ProjectId, ProjectSourceId};
use crate::paths::omniproj_home;
use crate::project::{
    parse_project_record, render_project_record, Cadence, CaptureCursor, ProjectRecord,
    ProjectSource, ProjectSourceKind, ProjectSourceStatus,
};
use crate::project_state::ProjectStateDoc;

#[cfg(test)]
type FreshInitTestPause = (
    std::sync::Arc<std::sync::Barrier>,
    std::sync::Arc<std::sync::Barrier>,
);
#[cfg(test)]
static FRESH_INIT_TEST_PAUSE: std::sync::OnceLock<std::sync::Mutex<Option<FreshInitTestPause>>> =
    std::sync::OnceLock::new();

/// Current on-disk schema version for `~/.omniproj`. A store without a version stamp is
/// interpreted as the v1 baseline and passed through the explicit v1→v2 migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// File under `~/.omniproj` recording the on-disk schema version (plain integer + newline).
pub const SCHEMA_VERSION_FILE: &str = "SCHEMA_VERSION";
const INITIAL_GITIGNORE: &str =
    "# derived, regenerable — not versioned (spec §4.1/§4.6)\nprojects/*/cache/\n/.migration-v2\n";

/// Failures from the checked store APIs.
#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    AuditCommit(String),
    AuditConflict { path: PathBuf },
    InvalidData(String),
    MigrationConflict { path: PathBuf },
    InjectedFailure(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::AuditCommit(error) => write!(f, "audit commit failed: {error}"),
            Self::AuditConflict { path } => write!(
                f,
                "audit conflict at {}: target bytes changed after the mutation",
                path.display()
            ),
            Self::InvalidData(error) => f.write_str(error),
            Self::MigrationConflict { path } => write!(
                f,
                "migration conflict at {}: refusing to overwrite existing project state",
                path.display()
            ),
            Self::InjectedFailure(name) => write!(f, "injected failure at {name}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::AuditCommit(_)
            | Self::AuditConflict { .. }
            | Self::InvalidData(_)
            | Self::MigrationConflict { .. }
            | Self::InjectedFailure(_) => None,
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl StoreError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Io(error) => error.kind(),
            Self::InvalidData(_) | Self::AuditConflict { .. } => ErrorKind::InvalidData,
            Self::MigrationConflict { .. } => ErrorKind::AlreadyExists,
            Self::AuditCommit(_) | Self::InjectedFailure(_) => ErrorKind::Other,
        }
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
/// - **existing store, no version file** (pre-versioning) → interpreted as v1 and migrated;
/// - **on-disk < CURRENT** → stepwise migration then version bump;
/// - **on-disk > CURRENT** → refuses to run (never silently downgrades a newer store);
/// - **malformed version file** → hard error (never silently overwritten).
pub fn ensure_home() -> Result<PathBuf, StoreError> {
    let home = omniproj_home();
    std::fs::create_dir_all(&home)?;
    with_store_txn(|| ensure_home_locked(&home))?;
    Ok(home)
}

fn ensure_home_locked(home: &Path) -> Result<(), StoreError> {
    let fresh = !home.join(".git").exists();
    if fresh {
        let create_gitignore = validate_fresh_init_inputs(home)?;
        if !git(home, &["init", "-q"]) {
            return Err(StoreError::AuditCommit(format!(
                "could not initialize store Git repository at {}",
                home.display()
            )));
        }
        fresh_init_test_pause_after_git_init();
        let gitignore = home.join(".gitignore");
        let mut initial_audit_targets = Vec::new();
        if create_gitignore {
            let target = audit_target_snapshot(
                home,
                PathBuf::from(".gitignore"),
                INITIAL_GITIGNORE.as_bytes(),
            )?;
            if target.prior != AuditPathIdentity::Missing {
                return Err(StoreError::AuditConflict { path: gitignore });
            }
            initial_audit_targets.push(target);
        }
        let schema_contents = format!("{CURRENT_SCHEMA_VERSION}\n");
        let schema_path = home.join(SCHEMA_VERSION_FILE);
        let schema_target = audit_target_snapshot(
            home,
            PathBuf::from(SCHEMA_VERSION_FILE),
            schema_contents.as_bytes(),
        )?;
        if schema_target.prior != AuditPathIdentity::Missing {
            return Err(StoreError::MigrationConflict { path: schema_path });
        }
        initial_audit_targets.push(schema_target);
        begin_pending_audit(home, "init omniproj store", &initial_audit_targets)?;
        if create_gitignore {
            atomic_write_store(home, &gitignore, INITIAL_GITIGNORE.as_bytes())?;
            failpoint("fresh_init_after_gitignore_write")?;
        }
        // Stamp the schema version so it lands in the very first commit.
        atomic_write_store(home, &schema_path, schema_contents.as_bytes())?;
        failpoint("fresh_init_after_schema_write_before_applied")?;
        mark_pending_audit_applied(home)?;
        finish_pending_audit(home)?;
    } else {
        recover_interrupted_fresh_init(home)?;
        ensure_schema_version(home)?;
        recover_pending_audit(home)?;
    }
    cleanup_empty_staging_dirs(home)
}

fn validate_fresh_init_inputs(home: &Path) -> Result<bool, StoreError> {
    let schema_path = home.join(SCHEMA_VERSION_FILE);
    match std::fs::symlink_metadata(&schema_path) {
        Ok(_) => return Err(StoreError::MigrationConflict { path: schema_path }),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(StoreError::Io(error)),
    }

    let gitignore_path = home.join(".gitignore");
    match std::fs::symlink_metadata(&gitignore_path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(false),
        Ok(_) => Err(StoreError::AuditConflict {
            path: gitignore_path,
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(StoreError::Io(error)),
    }
}

#[cfg(test)]
fn fresh_init_test_pause_after_git_init() {
    let pause = FRESH_INIT_TEST_PAUSE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    if let Some((reached, release)) = pause {
        reached.wait();
        release.wait();
    }
}

#[cfg(not(test))]
fn fresh_init_test_pause_after_git_init() {}

#[cfg(test)]
fn install_fresh_init_test_pause(
    reached: std::sync::Arc<std::sync::Barrier>,
    release: std::sync::Arc<std::sync::Barrier>,
) {
    *FRESH_INIT_TEST_PAUSE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some((reached, release));
}

fn cleanup_empty_staging_dirs(home: &Path) -> Result<(), StoreError> {
    let projects = home.join("projects");
    let entries = match std::fs::read_dir(&projects) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(StoreError::Io(error)),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(id) = name.strip_prefix(".staging-") else {
            continue;
        };
        if ProjectId::parse(id).is_err() || staging_tree_contains_file(&entry.path())? {
            continue;
        }
        std::fs::remove_dir_all(entry.path())?;
    }
    Ok(())
}

fn staging_tree_contains_file(path: &Path) -> Result<bool, StoreError> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || staging_tree_contains_file(&entry.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reconcile the on-disk `SCHEMA_VERSION` of an existing store with `CURRENT_SCHEMA_VERSION`.
/// See `ensure_home` for the full decision table.
fn ensure_schema_version(home: &Path) -> Result<(), StoreError> {
    let vpath = home.join(SCHEMA_VERSION_FILE);
    // Missing file == a pre-versioning store whose layout is the v1 baseline.
    let on_disk = if vpath.exists() {
        read_schema_version(&vpath)?
    } else {
        1
    };

    if on_disk > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::InvalidData(format!(
            "{} was written by a newer OmniProj (schema v{on_disk}); this binary only \
                 understands v{CURRENT_SCHEMA_VERSION}. Refusing to touch it to avoid \
                 corruption — upgrade the binary (cargo install --git \
                 https://github.com/SuooL/OmniProj, or grab a newer release) and retry.",
            home.join(SCHEMA_VERSION_FILE).display()
        )));
    }

    if on_disk < CURRENT_SCHEMA_VERSION || home.join(MIGRATION_V2_JOURNAL).exists() {
        return migrate(on_disk, CURRENT_SCHEMA_VERSION, home);
    }

    Ok(())
}

/// Parse a `SCHEMA_VERSION` file into a `u32`. A malformed value is a hard error
/// (never silently overwritten — that could mask a corrupted or newer store).
fn read_schema_version(path: &Path) -> Result<u32, StoreError> {
    let raw = std::fs::read_to_string(path)?;
    raw.trim().parse::<u32>().map_err(|_| {
        StoreError::InvalidData(format!(
            "{} is malformed ({:?}); refusing to run. Restore it to a plain integer, \
             or remove ~/.omniproj to reinitialize (this discards local state).",
            path.display(),
            raw.trim()
        ))
    })
}

/// Apply stepwise, versioned migrations from `from` to `to` (`from < to`).
///
/// Each step upgrades the store by exactly one version and should ideally land as its
/// own store git commit so a bad migration is revertable (`CURRENT_SCHEMA_VERSION`
/// stamping is already a separate commit; see `ensure_schema_version`).
///
fn migrate(from: u32, to: u32, home: &Path) -> Result<(), StoreError> {
    if from == CURRENT_SCHEMA_VERSION && home.join(MIGRATION_V2_JOURNAL).exists() {
        return migrate_v1_to_v2(home);
    }
    for v in from..to {
        apply_migration_step(v, home)?;
    }
    Ok(())
}

/// Reaching the fallthrough means a caller requested an undefined jump — a bug, since
/// callers only migrate when `v < CURRENT_SCHEMA_VERSION` and every gap below CURRENT
/// must have a defined step.
fn apply_migration_step(v: u32, home: &Path) -> Result<(), StoreError> {
    match v {
        1 => migrate_v1_to_v2(home),
        _ => Err(StoreError::InvalidData(format!(
            "no migration defined for schema v{v} -> v{}",
            v + 1
        ))),
    }
}

const MIGRATION_V2_JOURNAL: &str = ".migration-v2";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MigrationV2Phase {
    JournalCreated,
    IgnoreWritePrepared,
    IgnoreWritten,
    IgnoreAudited,
    ProjectsWritePrepared,
    ProjectsWritten,
    ProjectsAudited,
    SchemaWritePrepared,
    SchemaWritten,
    // Round-1 names, accepted only by the strict compatibility decoder and normalized.
    SchemaStampPending,
    SchemaStamped,
    SchemaAudited,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationV2Journal {
    target_schema_version: u32,
    phase: MigrationV2Phase,
    project_ids: Vec<ProjectId>,
    created_state_ids: Vec<ProjectId>,
    audit_targets: Vec<AuditTargetSnapshot>,
    pending_ignore_contents: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Round2MigrationV2Journal {
    target_schema_version: u32,
    phase: MigrationV2Phase,
    project_ids: Vec<ProjectId>,
    created_state_ids: Vec<ProjectId>,
    audit_targets: Vec<Round2AuditTargetSnapshot>,
    pending_ignore_contents: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Round1MigrationV2Journal {
    target_schema_version: u32,
    phase: MigrationV2Phase,
    project_ids: Vec<ProjectId>,
    created_state_ids: Vec<ProjectId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMigrationV2Journal {
    target_schema_version: u32,
    project_ids: Vec<ProjectId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProjectMetaV1 {
    path: String,
    name: String,
    hash: String,
    added_at: String,
    last_distilled: Option<String>,
    last_head: Option<String>,
    last_status_digest: Option<String>,
    last_session_mtime: Option<f64>,
    cadence: Option<Cadence>,
}

fn decode_migration_journal(
    home: &Path,
    on_disk: u32,
    path: &Path,
    text: &str,
) -> Result<MigrationV2Journal, StoreError> {
    if let Ok(mut journal) = toml::from_str::<MigrationV2Journal>(text) {
        validate_migration_journal_shape(path, &journal)?;
        normalize_legacy_schema_phase(home, &mut journal)?;
        validate_migration_journal(home, path, &journal)?;
        return Ok(journal);
    }
    if let Ok(round2) = toml::from_str::<Round2MigrationV2Journal>(text) {
        let mut journal = MigrationV2Journal {
            target_schema_version: round2.target_schema_version,
            phase: round2.phase,
            project_ids: round2.project_ids,
            created_state_ids: round2.created_state_ids,
            audit_targets: round2
                .audit_targets
                .into_iter()
                .map(|target| upgrade_round2_audit_target(path, target))
                .collect::<Result<_, _>>()?,
            pending_ignore_contents: round2.pending_ignore_contents,
        };
        validate_migration_journal_shape(path, &journal)?;
        normalize_legacy_schema_phase(home, &mut journal)?;
        validate_migration_journal(home, path, &journal)?;
        return Ok(journal);
    }
    if let Ok(round1) = toml::from_str::<Round1MigrationV2Journal>(text) {
        if matches!(
            round1.phase,
            MigrationV2Phase::IgnoreWritePrepared
                | MigrationV2Phase::IgnoreWritten
                | MigrationV2Phase::ProjectsWritePrepared
                | MigrationV2Phase::SchemaWritePrepared
                | MigrationV2Phase::SchemaWritten
        ) {
            return Err(StoreError::InvalidData(format!(
                "{} contains a phase that did not exist in that journal format",
                path.display()
            )));
        }
        return upgrade_round1_migration_journal(home, on_disk, path, round1);
    }
    if let Ok(legacy) = toml::from_str::<LegacyMigrationV2Journal>(text) {
        return upgrade_legacy_migration_journal(home, on_disk, path, legacy);
    }
    Err(StoreError::InvalidData(format!(
        "{} is malformed or does not match a supported migration journal format",
        path.display()
    )))
}

fn validate_authoritative_migration_snapshots(
    home: &Path,
    journal_path: &Path,
    journal: &MigrationV2Journal,
) -> Result<(), StoreError> {
    match journal.phase {
        MigrationV2Phase::IgnoreWritePrepared | MigrationV2Phase::IgnoreWritten => {
            let expected = journal.pending_ignore_contents.as_ref().ok_or_else(|| {
                StoreError::InvalidData(format!(
                    "{} omits authoritative .gitignore write bytes",
                    journal_path.display()
                ))
            })?;
            if journal.audit_targets[0].expected != regular_identity(expected.as_bytes()) {
                return Err(StoreError::AuditConflict {
                    path: journal_path.to_owned(),
                });
            }
        }
        MigrationV2Phase::ProjectsWritePrepared | MigrationV2Phase::ProjectsWritten => {
            let authoritative = legacy_project_audit_targets_from_history(home, journal)?;
            for target in &journal.audit_targets {
                let Some(expected) = authoritative
                    .iter()
                    .find(|candidate| candidate.relative_path == target.relative_path)
                else {
                    return Err(StoreError::AuditConflict {
                        path: journal_path.to_owned(),
                    });
                };
                if target.prior != expected.prior || target.expected != expected.expected {
                    return Err(StoreError::AuditConflict {
                        path: home.join(&target.relative_path),
                    });
                }
            }
            if authoritative.len() != journal.audit_targets.len() {
                return Err(StoreError::AuditConflict {
                    path: journal_path.to_owned(),
                });
            }
        }
        MigrationV2Phase::SchemaWritePrepared | MigrationV2Phase::SchemaWritten => {
            let authoritative = schema_audit_target_from_history(home)?;
            if journal.audit_targets[0].prior != authoritative.prior
                || journal.audit_targets[0].expected != authoritative.expected
            {
                return Err(StoreError::AuditConflict {
                    path: home.join(SCHEMA_VERSION_FILE),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_snapshot_phase_state(
    home: &Path,
    journal: &MigrationV2Journal,
) -> Result<(), StoreError> {
    match journal.phase {
        MigrationV2Phase::IgnoreWritePrepared | MigrationV2Phase::ProjectsWritePrepared => {
            validate_targets_are_prior_or_expected(home, &journal.audit_targets)
        }
        MigrationV2Phase::SchemaWritePrepared => {
            validate_targets_are_prior_or_expected(home, &journal.audit_targets)
        }
        MigrationV2Phase::IgnoreWritten
        | MigrationV2Phase::ProjectsWritten
        | MigrationV2Phase::SchemaWritten => {
            validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)
        }
        _ => Ok(()),
    }
}

fn normalize_legacy_schema_phase(
    home: &Path,
    journal: &mut MigrationV2Journal,
) -> Result<(), StoreError> {
    match journal.phase {
        MigrationV2Phase::SchemaStampPending => {
            journal.audit_targets = vec![schema_audit_target_from_history(home)?];
            validate_targets_are_prior_or_expected(home, &journal.audit_targets)?;
            journal.phase = MigrationV2Phase::SchemaWritePrepared;
        }
        MigrationV2Phase::SchemaStamped => {
            journal.audit_targets = vec![schema_audit_target_from_history(home)?];
            validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)?;
            journal.phase = MigrationV2Phase::SchemaWritten;
        }
        _ => {}
    }
    Ok(())
}

fn upgrade_round1_migration_journal(
    home: &Path,
    _on_disk: u32,
    path: &Path,
    round1: Round1MigrationV2Journal,
) -> Result<MigrationV2Journal, StoreError> {
    let mut journal = MigrationV2Journal {
        target_schema_version: round1.target_schema_version,
        phase: MigrationV2Phase::JournalCreated,
        project_ids: round1.project_ids,
        created_state_ids: round1.created_state_ids,
        audit_targets: Vec::new(),
        pending_ignore_contents: None,
    };
    validate_migration_journal_shape(path, &journal)?;
    let original_phase = round1.phase;
    let project_targets = legacy_project_audit_targets_from_history(home, &journal)?;

    match original_phase {
        MigrationV2Phase::JournalCreated => {
            validate_audit_targets(home, &project_targets, SnapshotSide::Prior)?;
        }
        MigrationV2Phase::IgnoreAudited => {
            validate_round1_ignore_audited(home)?;
            validate_targets_are_prior_or_expected(home, &project_targets)?;
            journal.audit_targets = project_targets;
            journal.phase = MigrationV2Phase::ProjectsWritePrepared;
        }
        MigrationV2Phase::ProjectsWritten => {
            validate_audit_targets(home, &project_targets, SnapshotSide::Expected)?;
            journal.audit_targets = project_targets;
            journal.phase = MigrationV2Phase::ProjectsWritten;
        }
        MigrationV2Phase::ProjectsAudited
        | MigrationV2Phase::SchemaStampPending
        | MigrationV2Phase::SchemaStamped
        | MigrationV2Phase::SchemaAudited => {
            validate_outputs_match_head(home, &project_targets)?;
            journal.phase = MigrationV2Phase::ProjectsAudited;
            if matches!(
                original_phase,
                MigrationV2Phase::SchemaStampPending | MigrationV2Phase::SchemaStamped
            ) {
                journal.audit_targets = vec![schema_audit_target_from_history(home)?];
                if original_phase == MigrationV2Phase::SchemaStampPending {
                    validate_targets_are_prior_or_expected(home, &journal.audit_targets)?;
                    journal.phase = MigrationV2Phase::SchemaWritePrepared;
                } else {
                    validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)?;
                    journal.phase = MigrationV2Phase::SchemaWritten;
                }
            } else if original_phase == MigrationV2Phase::SchemaAudited {
                let schema_target = schema_audit_target_from_history(home)?;
                validate_audit_targets(
                    home,
                    std::slice::from_ref(&schema_target),
                    SnapshotSide::Expected,
                )?;
                validate_outputs_match_head(home, std::slice::from_ref(&schema_target))?;
                journal.phase = MigrationV2Phase::SchemaAudited;
            }
        }
        MigrationV2Phase::IgnoreWritePrepared
        | MigrationV2Phase::IgnoreWritten
        | MigrationV2Phase::ProjectsWritePrepared
        | MigrationV2Phase::SchemaWritePrepared
        | MigrationV2Phase::SchemaWritten => unreachable!("rejected above"),
    }
    validate_migration_journal(home, path, &journal)?;
    write_migration_journal(path, &journal)?;
    Ok(journal)
}

fn validate_migration_journal(
    home: &Path,
    path: &Path,
    journal: &MigrationV2Journal,
) -> Result<(), StoreError> {
    validate_migration_journal_shape(path, journal)?;
    validate_audit_target_paths(home, &journal.audit_targets)?;
    validate_authoritative_migration_snapshots(home, path, journal)?;
    validate_snapshot_phase_state(home, journal)?;
    validate_migration_phase_milestone(home, journal)
}

#[derive(Clone, Copy)]
enum ProjectMilestone {
    Prior,
    PriorOrExpected,
    Expected,
    ExpectedInHead,
}

fn validate_migration_phase_milestone(
    home: &Path,
    journal: &MigrationV2Journal,
) -> Result<(), StoreError> {
    match journal.phase {
        MigrationV2Phase::JournalCreated => {
            migration_ignore_contents(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::Prior, false)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::IgnoreWritePrepared | MigrationV2Phase::IgnoreWritten => {
            validate_project_milestone(home, journal, ProjectMilestone::Prior, false)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::IgnoreAudited => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::PriorOrExpected, false)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::ProjectsWritePrepared => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::PriorOrExpected, false)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::ProjectsWritten => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::Expected, false)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::ProjectsAudited => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::ExpectedInHead, true)?;
            validate_schema_milestone(home, SnapshotSide::Prior, false)
        }
        MigrationV2Phase::SchemaWritePrepared | MigrationV2Phase::SchemaWritten => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::ExpectedInHead, true)
        }
        MigrationV2Phase::SchemaAudited => {
            validate_round1_ignore_audited(home)?;
            validate_project_milestone(home, journal, ProjectMilestone::ExpectedInHead, true)?;
            validate_schema_milestone(home, SnapshotSide::Expected, true)
        }
        MigrationV2Phase::SchemaStampPending | MigrationV2Phase::SchemaStamped => {
            Err(StoreError::InvalidData(
                "legacy schema phase was not normalized before validation".into(),
            ))
        }
    }
}

fn validate_project_milestone(
    home: &Path,
    journal: &MigrationV2Journal,
    milestone: ProjectMilestone,
    exact_project_set: bool,
) -> Result<(), StoreError> {
    let actual = migration_project_ids(home)?;
    let contains_all = journal
        .project_ids
        .iter()
        .all(|project_id| actual.contains(project_id));
    if !contains_all || (exact_project_set && actual.len() != journal.project_ids.len()) {
        return Err(StoreError::MigrationConflict {
            path: home.join("projects"),
        });
    }

    let targets = legacy_project_audit_targets_from_history(home, journal)?;
    match milestone {
        ProjectMilestone::Prior => {
            validate_audit_targets(home, &targets, SnapshotSide::Prior)?;
        }
        ProjectMilestone::PriorOrExpected => {
            validate_targets_are_prior_or_expected(home, &targets)?;
        }
        ProjectMilestone::Expected => {
            validate_audit_targets(home, &targets, SnapshotSide::Expected)?;
        }
        ProjectMilestone::ExpectedInHead => validate_outputs_match_head(home, &targets)?,
    }

    let created: std::collections::HashSet<_> = journal.created_state_ids.iter().collect();
    for project_id in &journal.project_ids {
        if created.contains(project_id) {
            continue;
        }
        let (_, setup) = migration_record_and_setup(home, project_id)?;
        let state_path = home
            .join("projects")
            .join(project_id.as_str())
            .join("notes/project.md");
        validate_store_file_target(home, &state_path)?;
        let existing =
            std::fs::read_to_string(&state_path).map_err(|_| StoreError::MigrationConflict {
                path: state_path.clone(),
            })?;
        if ProjectStateDoc::parse(&existing).ok().as_ref() != Some(&setup) {
            return Err(StoreError::MigrationConflict { path: state_path });
        }
    }
    Ok(())
}

fn validate_schema_milestone(
    home: &Path,
    side: SnapshotSide,
    require_head: bool,
) -> Result<(), StoreError> {
    let target = schema_audit_target_from_history(home)?;
    validate_audit_targets(home, std::slice::from_ref(&target), side)?;
    if require_head {
        validate_outputs_match_head(home, std::slice::from_ref(&target))?;
    }
    Ok(())
}

fn validate_migration_journal_shape(
    path: &Path,
    journal: &MigrationV2Journal,
) -> Result<(), StoreError> {
    if journal.target_schema_version != 2 {
        return Err(StoreError::InvalidData(format!(
            "{} targets unsupported schema {}",
            path.display(),
            journal.target_schema_version
        )));
    }
    let project_ids: std::collections::HashSet<_> = journal.project_ids.iter().collect();
    if project_ids.len() != journal.project_ids.len() {
        return Err(StoreError::InvalidData(format!(
            "{} contains duplicate project ids",
            path.display()
        )));
    }
    let created_ids: std::collections::HashSet<_> = journal.created_state_ids.iter().collect();
    if created_ids.len() != journal.created_state_ids.len()
        || !created_ids.iter().all(|id| project_ids.contains(id))
    {
        return Err(StoreError::InvalidData(format!(
            "{} has invalid created-state tracking",
            path.display()
        )));
    }
    let mut target_paths = std::collections::HashSet::new();
    for target in &journal.audit_targets {
        validate_relative_audit_path(&target.relative_path)?;
        if !target_paths.insert(target.relative_path.clone())
            || !valid_mutation_audit_target(target)
        {
            return Err(StoreError::InvalidData(format!(
                "{} has invalid audit target snapshots",
                path.display()
            )));
        }
    }
    if matches!(
        journal.phase,
        MigrationV2Phase::JournalCreated | MigrationV2Phase::IgnoreAudited
    ) && !journal.audit_targets.is_empty()
    {
        return Err(StoreError::InvalidData(format!(
            "{} records audit targets before the write-prepared phase",
            path.display()
        )));
    }
    let ignore_phase = matches!(
        journal.phase,
        MigrationV2Phase::IgnoreWritePrepared | MigrationV2Phase::IgnoreWritten
    );
    if ignore_phase != journal.pending_ignore_contents.is_some() {
        return Err(StoreError::InvalidData(format!(
            "{} has inconsistent pending .gitignore contents",
            path.display()
        )));
    }
    let expected_paths: std::collections::HashSet<PathBuf> = if ignore_phase {
        [PathBuf::from(".gitignore")].into_iter().collect()
    } else if matches!(
        journal.phase,
        MigrationV2Phase::ProjectsWritePrepared | MigrationV2Phase::ProjectsWritten
    ) {
        journal
            .project_ids
            .iter()
            .map(|id| PathBuf::from(format!("projects/{id}/meta.toml")))
            .chain(
                journal
                    .created_state_ids
                    .iter()
                    .map(|id| PathBuf::from(format!("projects/{id}/notes/project.md"))),
            )
            .collect()
    } else if matches!(
        journal.phase,
        MigrationV2Phase::SchemaWritePrepared | MigrationV2Phase::SchemaWritten
    ) {
        [PathBuf::from(SCHEMA_VERSION_FILE)].into_iter().collect()
    } else {
        std::collections::HashSet::new()
    };
    if target_paths != expected_paths {
        return Err(StoreError::InvalidData(format!(
            "{} audit targets do not match its migration phase",
            path.display()
        )));
    }
    Ok(())
}

fn valid_audit_identity(identity: &AuditPathIdentity) -> bool {
    match identity {
        AuditPathIdentity::RegularFile { sha256 } => {
            sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        }
        AuditPathIdentity::Missing
        | AuditPathIdentity::Directory
        | AuditPathIdentity::Symlink { .. } => true,
    }
}

fn valid_mutation_audit_target(target: &AuditTargetSnapshot) -> bool {
    valid_audit_identity(&target.prior)
        && valid_audit_identity(&target.expected)
        && matches!(
            target.prior,
            AuditPathIdentity::Missing | AuditPathIdentity::RegularFile { .. }
        )
        && matches!(target.expected, AuditPathIdentity::RegularFile { .. })
}

fn upgrade_legacy_migration_journal(
    home: &Path,
    on_disk: u32,
    path: &Path,
    legacy: LegacyMigrationV2Journal,
) -> Result<MigrationV2Journal, StoreError> {
    if legacy.target_schema_version != 2 {
        return Err(StoreError::InvalidData(format!(
            "{} targets unsupported schema {}",
            path.display(),
            legacy.target_schema_version
        )));
    }
    let unique: std::collections::HashSet<_> = legacy.project_ids.iter().collect();
    if unique.len() != legacy.project_ids.len() {
        return Err(StoreError::InvalidData(format!(
            "{} contains duplicate project ids",
            path.display()
        )));
    }
    let actual = migration_project_ids(home)?;
    if legacy
        .project_ids
        .iter()
        .any(|project_id| !actual.contains(project_id))
    {
        return Err(StoreError::InvalidData(format!(
            "{} names a project that is not present",
            path.display()
        )));
    }
    if on_disk == 2 && actual != legacy.project_ids {
        return Err(StoreError::InvalidData(format!(
            "{} project set does not match the stamped store",
            path.display()
        )));
    }

    let mut created_state_ids = Vec::new();
    for project_id in &legacy.project_ids {
        validate_legacy_migration_metadata(home, project_id)?;
        let (_, setup) = migration_record_and_setup(home, project_id)?;
        let relative_state = PathBuf::from(format!("projects/{project_id}/notes/project.md"));
        let state_path = home.join(&relative_state);
        if state_path.exists() {
            let existing = std::fs::read_to_string(&state_path)?;
            if ProjectStateDoc::parse(&existing).ok().as_ref() != Some(&setup) {
                return Err(StoreError::MigrationConflict { path: state_path });
            }
            if !path_matches_head(home, &relative_state)? {
                return Err(StoreError::MigrationConflict { path: state_path });
            }
        } else if on_disk == 1 {
            created_state_ids.push(project_id.clone());
        } else {
            return Err(StoreError::MigrationConflict { path: state_path });
        }
    }

    let phase = match on_disk {
        1 => MigrationV2Phase::JournalCreated,
        2 => {
            for project_id in &legacy.project_ids {
                for relative in [
                    PathBuf::from(format!("projects/{project_id}/meta.toml")),
                    PathBuf::from(format!("projects/{project_id}/notes/project.md")),
                ] {
                    if !path_matches_head(home, &relative)? {
                        return Err(StoreError::AuditConflict {
                            path: home.join(relative),
                        });
                    }
                }
            }
            if path_matches_head(home, Path::new(SCHEMA_VERSION_FILE))? {
                MigrationV2Phase::SchemaAudited
            } else {
                MigrationV2Phase::SchemaStamped
            }
        }
        _ => {
            return Err(StoreError::InvalidData(format!(
                "{} cannot resume against schema v{on_disk}",
                path.display()
            )));
        }
    };
    let mut journal = MigrationV2Journal {
        target_schema_version: 2,
        phase,
        project_ids: legacy.project_ids,
        created_state_ids,
        audit_targets: Vec::new(),
        pending_ignore_contents: None,
    };
    if on_disk == 1 {
        let targets = legacy_project_audit_targets_from_history(home, &journal)?;
        if !audit_targets_match(home, &targets, SnapshotSide::Prior)? {
            if !audit_targets_match(home, &targets, SnapshotSide::Expected)? {
                return Err(StoreError::AuditConflict {
                    path: path.to_owned(),
                });
            }
            validate_round1_ignore_audited(home)?;
            validate_outputs_match_head(home, &targets)?;
            journal.phase = MigrationV2Phase::ProjectsAudited;
        }
    }
    normalize_legacy_schema_phase(home, &mut journal)?;
    validate_migration_journal(home, path, &journal)?;
    write_migration_journal(path, &journal)?;
    Ok(journal)
}

fn validate_legacy_migration_metadata(
    home: &Path,
    project_id: &ProjectId,
) -> Result<(), StoreError> {
    let relative = PathBuf::from(format!("projects/{project_id}/meta.toml"));
    if path_matches_head(home, &relative)? {
        return Ok(());
    }
    let current_path = home.join(&relative);
    let current = std::fs::read_to_string(&current_path)?;
    let head = git_head_bytes(home, &relative)?.ok_or_else(|| StoreError::AuditConflict {
        path: current_path.clone(),
    })?;
    let head = String::from_utf8(head).map_err(|_| StoreError::AuditConflict {
        path: current_path.clone(),
    })?;
    let legacy: LegacyProjectMetaV1 =
        toml::from_str(&head).map_err(|_| StoreError::AuditConflict {
            path: current_path.clone(),
        })?;
    let expected = render_project_record(&legacy_to_v2(project_id, legacy)?).map_err(|_| {
        StoreError::AuditConflict {
            path: current_path.clone(),
        }
    })?;
    if current != expected {
        return Err(StoreError::AuditConflict { path: current_path });
    }
    Ok(())
}

fn path_matches_head(home: &Path, relative: &Path) -> Result<bool, StoreError> {
    let Some(head) = git_head_bytes(home, relative)? else {
        return Ok(false);
    };
    validate_store_path_ancestors(home, &home.join(relative))?;
    Ok(std::fs::read(home.join(relative))? == head)
}

fn git_head_bytes(home: &Path, relative: &Path) -> Result<Option<Vec<u8>>, StoreError> {
    git_revision_bytes(home, "HEAD", relative)
}

fn git_revision_bytes(
    home: &Path,
    revision: &str,
    relative: &Path,
) -> Result<Option<Vec<u8>>, StoreError> {
    validate_relative_audit_path(relative)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(home)
        .args(["show", &format!("{revision}:{}", relative.display())])
        .output()?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        Ok(None)
    }
}

fn legacy_project_audit_targets_from_history(
    home: &Path,
    journal: &MigrationV2Journal,
) -> Result<Vec<AuditTargetSnapshot>, StoreError> {
    let created: std::collections::HashSet<_> = journal.created_state_ids.iter().collect();
    let mut targets = Vec::new();
    for project_id in &journal.project_ids {
        let relative_meta = PathBuf::from(format!("projects/{project_id}/meta.toml"));
        let output = Command::new("git")
            .arg("-C")
            .arg(home)
            .args(["log", "--format=%H", "--", &relative_meta.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            return Err(audit_error(output));
        }
        let mut baseline = None;
        for revision in String::from_utf8_lossy(&output.stdout).lines() {
            let Some(bytes) = git_revision_bytes(home, revision, &relative_meta)? else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            let Ok(legacy) = toml::from_str::<LegacyProjectMetaV1>(text) else {
                continue;
            };
            if legacy.hash != project_id.as_str() {
                continue;
            }
            baseline = Some((revision.to_owned(), bytes, legacy));
            break;
        }
        let (revision, prior_bytes, legacy) =
            baseline.ok_or_else(|| StoreError::AuditConflict {
                path: home.join(&relative_meta),
            })?;
        let record = legacy_to_v2(project_id, legacy)?;
        let expected = render_project_record(&record)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        targets.push(AuditTargetSnapshot {
            relative_path: relative_meta,
            prior: regular_identity(&prior_bytes),
            expected: regular_identity(expected.as_bytes()),
        });

        if created.contains(project_id) {
            let relative_state = PathBuf::from(format!("projects/{project_id}/notes/project.md"));
            if git_revision_bytes(home, &revision, &relative_state)?.is_some() {
                return Err(StoreError::AuditConflict {
                    path: home.join(relative_state),
                });
            }
            let expected = ProjectStateDoc::new_setup(&record.created_at)
                .and_then(|state| state.render())
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            targets.push(AuditTargetSnapshot {
                relative_path: relative_state,
                prior: AuditPathIdentity::Missing,
                expected: regular_identity(expected.as_bytes()),
            });
        }
    }
    Ok(targets)
}

fn validate_round1_ignore_audited(home: &Path) -> Result<(), StoreError> {
    let relative = Path::new(".gitignore");
    let current = std::fs::read_to_string(home.join(relative))?;
    if !current.lines().any(|line| line == "/.migration-v2") || !path_matches_head(home, relative)?
    {
        return Err(StoreError::AuditConflict {
            path: home.join(relative),
        });
    }
    Ok(())
}

fn validate_outputs_match_head(
    home: &Path,
    targets: &[AuditTargetSnapshot],
) -> Result<(), StoreError> {
    validate_audit_targets(home, targets, SnapshotSide::Expected)?;
    for target in targets {
        let head = git_head_bytes(home, &target.relative_path)?
            .map(|bytes| regular_identity(&bytes))
            .unwrap_or(AuditPathIdentity::Missing);
        if head != target.expected {
            return Err(StoreError::AuditConflict {
                path: home.join(&target.relative_path),
            });
        }
    }
    Ok(())
}

fn schema_audit_target_from_history(home: &Path) -> Result<AuditTargetSnapshot, StoreError> {
    let relative = PathBuf::from(SCHEMA_VERSION_FILE);
    let output = Command::new("git")
        .arg("-C")
        .arg(home)
        .args(["log", "--format=%H", "--", SCHEMA_VERSION_FILE])
        .output()?;
    if !output.status.success() {
        return Err(audit_error(output));
    }
    let mut prior = None;
    for revision in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(bytes) = git_revision_bytes(home, revision, &relative)? {
            if bytes != b"2\n" {
                prior = Some(regular_identity(&bytes));
                break;
            }
        }
    }
    Ok(AuditTargetSnapshot {
        relative_path: relative,
        prior: prior.unwrap_or(AuditPathIdentity::Missing),
        expected: regular_identity(b"2\n"),
    })
}

fn regular_identity(contents: &[u8]) -> AuditPathIdentity {
    AuditPathIdentity::RegularFile {
        sha256: sha256(contents),
    }
}

fn migrate_v1_to_v2(home: &Path) -> Result<(), StoreError> {
    let journal_path = home.join(MIGRATION_V2_JOURNAL);
    let on_disk = if home.join(SCHEMA_VERSION_FILE).exists() {
        read_schema_version(&home.join(SCHEMA_VERSION_FILE))?
    } else {
        1
    };
    let mut journal = if journal_path.exists() {
        let text = std::fs::read_to_string(&journal_path)?;
        let journal = decode_migration_journal(home, on_disk, &journal_path, &text)?;
        if on_disk == 2 && migration_project_ids(home)? != journal.project_ids {
            return Err(StoreError::InvalidData(format!(
                "{} project set does not match the stamped store",
                journal_path.display()
            )));
        }
        if journal.target_schema_version != 2 {
            return Err(StoreError::InvalidData(format!(
                "{} targets unsupported schema {}",
                journal_path.display(),
                journal.target_schema_version
            )));
        }
        let phase_matches_stamp = match on_disk {
            1 => !matches!(
                journal.phase,
                MigrationV2Phase::SchemaWritten | MigrationV2Phase::SchemaAudited
            ),
            2 => matches!(
                journal.phase,
                MigrationV2Phase::SchemaWritePrepared
                    | MigrationV2Phase::SchemaWritten
                    | MigrationV2Phase::SchemaAudited
            ),
            _ => false,
        };
        if !phase_matches_stamp {
            return Err(StoreError::InvalidData(format!(
                "{} phase {:?} is inconsistent with schema v{on_disk}",
                journal_path.display(),
                journal.phase
            )));
        }
        journal
    } else {
        if on_disk != 1 {
            return Err(StoreError::InvalidData(format!(
                "schema v{on_disk} cannot start the v1 -> v2 migration"
            )));
        }
        let project_ids = migration_project_ids(home)?;
        let mut created_state_ids = Vec::new();
        for project_id in &project_ids {
            let (record, setup) = migration_record_and_setup(home, project_id)?;
            let state_path = home
                .join("projects")
                .join(project_id.as_str())
                .join("notes/project.md");
            validate_store_file_target(home, &state_path)?;
            if state_path.exists() {
                let existing = std::fs::read_to_string(&state_path)?;
                if ProjectStateDoc::parse(&existing).ok().as_ref() != Some(&setup) {
                    return Err(StoreError::MigrationConflict { path: state_path });
                }
            } else {
                created_state_ids.push(project_id.clone());
            }
            validate_migration_record(project_id, &record)?;
        }
        let journal = MigrationV2Journal {
            target_schema_version: 2,
            phase: MigrationV2Phase::JournalCreated,
            project_ids,
            created_state_ids,
            audit_targets: Vec::new(),
            pending_ignore_contents: None,
        };
        write_migration_journal(&journal_path, &journal)?;
        failpoint("migration_after_journal_creation")?;
        journal
    };

    loop {
        match journal.phase {
            MigrationV2Phase::JournalCreated => {
                refresh_migration_projects(home, &mut journal)?;
                let expected = migration_ignore_contents(home)?;
                journal.audit_targets = vec![audit_target_snapshot(
                    home,
                    PathBuf::from(".gitignore"),
                    expected.as_bytes(),
                )?];
                journal.pending_ignore_contents = Some(expected);
                journal.phase = MigrationV2Phase::IgnoreWritePrepared;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::IgnoreWritePrepared => {
                validate_targets_are_prior_or_expected(home, &journal.audit_targets)?;
                let expected = journal.pending_ignore_contents.as_ref().ok_or_else(|| {
                    StoreError::InvalidData("missing prepared .gitignore contents".into())
                })?;
                if !audit_targets_match(home, &journal.audit_targets, SnapshotSide::Expected)? {
                    atomic_write_store(home, &home.join(".gitignore"), expected.as_bytes())?;
                }
                journal.phase = MigrationV2Phase::IgnoreWritten;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::IgnoreWritten => {
                validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)?;
                commit_paths_checked(
                    "schema: ignore v2 migration journal",
                    &[PathBuf::from(".gitignore")],
                )?;
                journal.audit_targets.clear();
                journal.pending_ignore_contents = None;
                journal.phase = MigrationV2Phase::IgnoreAudited;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::IgnoreAudited => {
                if refresh_migration_projects(home, &mut journal)? {
                    write_migration_journal(&journal_path, &journal)?;
                }
                journal.audit_targets = migration_audit_targets(home, &journal)?;
                journal.phase = MigrationV2Phase::ProjectsWritePrepared;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::ProjectsWritePrepared => {
                validate_targets_are_prior_or_expected(home, &journal.audit_targets)?;
                for project_id in &journal.project_ids {
                    let (record, setup) = migration_record_and_setup(home, project_id)?;
                    let project_root = home.join("projects").join(project_id.as_str());
                    let state_path = project_root.join("notes/project.md");
                    validate_store_file_target(home, &state_path)?;
                    if state_path.exists() {
                        let existing = std::fs::read_to_string(&state_path)?;
                        if ProjectStateDoc::parse(&existing).ok().as_ref() != Some(&setup) {
                            return Err(StoreError::MigrationConflict { path: state_path });
                        }
                    } else {
                        setup
                            .save_to_store_path(home, &state_path)
                            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                    }
                    failpoint("migration_after_project_state_write")?;

                    let meta_path = project_root.join("meta.toml");
                    let text = render_project_record(&record)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                    atomic_write_store(home, &meta_path, text.as_bytes())?;
                    failpoint("migration_after_metadata_write")?;
                }
                journal.phase = MigrationV2Phase::ProjectsWritten;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::ProjectsWritten => {
                validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)?;
                let audit_paths = audit_target_paths(&journal.audit_targets);
                commit_paths_checked("schema: migrate project records to v2", &audit_paths)?;
                journal.audit_targets.clear();
                journal.phase = MigrationV2Phase::ProjectsAudited;
                write_migration_journal(&journal_path, &journal)?;
                failpoint("migration_after_project_audit_commit")?;
            }
            MigrationV2Phase::ProjectsAudited => {
                if refresh_migration_projects(home, &mut journal)? {
                    journal.phase = MigrationV2Phase::IgnoreAudited;
                    journal.audit_targets.clear();
                    write_migration_journal(&journal_path, &journal)?;
                    continue;
                }
                journal.audit_targets = vec![audit_target_snapshot(
                    home,
                    PathBuf::from(SCHEMA_VERSION_FILE),
                    b"2\n",
                )?];
                journal.phase = MigrationV2Phase::SchemaWritePrepared;
                write_migration_journal(&journal_path, &journal)?;
            }
            MigrationV2Phase::SchemaWritePrepared => {
                validate_targets_are_prior_or_expected(home, &journal.audit_targets)?;
                if !audit_targets_match(home, &journal.audit_targets, SnapshotSide::Expected)? {
                    let schema_path = home.join(SCHEMA_VERSION_FILE);
                    atomic_write_store(home, &schema_path, b"2\n")?;
                }
                failpoint("migration_after_schema_stamp_write_before_phase")?;
                journal.phase = MigrationV2Phase::SchemaWritten;
                write_migration_journal(&journal_path, &journal)?;
                failpoint("migration_after_schema_stamp")?;
            }
            MigrationV2Phase::SchemaWritten => {
                validate_audit_targets(home, &journal.audit_targets, SnapshotSide::Expected)?;
                commit_paths_checked(
                    "schema: migrate store v1 -> v2",
                    &[PathBuf::from(SCHEMA_VERSION_FILE)],
                )?;
                journal.audit_targets.clear();
                journal.phase = MigrationV2Phase::SchemaAudited;
                write_migration_journal(&journal_path, &journal)?;
                failpoint("migration_after_schema_audit_commit")?;
            }
            MigrationV2Phase::SchemaStampPending | MigrationV2Phase::SchemaStamped => {
                return Err(StoreError::InvalidData(format!(
                    "{} contains an unnormalized legacy schema phase",
                    journal_path.display()
                )));
            }
            MigrationV2Phase::SchemaAudited => {
                std::fs::remove_file(journal_path)?;
                return Ok(());
            }
        }
    }
}

fn write_migration_journal(path: &Path, journal: &MigrationV2Journal) -> Result<(), StoreError> {
    let text =
        toml::to_string(journal).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let home = path.parent().ok_or_else(|| {
        StoreError::InvalidData(format!(
            "migration journal has no store parent: {}",
            path.display()
        ))
    })?;
    atomic_write_store(home, path, text.as_bytes())
}

fn migration_audit_targets(
    home: &Path,
    journal: &MigrationV2Journal,
) -> Result<Vec<AuditTargetSnapshot>, StoreError> {
    let mut targets = Vec::new();
    for project_id in &journal.project_ids {
        let (record, _) = migration_record_and_setup(home, project_id)?;
        let expected = render_project_record(&record)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        targets.push(audit_target_snapshot(
            home,
            PathBuf::from(format!("projects/{project_id}/meta.toml")),
            expected.as_bytes(),
        )?);
    }
    for project_id in &journal.created_state_ids {
        let (_, setup) = migration_record_and_setup(home, project_id)?;
        let expected = setup
            .render()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        targets.push(audit_target_snapshot(
            home,
            PathBuf::from(format!("projects/{project_id}/notes/project.md")),
            expected.as_bytes(),
        )?);
    }
    Ok(targets)
}

fn validate_targets_are_prior_or_expected(
    home: &Path,
    targets: &[AuditTargetSnapshot],
) -> Result<(), StoreError> {
    for target in targets {
        validate_store_path_ancestors(home, &home.join(&target.relative_path))?;
        let actual = audit_path_identity(&home.join(&target.relative_path))?;
        if actual != target.prior && actual != target.expected {
            return Err(StoreError::AuditConflict {
                path: home.join(&target.relative_path),
            });
        }
    }
    Ok(())
}

fn audit_target_paths(targets: &[AuditTargetSnapshot]) -> Vec<PathBuf> {
    targets
        .iter()
        .map(|target| target.relative_path.clone())
        .collect()
}

fn refresh_migration_projects(
    home: &Path,
    journal: &mut MigrationV2Journal,
) -> Result<bool, StoreError> {
    let actual = migration_project_ids(home)?;
    if journal
        .project_ids
        .iter()
        .any(|project_id| !actual.contains(project_id))
    {
        return Err(StoreError::InvalidData(
            "a project disappeared during the v1 -> v2 migration".into(),
        ));
    }
    let added: Vec<_> = actual
        .iter()
        .filter(|project_id| !journal.project_ids.contains(project_id))
        .cloned()
        .collect();
    for project_id in &added {
        let (record, setup) = migration_record_and_setup(home, project_id)?;
        let state_path = home
            .join("projects")
            .join(project_id.as_str())
            .join("notes/project.md");
        validate_store_file_target(home, &state_path)?;
        if state_path.exists() {
            let existing = std::fs::read_to_string(&state_path)?;
            if ProjectStateDoc::parse(&existing).ok().as_ref() != Some(&setup) {
                return Err(StoreError::MigrationConflict { path: state_path });
            }
        } else {
            journal.created_state_ids.push(project_id.clone());
        }
        validate_migration_record(project_id, &record)?;
        journal.project_ids.push(project_id.clone());
    }
    if !added.is_empty() {
        journal.project_ids.sort();
        journal.created_state_ids.sort();
    }
    Ok(!added.is_empty())
}

fn migration_project_ids(home: &Path) -> Result<Vec<ProjectId>, StoreError> {
    let root = home.join("projects");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(StoreError::Io(error)),
    };
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| ProjectId::parse(name).is_ok())
            {
                return Err(StoreError::MigrationConflict { path: entry.path() });
            }
            continue;
        }
        match std::fs::symlink_metadata(entry.path().join("meta.toml")) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(StoreError::Io(error)),
        }
        let name = entry.file_name().into_string().map_err(|_| {
            StoreError::InvalidData(format!(
                "project directory name is not UTF-8: {}",
                entry.path().display()
            ))
        })?;
        ids.push(ProjectId::parse(&name).map_err(|error| {
            StoreError::InvalidData(format!("invalid project directory {name:?}: {error}"))
        })?);
    }
    ids.sort();
    Ok(ids)
}

fn migration_record_and_setup(
    home: &Path,
    project_id: &ProjectId,
) -> Result<(ProjectRecord, ProjectStateDoc), StoreError> {
    let meta_path = home
        .join("projects")
        .join(project_id.as_str())
        .join("meta.toml");
    validate_store_file_target(home, &meta_path)?;
    let text = std::fs::read_to_string(&meta_path)?;
    let record = match toml::from_str::<LegacyProjectMetaV1>(&text) {
        Ok(legacy) => legacy_to_v2(project_id, legacy)?,
        Err(legacy_error) => parse_project_record(&meta_path, &text).map_err(|v2_error| {
            StoreError::InvalidData(format!(
                "{} is neither strict schema v1 ({legacy_error}) nor schema v2 ({v2_error})",
                meta_path.display()
            ))
        })?,
    };
    validate_migration_record(project_id, &record)?;
    let setup = ProjectStateDoc::new_setup(&record.created_at)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    Ok((record, setup))
}

fn legacy_to_v2(
    project_id: &ProjectId,
    legacy: LegacyProjectMetaV1,
) -> Result<ProjectRecord, StoreError> {
    if legacy.hash != project_id.as_str() {
        return Err(StoreError::InvalidData(format!(
            "legacy hash {:?} does not match project directory {}",
            legacy.hash, project_id
        )));
    }
    let source_id = ProjectSourceId::parse(format!("source-{project_id}"))
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    Ok(ProjectRecord {
        id: project_id.clone(),
        name: legacy.name,
        created_at: legacy.added_at.clone(),
        sources: vec![ProjectSource {
            id: source_id,
            project_id: project_id.clone(),
            kind: ProjectSourceKind::GitRepo,
            location: legacy.path,
            is_primary: true,
            status: ProjectSourceStatus::Available,
            created_at: legacy.added_at,
            last_observed_at: None,
            last_successful_refresh_at: None,
            last_error_category: None,
            revision: 0,
        }],
        capture_cursor: CaptureCursor {
            last_distilled: legacy.last_distilled,
            last_head: legacy.last_head,
            last_status_digest: legacy.last_status_digest,
            last_session_mtime: legacy.last_session_mtime,
        },
        cadence: legacy.cadence,
    })
}

fn validate_migration_record(
    project_id: &ProjectId,
    record: &ProjectRecord,
) -> Result<(), StoreError> {
    if &record.id != project_id {
        return Err(StoreError::InvalidData(format!(
            "stored project id {} does not match directory {project_id}",
            record.id
        )));
    }
    Ok(())
}

fn migration_ignore_contents(home: &Path) -> Result<String, StoreError> {
    let path = home.join(".gitignore");
    validate_store_file_target(home, &path)?;
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => return Err(StoreError::Io(error)),
    };
    if !text.lines().any(|line| line == "/.migration-v2") {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("/.migration-v2\n");
    }
    Ok(text)
}

fn failpoint(name: &str) -> Result<(), StoreError> {
    if std::env::var("OMNIPROJ_TEST_FAILPOINT").as_deref() == Ok(name) {
        Err(StoreError::InjectedFailure(name.to_owned()))
    } else {
        Ok(())
    }
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
pub fn with_store_txn<T, E>(f: impl FnOnce() -> Result<T, E>) -> Result<T, E>
where
    E: From<StoreError>,
{
    use fs2::FileExt;

    let home = omniproj_home();
    std::fs::create_dir_all(&home).map_err(StoreError::Io)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("store.lock"))
        .map_err(StoreError::Io)?;
    lock.try_lock_exclusive().map_err(StoreError::Io)?;
    f()
}

/// Replace a file durably without exposing a partially written destination.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), StoreError> {
    atomic_write_with_guards(path, contents, || Ok(()))
}

pub(crate) fn atomic_write_store(
    home: &Path,
    path: &Path,
    contents: &[u8],
) -> Result<(), StoreError> {
    validate_store_file_target(home, path)?;
    atomic_write_with_guards(path, contents, || validate_store_file_target(home, path))
}

fn atomic_write_with_guards(
    path: &Path,
    contents: &[u8],
    validate_target: impl Fn() -> Result<(), StoreError>,
) -> Result<(), StoreError> {
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
        validate_target()?;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        validate_target()?;
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

const PENDING_AUDIT_JOURNAL: &str = ".git/omniproj-pending-audit.toml";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PendingAuditPhase {
    Prepared,
    Applied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditTargetSnapshot {
    relative_path: PathBuf,
    prior: AuditPathIdentity,
    expected: AuditPathIdentity,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Round2AuditTargetSnapshot {
    relative_path: PathBuf,
    prior_sha256: Option<String>,
    expected_sha256: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AuditPathIdentity {
    Missing,
    RegularFile { sha256: String },
    Directory,
    Symlink { target: PathBuf },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingAudit {
    message: String,
    phase: PendingAuditPhase,
    targets: Vec<AuditTargetSnapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Round2PendingAudit {
    message: String,
    phase: PendingAuditPhase,
    targets: Vec<Round2AuditTargetSnapshot>,
}

fn upgrade_round2_audit_target(
    journal_path: &Path,
    target: Round2AuditTargetSnapshot,
) -> Result<AuditTargetSnapshot, StoreError> {
    let valid_hash =
        |hash: &str| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid_hash(&target.expected_sha256)
        || target
            .prior_sha256
            .as_deref()
            .is_some_and(|hash| !valid_hash(hash))
    {
        return Err(StoreError::InvalidData(format!(
            "{} has invalid Round-2 audit target snapshots",
            journal_path.display()
        )));
    }
    Ok(AuditTargetSnapshot {
        relative_path: target.relative_path,
        prior: target
            .prior_sha256
            .map(|sha256| AuditPathIdentity::RegularFile { sha256 })
            .unwrap_or(AuditPathIdentity::Missing),
        expected: AuditPathIdentity::RegularFile {
            sha256: target.expected_sha256,
        },
    })
}

pub(crate) fn audit_target_snapshot(
    home: &Path,
    relative_path: PathBuf,
    expected_contents: &[u8],
) -> Result<AuditTargetSnapshot, StoreError> {
    validate_relative_audit_path(&relative_path)?;
    validate_store_path_ancestors(home, &home.join(&relative_path))?;
    let prior = audit_path_identity(&home.join(&relative_path))?;
    if !matches!(
        prior,
        AuditPathIdentity::Missing | AuditPathIdentity::RegularFile { .. }
    ) {
        return Err(StoreError::AuditConflict {
            path: home.join(&relative_path),
        });
    }
    Ok(AuditTargetSnapshot {
        relative_path,
        prior,
        expected: AuditPathIdentity::RegularFile {
            sha256: sha256(expected_contents),
        },
    })
}

/// Record an exact-path audit before exposing its corresponding durable mutation.
/// Callers must hold the store lock until [`finish_pending_audit`] succeeds.
pub(crate) fn begin_pending_audit(
    home: &Path,
    message: &str,
    targets: &[AuditTargetSnapshot],
) -> Result<(), StoreError> {
    for target in targets {
        validate_relative_audit_path(&target.relative_path)?;
        if !valid_mutation_audit_target(target) {
            return Err(StoreError::InvalidData(
                "pending audit contains an invalid mutation snapshot".into(),
            ));
        }
    }
    validate_audit_target_paths(home, targets)?;
    let journal_path = home.join(PENDING_AUDIT_JOURNAL);
    if journal_path.exists() {
        return Err(StoreError::InvalidData(format!(
            "{} already exists; recover it before starting another mutation",
            journal_path.display()
        )));
    }
    let journal = PendingAudit {
        message: message.to_owned(),
        phase: PendingAuditPhase::Prepared,
        targets: targets.to_vec(),
    };
    write_pending_audit(&journal_path, &journal)
}

/// Mark the prepared mutation durable before attempting its audit commit.
pub(crate) fn mark_pending_audit_applied(home: &Path) -> Result<(), StoreError> {
    let journal_path = home.join(PENDING_AUDIT_JOURNAL);
    let mut journal = read_pending_audit(&journal_path)?;
    validate_audit_targets(home, &journal.targets, SnapshotSide::Expected)?;
    journal.phase = PendingAuditPhase::Applied;
    write_pending_audit(&journal_path, &journal)
}

/// Complete the exact-path audit recorded by [`begin_pending_audit`].
pub(crate) fn finish_pending_audit(home: &Path) -> Result<(), StoreError> {
    recover_pending_audit(home)
}

fn recover_pending_audit(home: &Path) -> Result<(), StoreError> {
    let journal_path = home.join(PENDING_AUDIT_JOURNAL);
    let mut journal = match read_pending_audit(&journal_path) {
        Ok(journal) => journal,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for target in &journal.targets {
        validate_relative_audit_path(&target.relative_path)?;
    }
    if journal.phase == PendingAuditPhase::Prepared {
        if audit_targets_match(home, &journal.targets, SnapshotSide::Prior)? {
            std::fs::remove_file(journal_path)?;
            return Ok(());
        }
        validate_audit_targets(home, &journal.targets, SnapshotSide::Expected)?;
        journal.phase = PendingAuditPhase::Applied;
        write_pending_audit(&journal_path, &journal)?;
    }
    validate_audit_targets(home, &journal.targets, SnapshotSide::Expected)?;
    let relative_paths: Vec<_> = journal
        .targets
        .iter()
        .map(|target| target.relative_path.clone())
        .collect();
    commit_paths_checked(&journal.message, &relative_paths)?;
    std::fs::remove_file(journal_path)?;
    Ok(())
}

fn recover_interrupted_fresh_init(home: &Path) -> Result<(), StoreError> {
    let journal_path = home.join(PENDING_AUDIT_JOURNAL);
    let journal = match read_pending_audit(&journal_path) {
        Ok(journal) => journal,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if journal.message != "init omniproj store" {
        return Ok(());
    }

    let mut saw_schema = false;
    for target in &journal.targets {
        let expected_contents: &[u8] = match target.relative_path.to_str() {
            Some(SCHEMA_VERSION_FILE) => {
                saw_schema = true;
                b"2\n"
            }
            Some(".gitignore") => INITIAL_GITIGNORE.as_bytes(),
            _ => {
                return Err(StoreError::InvalidData(format!(
                    "{} contains an invalid fresh-init target",
                    journal_path.display()
                )));
            }
        };
        if target.prior != AuditPathIdentity::Missing
            || target.expected != regular_identity(expected_contents)
        {
            return Err(StoreError::InvalidData(format!(
                "{} contains an invalid fresh-init snapshot",
                journal_path.display()
            )));
        }
    }
    if !saw_schema {
        return Err(StoreError::InvalidData(format!(
            "{} omits the fresh-init schema target",
            journal_path.display()
        )));
    }

    if journal.phase == PendingAuditPhase::Prepared {
        validate_targets_are_prior_or_expected(home, &journal.targets)?;
        for target in &journal.targets {
            if audit_path_identity(&home.join(&target.relative_path))? == target.prior {
                let contents: &[u8] = if target.relative_path == Path::new(SCHEMA_VERSION_FILE) {
                    b"2\n"
                } else {
                    INITIAL_GITIGNORE.as_bytes()
                };
                let path = home.join(&target.relative_path);
                atomic_write_store(home, &path, contents)?;
            }
        }
        mark_pending_audit_applied(home)?;
    }
    finish_pending_audit(home)
}

fn read_pending_audit(path: &Path) -> Result<PendingAudit, StoreError> {
    let text = std::fs::read_to_string(path)?;
    let journal: PendingAudit = if let Ok(journal) = toml::from_str(&text) {
        journal
    } else if let Ok(round2) = toml::from_str::<Round2PendingAudit>(&text) {
        PendingAudit {
            message: round2.message,
            phase: round2.phase,
            targets: round2
                .targets
                .into_iter()
                .map(|target| upgrade_round2_audit_target(path, target))
                .collect::<Result<_, _>>()?,
        }
    } else {
        return Err(StoreError::InvalidData(format!(
            "{} is malformed or does not match a supported pending-audit format",
            path.display()
        )));
    };
    if journal.targets.is_empty() {
        return Err(StoreError::InvalidData(format!(
            "{} contains no audit targets",
            path.display()
        )));
    }
    let mut paths = std::collections::HashSet::new();
    for target in &journal.targets {
        validate_relative_audit_path(&target.relative_path)?;
        if !paths.insert(&target.relative_path) || !valid_mutation_audit_target(target) {
            return Err(StoreError::InvalidData(format!(
                "{} has invalid audit target snapshots",
                path.display()
            )));
        }
    }
    let home = path
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| {
            StoreError::InvalidData(format!(
                "pending audit has no store parent: {}",
                path.display()
            ))
        })?;
    validate_audit_target_paths(home, &journal.targets)?;
    Ok(journal)
}

fn write_pending_audit(path: &Path, journal: &PendingAudit) -> Result<(), StoreError> {
    let text =
        toml::to_string(journal).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let home = path
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| {
            StoreError::InvalidData(format!(
                "pending audit has no store parent: {}",
                path.display()
            ))
        })?;
    atomic_write_store(home, path, text.as_bytes())
}

#[derive(Clone, Copy)]
enum SnapshotSide {
    Prior,
    Expected,
}

fn audit_targets_match(
    home: &Path,
    targets: &[AuditTargetSnapshot],
    side: SnapshotSide,
) -> Result<bool, StoreError> {
    for target in targets {
        validate_store_path_ancestors(home, &home.join(&target.relative_path))?;
        let actual = audit_path_identity(&home.join(&target.relative_path))?;
        let wanted = match side {
            SnapshotSide::Prior => &target.prior,
            SnapshotSide::Expected => &target.expected,
        };
        if &actual != wanted {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_audit_targets(
    home: &Path,
    targets: &[AuditTargetSnapshot],
    side: SnapshotSide,
) -> Result<(), StoreError> {
    for target in targets {
        validate_store_path_ancestors(home, &home.join(&target.relative_path))?;
        let actual = audit_path_identity(&home.join(&target.relative_path))?;
        let wanted = match side {
            SnapshotSide::Prior => &target.prior,
            SnapshotSide::Expected => &target.expected,
        };
        if &actual != wanted {
            return Err(StoreError::AuditConflict {
                path: home.join(&target.relative_path),
            });
        }
    }
    Ok(())
}

fn validate_audit_target_paths(
    home: &Path,
    targets: &[AuditTargetSnapshot],
) -> Result<(), StoreError> {
    for target in targets {
        validate_store_file_target(home, &home.join(&target.relative_path))?;
    }
    Ok(())
}

fn audit_path_identity(path: &Path) -> Result<AuditPathIdentity, StoreError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(AuditPathIdentity::Missing),
        Err(error) => return Err(StoreError::Io(error)),
    };
    let file_type = metadata.file_type();
    if file_type.is_file() {
        return Ok(AuditPathIdentity::RegularFile {
            sha256: sha256(&std::fs::read(path)?),
        });
    }
    if file_type.is_dir() {
        return Ok(AuditPathIdentity::Directory);
    }
    if file_type.is_symlink() {
        return Ok(AuditPathIdentity::Symlink {
            target: std::fs::read_link(path)?,
        });
    }
    Err(StoreError::AuditConflict {
        path: path.to_owned(),
    })
}

pub(crate) fn validate_store_file_target(home: &Path, path: &Path) -> Result<(), StoreError> {
    validate_store_path_ancestors(home, path)?;
    if matches!(
        audit_path_identity(path)?,
        AuditPathIdentity::Missing | AuditPathIdentity::RegularFile { .. }
    ) {
        Ok(())
    } else {
        Err(StoreError::AuditConflict {
            path: path.to_owned(),
        })
    }
}

pub(crate) fn validate_store_directory_target(
    home: &Path,
    path: &Path,
    allow_missing: bool,
) -> Result<(), StoreError> {
    validate_store_path_ancestors(home, path)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Err(error) if allow_missing && error.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(StoreError::AuditConflict {
            path: path.to_owned(),
        }),
        Err(error) => Err(StoreError::Io(error)),
    }
}

pub(crate) fn validate_store_missing_target(home: &Path, path: &Path) -> Result<(), StoreError> {
    validate_store_path_ancestors(home, path)?;
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(StoreError::AuditConflict {
            path: path.to_owned(),
        }),
        Err(error) => Err(StoreError::Io(error)),
    }
}

fn validate_store_path_ancestors(home: &Path, path: &Path) -> Result<(), StoreError> {
    let relative = path.strip_prefix(home).map_err(|_| {
        StoreError::InvalidData(format!(
            "store target escapes {}: {}",
            home.display(),
            path.display()
        ))
    })?;
    let components: Vec<_> = relative.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(StoreError::InvalidData(format!(
            "store target is not a normalized descendant: {}",
            path.display()
        )));
    }
    let canonical_home = std::fs::canonicalize(home)?;
    let mut current = canonical_home;
    let mut logical = PathBuf::new();
    for component in components.iter().take(components.len() - 1) {
        current.push(component.as_os_str());
        logical.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(StoreError::AuditConflict {
                    path: home.join(&logical),
                });
            }
            Err(error) => return Err(StoreError::Io(error)),
        }
    }
    Ok(())
}

fn sha256(contents: &[u8]) -> String {
    Sha256::digest(contents)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    for path in relative_paths {
        validate_store_file_target(&home, &home.join(path))?;
    }
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

    for path in relative_paths {
        validate_store_file_target(&home, &home.join(path))?;
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
    fn fresh_initialization_holds_the_store_lock_before_git_becomes_visible() {
        let _g = crate::env_guard();
        let home = unique_home("fresh-init-race");
        std::env::set_var("OMNIPROJ_HOME", &home);
        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_fresh_init_test_pause(reached.clone(), release.clone());

        let first = std::thread::spawn(ensure_home);
        reached.wait();
        let concurrent = ensure_home();
        release.wait();
        first.join().unwrap().unwrap();

        assert!(
            matches!(concurrent, Err(StoreError::Io(ref error)) if error.kind() == ErrorKind::WouldBlock),
            "second startup must observe checked lock contention, got {concurrent:?}"
        );
        ensure_home().unwrap();
        assert_eq!(read_version(&home), CURRENT_SCHEMA_VERSION.to_string());
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn fresh_initialization_commits_only_tool_created_paths() {
        let _g = crate::env_guard();
        let home = unique_home("fresh-init-exact-audit");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("Human.md"), b"Pre-existing Human bytes\n").unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);

        ensure_home().unwrap();

        let mut names: Vec<_> = git_output(&home, &["show", "--format=", "--name-only", "HEAD"])
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        names.sort();
        assert_eq!(names, vec![".gitignore", SCHEMA_VERSION_FILE]);
        assert_eq!(
            git_output(&home, &["status", "--short", "--", "Human.md"]),
            "?? Human.md\n"
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn fresh_initialization_rejects_a_preexisting_schema_before_git_init() {
        let _g = crate::env_guard();
        for kind in ["regular", "directory"] {
            let home = unique_home(&format!("fresh-preexisting-schema-{kind}"));
            std::fs::create_dir_all(&home).unwrap();
            let schema = home.join(SCHEMA_VERSION_FILE);
            if kind == "regular" {
                std::fs::write(&schema, b"9\n").unwrap();
            } else {
                std::fs::create_dir(&schema).unwrap();
                std::fs::write(schema.join("Human.md"), b"Human directory bytes\n").unwrap();
            }
            std::env::set_var("OMNIPROJ_HOME", &home);

            let error = ensure_home().unwrap_err();

            assert!(matches!(error, StoreError::MigrationConflict { .. }));
            assert!(!home.join(".git").exists());
            if kind == "regular" {
                assert_eq!(std::fs::read(&schema).unwrap(), b"9\n");
            } else {
                assert_eq!(
                    std::fs::read(schema.join("Human.md")).unwrap(),
                    b"Human directory bytes\n"
                );
            }
            std::env::remove_var("OMNIPROJ_HOME");
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn fresh_initialization_rejects_schema_and_gitignore_symlinks_before_git_init() {
        use std::os::unix::fs::symlink;

        let _g = crate::env_guard();
        for relative in [SCHEMA_VERSION_FILE, ".gitignore"] {
            let home = unique_home(&format!("fresh-{relative}-symlink"));
            std::fs::create_dir_all(&home).unwrap();
            let external = home.join("Human-target");
            std::fs::write(&external, b"Human target bytes\n").unwrap();
            symlink(&external, home.join(relative)).unwrap();
            std::env::set_var("OMNIPROJ_HOME", &home);

            let error = ensure_home().unwrap_err();

            assert!(matches!(
                error,
                StoreError::MigrationConflict { .. } | StoreError::AuditConflict { .. }
            ));
            assert!(!home.join(".git").exists());
            assert_eq!(std::fs::read_link(home.join(relative)).unwrap(), external);
            assert_eq!(std::fs::read(&external).unwrap(), b"Human target bytes\n");
            std::env::remove_var("OMNIPROJ_HOME");
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn fresh_initialization_rejects_a_gitignore_directory_before_git_init() {
        let _g = crate::env_guard();
        let home = unique_home("fresh-gitignore-directory");
        std::fs::create_dir_all(home.join(".gitignore")).unwrap();
        std::fs::write(home.join(".gitignore/Human.md"), b"Human directory bytes\n").unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);

        let error = ensure_home().unwrap_err();

        assert!(matches!(error, StoreError::AuditConflict { .. }));
        assert!(!home.join(".git").exists());
        assert_eq!(
            std::fs::read(home.join(".gitignore/Human.md")).unwrap(),
            b"Human directory bytes\n"
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fresh_initialization_preserves_a_regular_gitignore_across_audit_retry() {
        use std::os::unix::fs::PermissionsExt;

        let _g = crate::env_guard();
        let home = unique_home("fresh-human-gitignore-retry");
        std::fs::create_dir_all(&home).unwrap();
        let human_ignore = b"# Human ignore bytes\nprivate/\n";
        std::fs::write(home.join(".gitignore"), human_ignore).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_fresh_init_test_pause(reached.clone(), release.clone());

        let first = std::thread::spawn(ensure_home);
        reached.wait();
        let hook = home.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook, permissions).unwrap();
        release.wait();
        assert!(matches!(
            first.join().unwrap(),
            Err(StoreError::AuditCommit(_))
        ));
        std::fs::remove_file(hook).unwrap();

        ensure_home().unwrap();

        assert_eq!(
            std::fs::read(home.join(".gitignore")).unwrap(),
            human_ignore
        );
        assert_eq!(
            git_output(&home, &["show", "--format=", "--name-only", "HEAD"]),
            format!("{SCHEMA_VERSION_FILE}\n")
        );
        assert_eq!(
            git_output(&home, &["status", "--short", "--", ".gitignore"]),
            "?? .gitignore\n"
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fresh_initialization_recovers_an_exact_audit_after_commit_failure() {
        use std::os::unix::fs::PermissionsExt;

        let _g = crate::env_guard();
        let home = unique_home("fresh-init-audit-recovery");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("Human.md"), b"Pre-existing Human bytes\n").unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_fresh_init_test_pause(reached.clone(), release.clone());

        let first = std::thread::spawn(ensure_home);
        reached.wait();
        let hook = home.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook,
            "#!/bin/sh\necho forced init audit failure >&2\nexit 1\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook, permissions).unwrap();
        release.wait();

        assert!(matches!(
            first.join().unwrap(),
            Err(StoreError::AuditCommit(_))
        ));
        assert!(home.join(PENDING_AUDIT_JOURNAL).exists());
        std::fs::remove_file(hook).unwrap();

        ensure_home().unwrap();

        let mut names: Vec<_> = git_output(&home, &["show", "--format=", "--name-only", "HEAD"])
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        names.sort();
        assert_eq!(names, vec![".gitignore", SCHEMA_VERSION_FILE]);
        assert_eq!(
            git_output(&home, &["status", "--short", "--", "Human.md"]),
            "?? Human.md\n"
        );
        assert!(!home.join(PENDING_AUDIT_JOURNAL).exists());
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn fresh_initialization_recovers_a_prepared_partial_write() {
        let _g = crate::env_guard();
        for (failpoint, schema_written) in [
            ("fresh_init_after_gitignore_write", false),
            ("fresh_init_after_schema_write_before_applied", true),
        ] {
            let home = unique_home(failpoint);
            std::fs::create_dir_all(&home).unwrap();
            std::fs::write(home.join("Human.md"), b"Pre-existing Human bytes\n").unwrap();
            std::env::set_var("OMNIPROJ_HOME", &home);
            std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);

            assert!(ensure_home().is_err());
            assert!(home.join(".gitignore").exists());
            assert_eq!(home.join(SCHEMA_VERSION_FILE).exists(), schema_written);
            assert!(home.join(PENDING_AUDIT_JOURNAL).exists());

            std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
            ensure_home().unwrap();
            assert_eq!(read_version(&home), CURRENT_SCHEMA_VERSION.to_string());
            assert_eq!(
                git_output(&home, &["log", "--format=%s"]),
                "init omniproj store\n"
            );
            assert_eq!(
                git_output(&home, &["status", "--short", "--", "Human.md"]),
                "?? Human.md\n"
            );
            assert!(!home.join(PENDING_AUDIT_JOURNAL).exists());
            std::env::remove_var("OMNIPROJ_HOME");
            std::fs::remove_dir_all(home).unwrap();
        }
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
            other => panic!("expected audit commit error, got {other}"),
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
                other => panic!("expected audit path validation error, got {other}"),
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

    /// A pre-versioning store (has `.git`, no version file) is interpreted as v1 and
    /// migrated to the current schema without touching unrelated existing state.
    #[test]
    fn existing_store_missing_version_migrates_from_v1() {
        let _g = crate::env_guard();
        let home = unique_home("adopt");
        std::env::set_var("OMNIPROJ_HOME", &home);
        // Simulate an existing, pre-versioning store with a real audit repository and
        // no version file. Schema v2 migration uses checked commits, so a placeholder
        // `.git` directory is intentionally insufficient.
        std::fs::create_dir_all(&home).unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(&home)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        let sentinel = home.join("projects/abc/auto/briefing.md");
        std::fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
        std::fs::write(&sentinel, "original briefing").unwrap();

        ensure_home().unwrap();

        assert_eq!(read_version(&home), CURRENT_SCHEMA_VERSION.to_string());
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
