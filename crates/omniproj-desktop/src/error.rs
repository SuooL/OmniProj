//! The fixed serialized error contract for the R0 IPC boundary.
//!
//! Every recoverable failure the desktop surface can return is one `CommandError`
//! with a stable `code` string plus a small set of fields the UI reasons about
//! directly. The two facts the UI must never confuse are carried explicitly:
//!
//! - `state_applied` — did the Human mutation reach durable storage? Only
//!   `audit_commit_failed` returns `true`: the revision is durable and the UI must
//!   refetch it and NOT resend the mutation. Everything else is `false`.
//! - `retryable` — is a verbatim retry safe and potentially useful (e.g. a transient
//!   `store_write_failed`)? A `revision_conflict` is not retryable verbatim: the UI
//!   must refetch and rebuild the request.
//!
//! Raw error chains / stack traces are never serialized — only the curated `message`.

use serde::Serialize;

use omniproj_capture::git::{RepositoryReadError, RepositoryReadErrorKind};
use omniproj_core::project::ProjectStoreError;
use omniproj_core::project_state::ProjectStateError;
use omniproj_core::store::StoreError;

/// The closed set of R0 error codes. Serialized snake_case; this is wire contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ProjectNotFound,
    InvalidInput,
    InvalidPath,
    SourceMissing,
    SourceUnreadable,
    NotGitRepository,
    BareRepository,
    DuplicateSource,
    SourceObservationFailed,
    StoreReadFailed,
    StoreWriteFailed,
    AuditCommitFailed,
    RevisionConflict,
    CurrentCommitmentExists,
    NoCurrentCommitment,
    CurrentCommitmentChanged,
    ReasonRequired,
    TransitionNotFound,
    UndoNotAvailable,
    UndoConflict,
}

/// A single serialized IPC error. Fixed shape: `code` + `message` + `retryable` +
/// `state_applied`, with a few optional discriminators. Optional fields are omitted
/// when absent so the wire form stays minimal and deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    pub state_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable_revision: Option<u64>,
}

/// Result alias for every command / service method.
pub type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    /// A plain error: not retryable, no durable state applied, no discriminators.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            state_applied: false,
            field: None,
            project_id: None,
            existing_project_id: None,
            durable_revision: None,
        }
    }

    /// Mark this error as safe to retry verbatim.
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    /// Attach the offending input field name (for `invalid_input` / validation errors).
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Attach the project id this error concerns.
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// `invalid_input` shorthand.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    /// The one error whose Human state DID reach durable storage: the revision is
    /// saved but its audit commit failed. The UI refetches `durable_revision` and must
    /// not resend the mutation.
    pub fn audit_commit_failed(durable_revision: u64) -> Self {
        Self {
            code: ErrorCode::AuditCommitFailed,
            message: format!(
                "the change was saved as revision {durable_revision} but its audit commit failed; \
                 reload the project"
            ),
            retryable: false,
            state_applied: true,
            field: None,
            project_id: None,
            existing_project_id: None,
            durable_revision: Some(durable_revision),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<RepositoryReadError> for CommandError {
    fn from(error: RepositoryReadError) -> Self {
        let code = match error.kind {
            RepositoryReadErrorKind::PathMissing => ErrorCode::SourceMissing,
            RepositoryReadErrorKind::PermissionDenied => ErrorCode::SourceUnreadable,
            RepositoryReadErrorKind::NotRepository => ErrorCode::NotGitRepository,
            RepositoryReadErrorKind::BareRepository => ErrorCode::BareRepository,
            RepositoryReadErrorKind::GitUnavailable
            | RepositoryReadErrorKind::CommandFailed
            | RepositoryReadErrorKind::InvalidOutput => ErrorCode::SourceObservationFailed,
        };
        let error = Self::new(code, error.message);
        match code {
            // Transient/environmental failures are worth another attempt.
            ErrorCode::SourceObservationFailed => error.retryable(),
            _ => error,
        }
    }
}

impl From<StoreError> for CommandError {
    fn from(error: StoreError) -> Self {
        match error {
            // A store-level audit commit failed: the durable bytes were already written.
            // (The Human-mutation path uses ProjectStateError::AuditCommitFailed, which
            // additionally carries the durable revision.)
            StoreError::AuditCommit(message) => {
                let mut error = Self::new(ErrorCode::AuditCommitFailed, message);
                error.state_applied = true;
                error
            }
            StoreError::InvalidData(message) => Self::new(ErrorCode::StoreReadFailed, message),
            other => Self::new(ErrorCode::StoreWriteFailed, other.to_string()).retryable(),
        }
    }
}

impl From<ProjectStoreError> for CommandError {
    fn from(error: ProjectStoreError) -> Self {
        match error {
            ProjectStoreError::NotFound(project_id) => Self::new(
                ErrorCode::ProjectNotFound,
                format!("project {project_id} was not found"),
            )
            .with_project_id(project_id.as_str()),
            ProjectStoreError::SourceNotFound {
                project_id,
                source_id,
            } => Self::new(
                ErrorCode::ProjectNotFound,
                format!("source {source_id} of project {project_id} was not found"),
            )
            .with_project_id(project_id.as_str()),
            ProjectStoreError::InvalidPath { path, message } => Self::new(
                ErrorCode::InvalidPath,
                format!("{}: {message}", path.display()),
            ),
            ProjectStoreError::InvalidInput(message) => Self::invalid_input(message),
            ProjectStoreError::DuplicateSource {
                existing_project_id,
            } => {
                let mut error = Self::new(
                    ErrorCode::DuplicateSource,
                    format!("that location already belongs to project {existing_project_id}"),
                );
                error.existing_project_id = Some(existing_project_id.as_str().to_owned());
                error
            }
            ProjectStoreError::RevisionConflict { expected, actual } => Self::new(
                ErrorCode::RevisionConflict,
                format!("revision conflict: expected {expected}, found {actual}"),
            ),
            ProjectStoreError::LocationConflict { expected, actual } => Self::new(
                ErrorCode::RevisionConflict,
                format!("source location changed: expected {expected}, found {actual}"),
            ),
            ProjectStoreError::InvalidRecord { path, message } => Self::new(
                ErrorCode::StoreReadFailed,
                format!("invalid stored record at {}: {message}", path.display()),
            ),
            ProjectStoreError::Io(error) => {
                Self::new(ErrorCode::StoreReadFailed, error.to_string()).retryable()
            }
            ProjectStoreError::Store(error) => Self::from(error),
        }
    }
}

impl From<ProjectStateError> for CommandError {
    fn from(error: ProjectStateError) -> Self {
        match error {
            ProjectStateError::NotFound(path) => Self::new(
                ErrorCode::ProjectNotFound,
                format!("project state not found: {}", path.display()),
            ),
            ProjectStateError::Io(error) => {
                Self::new(ErrorCode::StoreReadFailed, error.to_string()).retryable()
            }
            ProjectStateError::InvalidDocument(message) => Self::new(
                ErrorCode::StoreReadFailed,
                format!("invalid project state: {message}"),
            ),
            ProjectStateError::UnsupportedSchema(version) => Self::new(
                ErrorCode::StoreReadFailed,
                format!("unsupported project state schema version {version}"),
            ),
            ProjectStateError::RevisionConflict { expected, actual } => Self::new(
                ErrorCode::RevisionConflict,
                format!("revision conflict: expected {expected}, found {actual}"),
            ),
            ProjectStateError::FieldRequired { field } => {
                Self::invalid_input(format!("{field} is required")).with_field(field)
            }
            ProjectStateError::ReasonRequired => {
                Self::new(ErrorCode::ReasonRequired, "a nonempty reason is required")
            }
            ProjectStateError::InvalidTimestamp { field, value } => {
                Self::invalid_input(format!("{field} must be RFC3339, got {value:?}"))
                    .with_field(field)
            }
            ProjectStateError::CurrentCommitmentExists { work_item_id } => Self::new(
                ErrorCode::CurrentCommitmentExists,
                format!("a current commitment ({work_item_id}) already exists"),
            ),
            ProjectStateError::CurrentCommitmentMismatch { expected, actual } => match actual {
                None => Self::new(
                    ErrorCode::NoCurrentCommitment,
                    format!("there is no current commitment (expected {expected})"),
                ),
                Some(actual) => Self::new(
                    ErrorCode::CurrentCommitmentChanged,
                    format!("the current commitment changed: expected {expected}, found {actual}"),
                ),
            },
            ProjectStateError::WorkItemNotFound(work_item_id) => {
                Self::invalid_input(format!("work item {work_item_id} was not found"))
            }
            ProjectStateError::UndoConflict { transition_id } => Self::new(
                ErrorCode::UndoConflict,
                format!("transition {transition_id} is not the newest undoable transition"),
            ),
            ProjectStateError::InvalidCommand(message) => Self::invalid_input(message),
            ProjectStateError::AuditCommitFailed {
                durable_revision, ..
            } => Self::audit_commit_failed(durable_revision),
            ProjectStateError::Store(error) => Self::from(error),
        }
    }
}
