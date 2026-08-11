//! Project registry (spec §4.1). A tracked project is a `~/.omniproj/projects/<hash>/`
//! dir with a `meta.toml`. Registration is explicit (`omniproj add`) and lives entirely
//! in `~/.omniproj`, never in the user's repo (charter §5 原则2).
#![allow(deprecated)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fmt, io};

use serde::{Deserialize, Serialize};

use crate::ids::{ProjectId, ProjectSourceId};
#[cfg(test)]
use crate::paths::project_hash;
use crate::paths::{omniproj_home, project_dir, project_dir_for};
use crate::project_state::ProjectStateDoc;
use crate::store::{
    atomic_write_store, audit_target_snapshot, begin_pending_audit, ensure_home,
    finish_pending_audit, mark_pending_audit_applied, validate_store_directory_target,
    validate_store_missing_target, with_store_txn, StoreError,
};

#[cfg(test)]
type RegistrationTestPause = (
    std::sync::Arc<std::sync::Barrier>,
    std::sync::Arc<std::sync::Barrier>,
);
#[cfg(test)]
static REGISTRATION_TEST_PAUSE: std::sync::OnceLock<
    std::sync::Mutex<Option<RegistrationTestPause>>,
> = std::sync::OnceLock::new();
#[cfg(test)]
static LEGACY_CURSOR_TEST_PAUSE: std::sync::OnceLock<
    std::sync::Mutex<Option<RegistrationTestPause>>,
> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSourceKind {
    GitRepo,
    Session,
    DocumentPath,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSourceStatus {
    Available,
    Moved,
    Unreadable,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSource {
    pub id: ProjectSourceId,
    pub project_id: ProjectId,
    pub kind: ProjectSourceKind,
    pub location: String,
    pub is_primary: bool,
    pub status: ProjectSourceStatus,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_refresh_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_category: Option<String>,
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureCursor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_distilled: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_session_mtime: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub name: String,
    pub created_at: String,
    pub sources: Vec<ProjectSource>,
    pub capture_cursor: CaptureCursor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<Cadence>,
}

#[derive(Debug)]
pub enum ProjectStoreError {
    NotFound(ProjectId),
    SourceNotFound {
        project_id: ProjectId,
        source_id: ProjectSourceId,
    },
    InvalidPath {
        path: PathBuf,
        message: String,
    },
    InvalidInput(String),
    DuplicateSource {
        existing_project_id: ProjectId,
    },
    RevisionConflict {
        expected: u64,
        actual: u64,
    },
    LocationConflict {
        expected: String,
        actual: String,
    },
    InvalidRecord {
        path: PathBuf,
        message: String,
    },
    Io(io::Error),
    Store(StoreError),
}

impl fmt::Display for ProjectStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "project {id} was not found"),
            Self::SourceNotFound {
                project_id,
                source_id,
            } => write!(
                f,
                "source {source_id} was not found in project {project_id}"
            ),
            Self::InvalidPath { path, message } => {
                write!(f, "invalid source path {}: {message}", path.display())
            }
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::DuplicateSource {
                existing_project_id,
            } => write!(f, "source already belongs to project {existing_project_id}"),
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "source revision conflict: expected {expected}, found {actual}"
            ),
            Self::LocationConflict { expected, actual } => write!(
                f,
                "source location conflict: expected {expected:?}, found {actual:?}"
            ),
            Self::InvalidRecord { path, message } => {
                write!(f, "invalid project record {}: {message}", path.display())
            }
            Self::Io(error) => error.fmt(f),
            Self::Store(error) => error.fmt(f),
        }
    }
}

pub struct RegisterProjectInput<'a> {
    pub location: &'a Path,
    pub name: &'a str,
    pub created_at: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegisterOutcome {
    Created(ProjectRecord),
    Existing(ProjectId),
}

pub struct RelinkSourceInput<'a> {
    pub project_id: &'a ProjectId,
    pub expected_source_revision: u64,
    pub expected_location: &'a str,
    pub new_location: &'a Path,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceObservationOutcome<'a> {
    Success {
        successful_refresh_at: &'a str,
    },
    Failure {
        status: ProjectSourceStatus,
        error_category: &'a str,
    },
}

pub struct RecordSourceObservationInput<'a> {
    pub project_id: &'a ProjectId,
    pub source_id: &'a ProjectSourceId,
    pub expected_source_revision: u64,
    pub expected_location: &'a str,
    pub attempted_at: &'a str,
    pub outcome: SourceObservationOutcome<'a>,
}

impl std::error::Error for ProjectStoreError {}

impl From<io::Error> for ProjectStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StoreError> for ProjectStoreError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl ProjectRecord {
    pub fn storage_id(&self) -> &ProjectId {
        &self.id
    }

    pub fn primary_git_source(&self) -> Option<&ProjectSource> {
        self.sources
            .iter()
            .find(|source| source.is_primary && source.kind == ProjectSourceKind::GitRepo)
    }

    pub fn primary_git_source_mut(&mut self) -> Option<&mut ProjectSource> {
        self.sources
            .iter_mut()
            .find(|source| source.is_primary && source.kind == ProjectSourceKind::GitRepo)
    }
}

pub(crate) fn project_record_path(id: &ProjectId) -> PathBuf {
    project_dir_for(id).join("meta.toml")
}

pub(crate) fn parse_project_record(
    path: &Path,
    text: &str,
) -> Result<ProjectRecord, ProjectStoreError> {
    let record: ProjectRecord =
        toml::from_str(text).map_err(|error| ProjectStoreError::InvalidRecord {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    validate_project_record(path, &record)?;
    Ok(record)
}

pub(crate) fn validate_project_record(
    path: &Path,
    record: &ProjectRecord,
) -> Result<(), ProjectStoreError> {
    let invalid = |message: &str| ProjectStoreError::InvalidRecord {
        path: path.to_owned(),
        message: message.into(),
    };
    if record.name.trim().is_empty() {
        return Err(invalid("project name must not be empty"));
    }
    if chrono::DateTime::parse_from_rfc3339(&record.created_at).is_err() {
        return Err(invalid("project created_at must be RFC3339"));
    }
    if record
        .capture_cursor
        .last_distilled
        .as_deref()
        .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_err())
    {
        return Err(invalid("capture_cursor last_distilled must be RFC3339"));
    }
    if record.sources.is_empty() {
        return Err(invalid("project must contain at least one source"));
    }
    let mut source_ids = HashSet::new();
    for source in &record.sources {
        if source.project_id != record.id {
            return Err(invalid("source project_id does not match project id"));
        }
        if !source_ids.insert(source.id.clone()) {
            return Err(invalid("project contains duplicate source ids"));
        }
        if source.location.trim().is_empty() {
            return Err(invalid("source location must not be empty"));
        }
        for (field, value) in [
            ("source created_at", Some(source.created_at.as_str())),
            (
                "source last_observed_at",
                source.last_observed_at.as_deref(),
            ),
            (
                "source last_successful_refresh_at",
                source.last_successful_refresh_at.as_deref(),
            ),
        ] {
            if value.is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_err()) {
                return Err(invalid(&format!("{field} must be RFC3339")));
            }
        }
        if source
            .last_error_category
            .as_deref()
            .is_some_and(|category| category.trim().is_empty())
        {
            return Err(invalid("source last_error_category must not be empty"));
        }
        match source.status {
            ProjectSourceStatus::Available if source.last_error_category.is_some() => {
                return Err(invalid(
                    "available source must not retain an error category",
                ));
            }
            ProjectSourceStatus::Moved
            | ProjectSourceStatus::Unreadable
            | ProjectSourceStatus::Missing
                if source.last_error_category.is_none() =>
            {
                return Err(invalid("unavailable source must record an error category"));
            }
            _ => {}
        }
    }
    let primary_sources: Vec<_> = record
        .sources
        .iter()
        .filter(|source| source.is_primary)
        .collect();
    if primary_sources.len() != 1 || primary_sources[0].kind != ProjectSourceKind::GitRepo {
        return Err(invalid(
            "project must contain exactly one primary Git source",
        ));
    }
    Ok(())
}

pub(crate) fn render_project_record(record: &ProjectRecord) -> Result<String, ProjectStoreError> {
    validate_project_record(&project_record_path(&record.id), record)?;
    toml::to_string_pretty(record).map_err(|error| ProjectStoreError::InvalidRecord {
        path: project_record_path(&record.id),
        message: error.to_string(),
    })
}

pub fn load_project(id: &ProjectId) -> Result<ProjectRecord, ProjectStoreError> {
    let path = project_record_path(id);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ProjectStoreError::NotFound(id.clone()));
        }
        Err(error) => return Err(ProjectStoreError::Io(error)),
    };
    let record = parse_project_record(&path, &text)?;
    if record.id != *id {
        return Err(ProjectStoreError::InvalidRecord {
            path,
            message: format!(
                "stored project id {} does not match directory {id}",
                record.id
            ),
        });
    }
    Ok(record)
}

pub fn list_project_records() -> Result<Vec<ProjectRecord>, ProjectStoreError> {
    let root = omniproj_home().join("projects");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(ProjectStoreError::Io(error)),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with(".staging-") {
            continue;
        }
        let Ok(id) = ProjectId::parse(&name) else {
            continue;
        };
        records.push(load_project(&id)?);
    }
    records.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(records)
}

pub fn canonical_source_owner(location: &Path) -> Result<Option<ProjectId>, ProjectStoreError> {
    let canonical = canonical_location(location)?;
    for record in list_project_records()? {
        let Some(source) = record.primary_git_source() else {
            continue;
        };
        let stored = match std::fs::canonicalize(&source.location) {
            Ok(path) => path.to_string_lossy().into_owned(),
            Err(_) => source.location.clone(),
        };
        if stored == canonical {
            return Ok(Some(record.id));
        }
    }
    Ok(None)
}

pub fn register_project(
    input: RegisterProjectInput<'_>,
) -> Result<RegisterOutcome, ProjectStoreError> {
    let canonical = canonical_location(input.location)?;
    validate_nonempty("name", input.name)?;
    ProjectStateDoc::new_setup(input.created_at)
        .map_err(|error| ProjectStoreError::InvalidInput(error.to_string()))?;
    ensure_home()?;
    if let Some(existing) = canonical_owner_for_location(&canonical)? {
        return Ok(RegisterOutcome::Existing(existing));
    }

    with_store_txn(|| {
        if let Some(existing) = canonical_owner_for_location(&canonical)? {
            return Ok(RegisterOutcome::Existing(existing));
        }
        let project_id = ProjectId::new();
        let source_id = ProjectSourceId::new();
        let record = ProjectRecord {
            id: project_id.clone(),
            name: input.name.to_owned(),
            created_at: input.created_at.to_owned(),
            sources: vec![ProjectSource {
                id: source_id,
                project_id: project_id.clone(),
                kind: ProjectSourceKind::GitRepo,
                location: canonical.clone(),
                is_primary: true,
                status: ProjectSourceStatus::Available,
                created_at: input.created_at.to_owned(),
                last_observed_at: None,
                last_successful_refresh_at: None,
                last_error_category: None,
                revision: 0,
            }],
            capture_cursor: CaptureCursor::default(),
            cadence: None,
        };

        let home = omniproj_home();
        let projects_root = home.join("projects");
        validate_store_directory_target(&home, &projects_root, true)?;
        std::fs::create_dir_all(&projects_root)?;
        validate_store_directory_target(&home, &projects_root, false)?;
        let staging = projects_root.join(format!(".staging-{project_id}"));
        validate_store_directory_target(&home, &staging, true)?;
        std::fs::create_dir_all(&staging)?;
        validate_store_directory_target(&home, &staging, false)?;
        for subdir in ["auto", "notes", "cache"] {
            let path = staging.join(subdir);
            validate_store_directory_target(&home, &path, true)?;
            std::fs::create_dir_all(&path)?;
            validate_store_directory_target(&home, &path, false)?;
        }
        registration_test_pause_after_staging_skeleton();
        let state_text = ProjectStateDoc::new_setup(input.created_at)
            .map_err(|error| ProjectStoreError::InvalidInput(error.to_string()))?
            .render()
            .map_err(|error| ProjectStoreError::InvalidInput(error.to_string()))?;
        atomic_write_store(
            &home,
            &staging.join("notes/project.md"),
            state_text.as_bytes(),
        )?;
        registry_failpoint("registration_after_project_state_write")?;
        write_record_to_path(&home, &record, &staging.join("meta.toml"))?;
        registry_failpoint("registration_after_metadata_write")?;
        validate_store_directory_target(&home, &staging, false)?;
        sync_directory(&staging)?;
        let audit_targets = [
            audit_target_snapshot(
                &home,
                PathBuf::from(format!("projects/{project_id}/meta.toml")),
                &std::fs::read(staging.join("meta.toml"))?,
            )?,
            audit_target_snapshot(
                &home,
                PathBuf::from(format!("projects/{project_id}/notes/project.md")),
                &std::fs::read(staging.join("notes/project.md"))?,
            )?,
        ];
        registry_failpoint("registration_before_directory_rename")?;
        begin_pending_audit(&home, "project: register source", &audit_targets)?;

        let final_dir = project_dir_for(&project_id);
        registry_failpoint("registration_directory_rename_failure")?;
        validate_store_directory_target(&home, &staging, false)?;
        validate_store_missing_target(&home, &final_dir)?;
        std::fs::rename(&staging, &final_dir)?;
        registry_failpoint("registration_parent_fsync_failure")?;
        validate_store_directory_target(&home, &projects_root, false)?;
        sync_directory(&projects_root)?;
        mark_pending_audit_applied(&home)?;
        finish_pending_audit(&home)?;
        Ok(RegisterOutcome::Created(record))
    })
}

pub fn relink_primary_git_source(
    input: RelinkSourceInput<'_>,
) -> Result<ProjectRecord, ProjectStoreError> {
    let canonical = canonical_location(input.new_location)?;
    ensure_home()?;
    with_store_txn(|| {
        if let Some(existing_project_id) = canonical_owner_for_location(&canonical)? {
            if existing_project_id != *input.project_id {
                return Err(ProjectStoreError::DuplicateSource {
                    existing_project_id,
                });
            }
        }
        let mut project = load_project(input.project_id)?;
        let source =
            project
                .primary_git_source_mut()
                .ok_or_else(|| ProjectStoreError::InvalidRecord {
                    path: project_record_path(input.project_id),
                    message: "primary Git source is missing".into(),
                })?;
        compare_source(
            source,
            input.expected_source_revision,
            input.expected_location,
        )?;
        source.location = canonical;
        source.status = ProjectSourceStatus::Available;
        source.last_error_category = None;
        source.revision += 1;
        save_and_audit_record(&project, "project: relink primary source")?;
        Ok(project)
    })
}

pub fn record_source_observation(
    input: RecordSourceObservationInput<'_>,
) -> Result<ProjectRecord, ProjectStoreError> {
    ensure_home()?;
    with_store_txn(|| {
        let mut project = load_project(input.project_id)?;
        let source = project
            .sources
            .iter_mut()
            .find(|source| source.id == *input.source_id)
            .ok_or_else(|| ProjectStoreError::SourceNotFound {
                project_id: input.project_id.clone(),
                source_id: input.source_id.clone(),
            })?;
        compare_source(
            source,
            input.expected_source_revision,
            input.expected_location,
        )?;
        validate_rfc3339("attempted_at", input.attempted_at)?;
        source.last_observed_at = Some(input.attempted_at.to_owned());
        match input.outcome {
            SourceObservationOutcome::Success {
                successful_refresh_at,
            } => {
                validate_rfc3339("successful_refresh_at", successful_refresh_at)?;
                source.status = ProjectSourceStatus::Available;
                source.last_successful_refresh_at = Some(successful_refresh_at.to_owned());
                source.last_error_category = None;
            }
            SourceObservationOutcome::Failure {
                status,
                error_category,
            } => {
                if status == ProjectSourceStatus::Available {
                    return Err(ProjectStoreError::InvalidInput(
                        "failure observation status cannot be available".into(),
                    ));
                }
                validate_nonempty("error_category", error_category)?;
                source.status = status;
                source.last_error_category = Some(error_category.to_owned());
            }
        }
        source.revision += 1;
        save_and_audit_record(&project, "project: record source observation")?;
        Ok(project)
    })
}

pub fn find_project_by_cwd(cwd: &Path) -> Result<Option<ProjectRecord>, ProjectStoreError> {
    let canonical = canonical_location(cwd)?;
    let cwd = Path::new(&canonical);
    Ok(list_project_records()?
        .into_iter()
        .filter_map(|project| {
            let source = project.primary_git_source()?;
            let location = PathBuf::from(&source.location);
            cwd.starts_with(&location)
                .then_some((location.components().count(), project))
        })
        .max_by_key(|(components, _)| *components)
        .map(|(_, project)| project))
}

fn canonical_owner_for_location(
    canonical_location: &str,
) -> Result<Option<ProjectId>, ProjectStoreError> {
    for record in list_project_records()? {
        let Some(source) = record.primary_git_source() else {
            continue;
        };
        let stored = match std::fs::canonicalize(&source.location) {
            Ok(path) => path.to_string_lossy().into_owned(),
            Err(_) => source.location.clone(),
        };
        if stored == canonical_location {
            return Ok(Some(record.id));
        }
    }
    Ok(None)
}

fn canonical_location(location: &Path) -> Result<String, ProjectStoreError> {
    let canonical =
        std::fs::canonicalize(location).map_err(|error| ProjectStoreError::InvalidPath {
            path: location.to_owned(),
            message: error.to_string(),
        })?;
    if !canonical.is_dir() {
        return Err(ProjectStoreError::InvalidPath {
            path: canonical,
            message: "Git source must be a directory".into(),
        });
    }
    Ok(canonical.to_string_lossy().into_owned())
}

fn compare_source(
    source: &ProjectSource,
    expected_revision: u64,
    expected_location: &str,
) -> Result<(), ProjectStoreError> {
    if source.revision != expected_revision {
        return Err(ProjectStoreError::RevisionConflict {
            expected: expected_revision,
            actual: source.revision,
        });
    }
    if source.location != expected_location {
        return Err(ProjectStoreError::LocationConflict {
            expected: expected_location.to_owned(),
            actual: source.location.clone(),
        });
    }
    Ok(())
}

fn write_record_to_path(
    home: &Path,
    record: &ProjectRecord,
    path: &Path,
) -> Result<(), ProjectStoreError> {
    let text = render_project_record(record)?;
    atomic_write_store(home, path, text.as_bytes())?;
    Ok(())
}

fn save_and_audit_record(record: &ProjectRecord, message: &str) -> Result<(), ProjectStoreError> {
    let home = omniproj_home();
    let relative_path = PathBuf::from(format!("projects/{}/meta.toml", record.id));
    let text = render_project_record(record)?;
    let audit_targets = [audit_target_snapshot(
        &home,
        relative_path,
        text.as_bytes(),
    )?];
    begin_pending_audit(&home, message, &audit_targets)?;
    atomic_write_store(&home, &project_record_path(&record.id), text.as_bytes())?;
    mark_pending_audit_applied(&home)?;
    finish_pending_audit(&home)?;
    Ok(())
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), ProjectStoreError> {
    if value.trim().is_empty() {
        Err(ProjectStoreError::InvalidInput(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_rfc3339(field: &str, value: &str) -> Result<(), ProjectStoreError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| ProjectStoreError::InvalidInput(format!("{field} must be RFC3339")))
}

fn sync_directory(path: &Path) -> Result<(), ProjectStoreError> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn registry_failpoint(name: &str) -> Result<(), ProjectStoreError> {
    if std::env::var("OMNIPROJ_TEST_FAILPOINT").as_deref() == Ok(name) {
        Err(ProjectStoreError::Store(StoreError::InjectedFailure(
            name.to_owned(),
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn registration_test_pause_after_staging_skeleton() {
    let pause = REGISTRATION_TEST_PAUSE
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
fn registration_test_pause_after_staging_skeleton() {}

#[cfg(test)]
fn install_registration_test_pause(
    reached: std::sync::Arc<std::sync::Barrier>,
    release: std::sync::Arc<std::sync::Barrier>,
) {
    *REGISTRATION_TEST_PAUSE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some((reached, release));
}

/// Per-project metadata. Tool-managed but human-readable/editable.
#[deprecated(note = "use ProjectRecord; this is a staged-migration adapter")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Absolute path of the tracked directory (identity source).
    pub path: String,
    pub name: String,
    pub hash: String,
    /// RFC3339, supplied by the caller (core stays free of a clock dependency at call sites).
    pub added_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_distilled: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_head: Option<String>,
    /// Cursor: digest of full `git status --porcelain` at the last distill.
    /// This catches dirty worktree/staging/untracked changes even when HEAD did not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status_digest: Option<String>,
    /// Cursor: mtime (epoch secs) of the newest session seen at the last distill.
    /// Part of the change fingerprint (with `last_head` and `last_status_digest`);
    /// see [`Fingerprint`] (spec §5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_session_mtime: Option<f64>,
    /// Per-project cadence overrides (charter §5 原则6: cadence 可为不同项目、不同
    /// 阶段设置). Absent → the project follows the global config / daemon defaults.
    /// Additive optional field: an older store simply lacks it, so no schema bump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<Cadence>,
}

/// Per-project cadence knobs (charter §5 原则6). Both optional so a project can
/// override just one. Empty tables serialize to nothing (all `None` → parent field
/// skipped), keeping `meta.toml` clean for projects that don't customize cadence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cadence {
    /// Staleness-floor override in seconds — how long the daemon may go without a
    /// forced refresh for THIS project (孵化期调高、冲刺期调低). None → global floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_floor_secs: Option<u64>,
    /// Reasoning depth override for this project: "shallow" | "deep". None → config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<String>,
}

impl Cadence {
    /// True when nothing is set — used to drop an all-empty table rather than persist it.
    pub fn is_empty(&self) -> bool {
        self.refresh_floor_secs.is_none() && self.depth.is_none()
    }
}

/// A deterministic change signal for a tracked project (spec §5): the current git
/// `HEAD` plus the newest captured session mtime. Compared against the cursor stored
/// in [`ProjectMeta`] to decide whether a re-distill is warranted — no LLM, zero cost.
/// `None` fields mean "no git" / "no sessions", which still register as change when
/// the cursor side differs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fingerprint {
    pub head: Option<String>,
    pub status_digest: Option<String>,
    pub latest_session_mtime: Option<f64>,
}

impl Fingerprint {
    /// Has the substrate changed since `meta`'s last distill? `true` when never
    /// distilled, when `HEAD` moved, or when a newer session appeared. Pure +
    /// deterministic so the staleness floor is testable without IO (spec §5.2).
    pub fn is_stale(&self, meta: &ProjectMeta) -> bool {
        // Never distilled → always stale (first run must produce output).
        if meta.last_distilled.is_none() {
            return true;
        }
        if self.head != meta.last_head {
            return true;
        }
        if self.status_digest != meta.last_status_digest {
            return true;
        }
        match (self.latest_session_mtime, meta.last_session_mtime) {
            // A strictly-newer session than the cursor → new conversation to fold in.
            (Some(now), Some(prev)) => now > prev,
            // Sessions appeared where there were none before.
            (Some(_), None) => true,
            // No sessions now: mtime side adds no new signal (HEAD already compared).
            (None, _) => false,
        }
    }
}

pub fn meta_path(hash: &str) -> PathBuf {
    project_dir(hash).join("meta.toml")
}

#[deprecated(note = "use load_project with a typed ProjectId")]
pub fn load_meta(hash: &str) -> Option<ProjectMeta> {
    let id = ProjectId::parse(hash).ok()?;
    load_project(&id).ok().and_then(project_meta_from_record)
}

/// Register (or refresh) a tracked project. Creates the `auto/`/`notes/`/`cache/`
/// skeleton and writes `meta.toml`. Idempotent: re-registering updates path/name
/// but preserves distill bookkeeping. `now` is an RFC3339 timestamp from the caller.
#[deprecated(note = "use register_project")]
pub fn register(abs_path: &str, name: &str, now: &str) -> std::io::Result<ProjectMeta> {
    let outcome = register_project(RegisterProjectInput {
        location: Path::new(abs_path),
        name,
        created_at: now,
    })
    .map_err(project_error_as_io)?;
    let record = match outcome {
        RegisterOutcome::Created(record) => record,
        RegisterOutcome::Existing(id) => load_project(&id).map_err(project_error_as_io)?,
    };
    project_meta_from_record(record).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "registered project has no primary Git source",
        )
    })
}

/// All registered projects, sorted by name.
#[deprecated(note = "use list_project_records")]
pub fn list_projects() -> Vec<ProjectMeta> {
    list_project_records()
        .unwrap_or_default()
        .into_iter()
        .filter_map(project_meta_from_record)
        .collect()
}

/// Unregister a project: removes `meta.toml` + `auto/` + `cache/` (AI/derived,
/// regenerable). **Preserves `notes/`** if it holds user content (charter §5 原则4).
/// Returns `true` when `notes/` was kept (so the caller can tell the user).
pub fn remove_project(hash: &str) -> bool {
    let dir = project_dir(hash);
    let notes_has_content = std::fs::read_dir(dir.join("notes"))
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);
    let _ = std::fs::remove_file(meta_path(hash));
    let _ = std::fs::remove_dir_all(dir.join("auto"));
    let _ = std::fs::remove_dir_all(dir.join("cache"));
    if notes_has_content {
        true
    } else {
        let _ = std::fs::remove_dir_all(&dir);
        false
    }
}

/// Update distill bookkeeping after a successful distill. No-op if unregistered.
/// Persists the full change cursor (`now` + `head` + newest session mtime) so the
/// next staleness check (spec §5) compares against exactly what was just distilled.
pub fn set_last_distilled(
    hash: &str,
    now: &str,
    head: Option<&str>,
    status_digest: Option<&str>,
    session_mtime: Option<f64>,
) {
    let Ok(id) = ProjectId::parse(hash) else {
        return;
    };
    if ensure_home().is_err() {
        return;
    }
    legacy_cursor_test_pause();
    let _ = with_store_txn(|| {
        let mut record = load_project(&id)?;
        record.capture_cursor.last_distilled = Some(now.to_string());
        record.capture_cursor.last_head = head.map(str::to_string);
        record.capture_cursor.last_status_digest = status_digest.map(str::to_string);
        record.capture_cursor.last_session_mtime = session_mtime;
        save_and_audit_record(&record, "project: update capture cursor")
    });
}

#[cfg(test)]
fn legacy_cursor_test_pause() {
    let pause = LEGACY_CURSOR_TEST_PAUSE
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
fn legacy_cursor_test_pause() {}

#[cfg(test)]
fn install_legacy_cursor_test_pause(
    reached: std::sync::Arc<std::sync::Barrier>,
    release: std::sync::Arc<std::sync::Barrier>,
) {
    *LEGACY_CURSOR_TEST_PAUSE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some((reached, release));
}

#[allow(deprecated)]
fn project_meta_from_record(record: ProjectRecord) -> Option<ProjectMeta> {
    let source = record.primary_git_source()?;
    Some(ProjectMeta {
        path: source.location.clone(),
        name: record.name,
        hash: record.id.to_string(),
        added_at: record.created_at,
        last_distilled: record.capture_cursor.last_distilled,
        last_head: record.capture_cursor.last_head,
        last_status_digest: record.capture_cursor.last_status_digest,
        last_session_mtime: record.capture_cursor.last_session_mtime,
        cadence: record.cadence,
    })
}

fn project_error_as_io(error: ProjectStoreError) -> io::Error {
    io::Error::other(error.to_string())
}

/// The registered project whose `path` is the longest prefix of `cwd`, so running
/// from a subdirectory resolves to its project.
pub fn find_by_cwd(cwd: &Path) -> Option<ProjectMeta> {
    best_prefix_match(&cwd.to_string_lossy(), list_projects())
}

/// Pure prefix-match core of `find_by_cwd` (separated so it's testable without IO).
fn best_prefix_match(cwd: &str, metas: Vec<ProjectMeta>) -> Option<ProjectMeta> {
    metas
        .into_iter()
        .filter(|m| cwd == m.path || cwd.starts_with(&format!("{}/", m.path)))
        .max_by_key(|m| m.path.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cleanup_cannot_delete_an_active_registration_skeleton() {
        let _guard = crate::env_guard();
        let home = std::env::temp_dir().join(format!(
            "omniproj-registration-cleanup-race-{}",
            uuid::Uuid::now_v7()
        ));
        let source = std::env::temp_dir().join(format!(
            "omniproj-registration-cleanup-source-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&source).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();

        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_registration_test_pause(reached.clone(), release.clone());
        let registration_source = source.clone();
        let registration = std::thread::spawn(move || {
            register_project(RegisterProjectInput {
                location: &registration_source,
                name: "Concurrent registration",
                created_at: "2026-08-10T12:00:00Z",
            })
        });
        reached.wait();

        let concurrent_startup = ensure_home();
        release.wait();
        let created = match registration.join().unwrap().unwrap() {
            RegisterOutcome::Created(record) => record,
            RegisterOutcome::Existing(id) => panic!("unexpected existing project {id}"),
        };

        assert!(
            concurrent_startup.is_err(),
            "startup must respect the active registration lock"
        );
        let root = project_dir_for(&created.id);
        for required in ["auto", "notes", "cache"] {
            assert!(root.join(required).is_dir(), "missing {required} directory");
        }
        assert!(root.join("meta.toml").is_file());
        assert!(root.join("notes/project.md").is_file());

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
        std::fs::remove_dir_all(source).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn registration_rejects_a_staging_project_root_symlink_before_external_writes() {
        use std::os::unix::fs::symlink;

        let _guard = crate::env_guard();
        let home = std::env::temp_dir().join(format!(
            "omniproj-registration-staging-symlink-{}",
            uuid::Uuid::now_v7()
        ));
        let source = std::env::temp_dir().join(format!(
            "omniproj-registration-staging-source-{}",
            uuid::Uuid::now_v7()
        ));
        let external = std::env::temp_dir().join(format!(
            "omniproj-registration-external-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(external.join("notes")).unwrap();
        let sentinel = external.join("sentinel.md");
        std::fs::write(&sentinel, b"Human external sentinel\n").unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();

        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_registration_test_pause(reached.clone(), release.clone());
        let registration_source = source.clone();
        let registration = std::thread::spawn(move || {
            register_project(RegisterProjectInput {
                location: &registration_source,
                name: "Symlink attack",
                created_at: "2026-08-10T12:00:00Z",
            })
        });
        reached.wait();
        let staging = std::fs::read_dir(home.join("projects"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".staging-"))
            })
            .unwrap();
        std::fs::remove_dir_all(&staging).unwrap();
        symlink(&external, &staging).unwrap();
        release.wait();

        let error = registration.join().unwrap().unwrap_err();

        assert_eq!(
            std::fs::read(&sentinel).unwrap(),
            b"Human external sentinel\n"
        );
        assert!(!external.join("notes/project.md").exists());
        assert!(!external.join("meta.toml").exists());
        assert!(matches!(
            error,
            ProjectStoreError::Store(StoreError::AuditConflict { .. })
                | ProjectStoreError::Store(StoreError::InvalidData(_))
        ));
        let staged = std::process::Command::new("git")
            .arg("-C")
            .arg(&home)
            .args(["diff", "--cached", "--name-only", "--", "projects"])
            .output()
            .unwrap();
        assert!(staged.status.success());
        assert!(staged.stdout.is_empty());

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
        std::fs::remove_dir_all(source).unwrap();
        std::fs::remove_dir_all(external).unwrap();
    }

    #[test]
    fn legacy_cursor_update_cannot_overwrite_a_concurrent_source_relink() {
        let _guard = crate::env_guard();
        let home = std::env::temp_dir().join(format!(
            "omniproj-legacy-cursor-race-{}",
            uuid::Uuid::now_v7()
        ));
        let old_source = std::env::temp_dir().join(format!(
            "omniproj-legacy-cursor-old-{}",
            uuid::Uuid::now_v7()
        ));
        let new_source = std::env::temp_dir().join(format!(
            "omniproj-legacy-cursor-new-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&old_source).unwrap();
        std::fs::create_dir_all(&new_source).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();
        let created = match register_project(RegisterProjectInput {
            location: &old_source,
            name: "Legacy cursor race",
            created_at: "2026-08-10T12:00:00Z",
        })
        .unwrap()
        {
            RegisterOutcome::Created(record) => record,
            RegisterOutcome::Existing(id) => panic!("unexpected existing project {id}"),
        };
        let original_source = created.primary_git_source().unwrap().clone();
        let human_state = project_dir_for(&created.id).join("notes/project.md");
        std::fs::write(&human_state, b"Concurrent Human state bytes\n").unwrap();

        let reached = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        install_legacy_cursor_test_pause(reached.clone(), release.clone());
        let project_id = created.id.clone();
        let cursor_update = std::thread::spawn(move || {
            set_last_distilled(
                project_id.as_str(),
                "2026-08-10T13:00:00Z",
                Some("abc123"),
                Some("clean"),
                Some(42.0),
            );
        });
        reached.wait();

        relink_primary_git_source(RelinkSourceInput {
            project_id: &created.id,
            expected_source_revision: original_source.revision,
            expected_location: &original_source.location,
            new_location: &new_source,
        })
        .unwrap();
        release.wait();
        cursor_update.join().unwrap();

        let final_record = load_project(&created.id).unwrap();
        let final_source = final_record.primary_git_source().unwrap();
        assert_eq!(final_source.revision, 1);
        assert_eq!(
            final_source.location,
            std::fs::canonicalize(&new_source)
                .unwrap()
                .to_string_lossy()
        );
        assert_eq!(
            final_record.capture_cursor.last_distilled.as_deref(),
            Some("2026-08-10T13:00:00Z")
        );
        assert_eq!(
            std::fs::read(&human_state).unwrap(),
            b"Concurrent Human state bytes\n"
        );
        let audit = std::process::Command::new("git")
            .arg("-C")
            .arg(&home)
            .args(["show", "--format=", "--name-only", "HEAD"])
            .output()
            .unwrap();
        assert!(audit.status.success());
        assert_eq!(
            String::from_utf8(audit.stdout).unwrap().trim(),
            format!("projects/{}/meta.toml", created.id)
        );

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
        std::fs::remove_dir_all(old_source).unwrap();
        std::fs::remove_dir_all(new_source).unwrap();
    }

    fn meta(path: &str) -> ProjectMeta {
        ProjectMeta {
            path: path.into(),
            name: "p".into(),
            hash: project_hash(path),
            added_at: "2026-06-04T00:00:00Z".into(),
            last_distilled: None,
            last_head: None,
            last_status_digest: None,
            last_session_mtime: None,
            cadence: None,
        }
    }

    /// A meta that has already been distilled once at `head` / `status` / `mtime`.
    fn distilled(
        path: &str,
        head: Option<&str>,
        status_digest: Option<&str>,
        mtime: Option<f64>,
    ) -> ProjectMeta {
        let mut m = meta(path);
        m.last_distilled = Some("2026-06-04T01:00:00Z".into());
        m.last_head = head.map(str::to_string);
        m.last_status_digest = status_digest.map(str::to_string);
        m.last_session_mtime = mtime;
        m
    }

    fn fp(head: Option<&str>, status_digest: Option<&str>, mtime: Option<f64>) -> Fingerprint {
        Fingerprint {
            head: head.map(str::to_string),
            status_digest: status_digest.map(str::to_string),
            latest_session_mtime: mtime,
        }
    }

    #[test]
    fn never_distilled_is_always_stale() {
        let m = meta("/p"); // last_distilled = None
        assert!(fp(None, None, None).is_stale(&m));
        assert!(fp(Some("abc"), Some("clean"), Some(10.0)).is_stale(&m));
    }

    #[test]
    fn unchanged_fingerprint_is_fresh() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(!fp(Some("abc123"), Some("clean"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn moved_head_is_stale() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("def456"), Some("clean"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn dirty_status_digest_is_stale_even_when_head_did_not_move() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("abc123"), Some("dirty"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn newer_session_is_stale_but_older_or_equal_is_not() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("abc123"), Some("clean"), Some(100.5)).is_stale(&m)); // newer
        assert!(!fp(Some("abc123"), Some("clean"), Some(100.0)).is_stale(&m)); // equal
        assert!(!fp(Some("abc123"), Some("clean"), Some(99.0)).is_stale(&m)); // older (e.g. file removed)
    }

    #[test]
    fn first_session_after_distill_is_stale() {
        let m = distilled("/p", Some("abc123"), Some("clean"), None);
        assert!(fp(Some("abc123"), Some("clean"), Some(50.0)).is_stale(&m));
    }

    #[test]
    fn toml_roundtrips() {
        let m = meta("/Users/x/git/foo");
        let text = toml::to_string_pretty(&m).unwrap();
        let back: ProjectMeta = toml::from_str(&text).unwrap();
        assert_eq!(m, back);
        assert!(!text.contains("last_distilled")); // None skipped
        assert!(!text.contains("cadence")); // None skipped
    }

    #[test]
    fn pre_cadence_meta_loads_without_the_field() {
        // A meta.toml written by an older OmniProj has no `[cadence]` — it must still
        // parse (additive optional field, no schema bump).
        let text = r#"
path = "/Users/x/git/foo"
name = "foo"
hash = "deadbeefdeadbeef"
added_at = "2026-06-04T00:00:00Z"
"#;
        let m: ProjectMeta = toml::from_str(text).unwrap();
        assert_eq!(m.name, "foo");
        assert!(m.cadence.is_none());
    }

    #[test]
    fn cadence_roundtrips_and_partial_override_is_allowed() {
        let mut m = meta("/p");
        m.cadence = Some(Cadence {
            refresh_floor_secs: Some(3600),
            depth: None, // only one knob set
        });
        let text = toml::to_string_pretty(&m).unwrap();
        let back: ProjectMeta = toml::from_str(&text).unwrap();
        assert_eq!(m, back);
        let c = back.cadence.unwrap();
        assert_eq!(c.refresh_floor_secs, Some(3600));
        assert!(c.depth.is_none());
        assert!(!c.is_empty());
    }

    #[test]
    fn cwd_resolves_to_longest_prefix() {
        let metas = vec![meta("/Users/x/git"), meta("/Users/x/git/foo")];
        let hit = best_prefix_match("/Users/x/git/foo/sub", metas).unwrap();
        assert_eq!(hit.path, "/Users/x/git/foo"); // longest prefix wins, not the shorter parent
    }

    #[test]
    fn cwd_no_match_is_none() {
        let metas = vec![meta("/Users/x/git/foo")];
        assert!(best_prefix_match("/Users/y/other", metas).is_none());
        // a sibling sharing a string prefix but not a path boundary must not match
        assert!(best_prefix_match("/Users/x/git/foobar", vec![meta("/Users/x/git/foo")]).is_none());
    }
}
