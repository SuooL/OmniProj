use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::ids::{CommitmentTransitionId, ProjectId, WorkItemId};
use crate::paths::{notes_dir_for, omniproj_home};
use crate::store::{
    atomic_write_store, audit_target_snapshot, begin_pending_audit, ensure_home_then_write,
    finish_pending_audit, mark_pending_audit_applied, StoreError,
};

/// v2 (R1) adds optional `tags` to work items. v1 documents load unchanged (tags default
/// to empty) and are upgraded in memory; every save writes the current version.
const DOCUMENT_SCHEMA_VERSION: u32 = 2;
const MIN_DOCUMENT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_MARKDOWN_BODY: &str = "\n# Project notes\n";

/// Tag invariants (FR-R5): a pure classification dimension, deliberately small.
const MAX_TAGS_PER_ITEM: usize = 8;
const MAX_TAG_CHARS: usize = 24;

#[derive(Debug)]
pub enum ProjectStateError {
    NotFound(PathBuf),
    Io(std::io::Error),
    InvalidDocument(String),
    UnsupportedSchema(u32),
    RevisionConflict {
        expected: u64,
        actual: u64,
    },
    FieldRequired {
        field: &'static str,
    },
    ReasonRequired,
    InvalidTimestamp {
        field: &'static str,
        value: String,
    },
    CurrentCommitmentExists {
        work_item_id: WorkItemId,
    },
    CurrentCommitmentMismatch {
        expected: WorkItemId,
        actual: Option<WorkItemId>,
    },
    WorkItemNotFound(WorkItemId),
    UndoConflict {
        transition_id: CommitmentTransitionId,
    },
    InvalidCommand(String),
    AuditCommitFailed {
        durable_revision: u64,
        source: StoreError,
    },
    Store(StoreError),
}

impl fmt::Display for ProjectStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "project state not found: {}", path.display()),
            Self::Io(error) => error.fmt(f),
            Self::InvalidDocument(message) => write!(f, "invalid project state: {message}"),
            Self::UnsupportedSchema(version) => {
                write!(f, "unsupported project state schema version {version}")
            }
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "project revision conflict: expected {expected}, found {actual}"
            ),
            Self::FieldRequired { field } => write!(f, "{field} is required"),
            Self::ReasonRequired => f.write_str("a nonempty reason is required"),
            Self::InvalidTimestamp { field, value } => {
                write!(f, "{field} must be RFC3339 and monotonic, got {value:?}")
            }
            Self::CurrentCommitmentExists { work_item_id } => {
                write!(f, "current commitment {work_item_id} already exists")
            }
            Self::CurrentCommitmentMismatch { expected, actual } => write!(
                f,
                "current commitment mismatch: expected {expected}, found {actual:?}"
            ),
            Self::WorkItemNotFound(work_item_id) => {
                write!(f, "work item {work_item_id} was not found")
            }
            Self::UndoConflict { transition_id } => write!(
                f,
                "transition {transition_id} is not the newest undoable transition"
            ),
            Self::InvalidCommand(message) => write!(f, "invalid project command: {message}"),
            Self::AuditCommitFailed {
                durable_revision,
                source,
            } => write!(
                f,
                "project revision {durable_revision} was saved but its audit commit failed: {source}"
            ),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProjectStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::AuditCommitFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProjectStateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StoreError> for ProjectStateError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Setup,
    Active,
    Waiting,
    Parked,
    Archived,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    Planned,
    Doing,
    Blocked,
    Done,
    Abandoned,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentTransitionKind {
    Set,
    Confirmed,
    Completed,
    Replaced,
    Cleared,
    Correction,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub project_id: ProjectId,
    pub text: String,
    pub status: WorkItemStatus,
    #[serde(default)]
    pub unclear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// User classification labels (schema v2). Normalized on write: trimmed, non-empty,
    /// case-insensitively unique, at most `MAX_TAGS_PER_ITEM` of `MAX_TAG_CHARS` chars.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopted_from_proposal_id: Option<String>,
    /// Optional stable id of the Human planning task that was explicitly promoted to
    /// this project-level commitment. This is provenance, not a second pointer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<String>,
}

/// Input used by one-time legacy import and atomic Agent adoption. It deliberately contains
/// only Human work fields; commitment history remains a separate append-only concern.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkItemDraft {
    pub text: String,
    pub status: WorkItemStatus,
    pub unclear: bool,
    pub due: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub commits: Vec<String>,
    pub adopted_from_proposal_id: Option<String>,
    pub source_task_id: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentTransition {
    pub id: CommitmentTransitionId,
    pub project_id: ProjectId,
    pub document_revision: u64,
    #[serde(rename = "type")]
    pub kind: CommitmentTransitionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_work_item_id: Option<WorkItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_work_item_id: Option<WorkItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrects_transition_id: Option<CommitmentTransitionId>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectStateDoc {
    pub schema_version: u32,
    pub revision: u64,
    pub status: ProjectStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    pub status_changed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_next_action_id: Option<WorkItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub work_items: Vec<WorkItem>,
    pub commitment_transitions: Vec<CommitmentTransition>,
    #[serde(skip)]
    markdown_body: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProjectMutation {
    pub revision: u64,
    pub state: ProjectStateDoc,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProjectCommand {
    SaveFraming {
        objective: String,
        desired_outcome: String,
        phase: Option<String>,
    },
    CompleteSetup {
        objective: String,
        desired_outcome: String,
        phase: Option<String>,
        first_commitment: String,
    },
    SetCommitment {
        text: String,
    },
    SetCommitmentFromTask {
        text: String,
        source_task_id: String,
        adopted_from_proposal_id: Option<String>,
    },
    AddWorkItems {
        items: Vec<WorkItemDraft>,
    },
    UpdateWorkItem {
        work_item_id: WorkItemId,
        status: WorkItemStatus,
        unclear: bool,
        due: Option<String>,
        note: Option<String>,
        /// `None` leaves the stored tags unchanged; `Some` replaces them (normalized).
        tags: Option<Vec<String>>,
    },
    RemoveWorkItem {
        work_item_id: WorkItemId,
    },
    SetCommitmentFromWorkItem {
        work_item_id: WorkItemId,
    },
    AttributeCommit {
        work_item_id: WorkItemId,
        sha: String,
        attributed: bool,
    },
    ImportLegacyWorkItems {
        items: Vec<WorkItemDraft>,
    },
    ConfirmCommitment {
        work_item_id: WorkItemId,
    },
    CompleteCommitment {
        work_item_id: WorkItemId,
    },
    ReplaceCommitment {
        previous_work_item_id: WorkItemId,
        text: String,
        reason: String,
    },
    ClearCommitment {
        work_item_id: WorkItemId,
        reason: Option<String>,
    },
    SetStatus {
        status: ProjectStatus,
        reason: Option<String>,
        review_at: Option<String>,
    },
    Undo {
        transition_id: CommitmentTransitionId,
    },
}

impl ProjectStateDoc {
    pub fn new_setup(created_at: &str) -> Result<Self, ProjectStateError> {
        validate_timestamp("created_at", created_at)?;
        Ok(Self {
            schema_version: DOCUMENT_SCHEMA_VERSION,
            revision: 0,
            status: ProjectStatus::Setup,
            status_reason: None,
            status_changed_at: created_at.to_owned(),
            objective: None,
            desired_outcome: None,
            phase: None,
            current_next_action_id: None,
            review_at: None,
            created_at: created_at.to_owned(),
            updated_at: created_at.to_owned(),
            work_items: Vec::new(),
            commitment_transitions: Vec::new(),
            markdown_body: DEFAULT_MARKDOWN_BODY.to_owned(),
        })
    }

    pub fn parse(input: &str) -> Result<Self, ProjectStateError> {
        let front_matter = input.strip_prefix("+++\n").ok_or_else(|| {
            ProjectStateError::InvalidDocument("missing opening +++ delimiter".into())
        })?;
        let delimiter = "\n+++\n";
        let close = front_matter.find(delimiter).ok_or_else(|| {
            ProjectStateError::InvalidDocument("missing closing +++ delimiter".into())
        })?;
        let toml_text = &front_matter[..close];
        let markdown_body = &front_matter[close + delimiter.len()..];
        // Check the version BEFORE the typed (deny_unknown_fields) deserialization, so a
        // document from a newer OmniProj is refused with a clear version error instead of
        // an incidental unknown-field parse error.
        let probe: toml::Value = toml::from_str(toml_text)
            .map_err(|error| ProjectStateError::InvalidDocument(error.to_string()))?;
        let version = probe
            .get("schema_version")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| {
                ProjectStateError::InvalidDocument("missing or non-integer schema_version".into())
            })?;
        let version =
            u32::try_from(version).map_err(|_| ProjectStateError::UnsupportedSchema(u32::MAX))?;
        if !(MIN_DOCUMENT_SCHEMA_VERSION..=DOCUMENT_SCHEMA_VERSION).contains(&version) {
            return Err(ProjectStateError::UnsupportedSchema(version));
        }
        let mut document: Self = toml::from_str(toml_text)
            .map_err(|error| ProjectStateError::InvalidDocument(error.to_string()))?;
        // A v1 document is upgraded in memory (v2 only adds defaulted fields); the next
        // save persists the current version.
        document.schema_version = DOCUMENT_SCHEMA_VERSION;
        document.markdown_body = markdown_body.to_owned();
        document.validate()?;
        Ok(document)
    }

    pub fn render(&self) -> Result<String, ProjectStateError> {
        self.validate()?;
        let front_matter = toml::to_string(self)
            .map_err(|error| ProjectStateError::InvalidDocument(error.to_string()))?;
        Ok(format!("+++\n{front_matter}+++\n{}", self.markdown_body))
    }

    pub fn load(project_id: &ProjectId) -> Result<Self, ProjectStateError> {
        let path = state_path(project_id);
        let input = match std::fs::read_to_string(&path) {
            Ok(input) => input,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ProjectStateError::NotFound(path));
            }
            Err(error) => return Err(ProjectStateError::Io(error)),
        };
        let document = Self::parse(&input)?;
        document.validate_for_project(project_id)?;
        Ok(document)
    }

    pub fn save(&self, project_id: &ProjectId) -> Result<(), ProjectStateError> {
        self.validate_for_project(project_id)?;
        let home = crate::paths::omniproj_home();
        std::fs::create_dir_all(&home).map_err(ProjectStateError::Io)?;
        self.save_to_store_path(&home, &state_path(project_id))
    }

    pub fn markdown_body(&self) -> &str {
        &self.markdown_body
    }

    pub(crate) fn save_to_store_path(
        &self,
        home: &Path,
        path: &Path,
    ) -> Result<(), ProjectStateError> {
        atomic_write_store(home, path, self.render()?.as_bytes()).map_err(ProjectStateError::Store)
    }

    fn validate(&self) -> Result<(), ProjectStateError> {
        if self.schema_version != DOCUMENT_SCHEMA_VERSION {
            return Err(ProjectStateError::UnsupportedSchema(self.schema_version));
        }
        validate_timestamp("status_changed_at", &self.status_changed_at)?;
        validate_timestamp("created_at", &self.created_at)?;
        validate_timestamp("updated_at", &self.updated_at)?;
        validate_optional_timestamp("review_at", self.review_at.as_deref())?;
        validate_ordered_timestamp(
            "created_at",
            &self.created_at,
            "updated_at",
            &self.updated_at,
        )?;
        validate_ordered_timestamp(
            "created_at",
            &self.created_at,
            "status_changed_at",
            &self.status_changed_at,
        )?;
        validate_ordered_timestamp(
            "status_changed_at",
            &self.status_changed_at,
            "updated_at",
            &self.updated_at,
        )?;
        match self.status {
            ProjectStatus::Waiting => {
                validate_nonempty_option("status_reason", self.status_reason.as_deref())?;
                if self.review_at.is_none() {
                    return invalid("review_at is required for waiting status".into());
                }
            }
            ProjectStatus::Parked => {
                validate_nonempty_option("status_reason", self.status_reason.as_deref())?;
            }
            _ => {}
        }

        let mut aggregate_project_id: Option<&ProjectId> = None;
        let mut work_item_ids = HashSet::new();
        for item in &self.work_items {
            if !work_item_ids.insert(item.id.clone()) {
                return invalid(format!("duplicate work item id {}", item.id));
            }
            if item.text.trim().is_empty() {
                return invalid(format!("work item {} has empty text", item.id));
            }
            if let Some(due) = item.due.as_deref() {
                NaiveDate::parse_from_str(due, "%Y-%m-%d").map_err(|_| {
                    ProjectStateError::InvalidDocument(format!(
                        "work item {} has invalid due date",
                        item.id
                    ))
                })?;
            }
            let mut commit_ids = HashSet::new();
            if item
                .commits
                .iter()
                .any(|sha| sha.trim().is_empty() || !commit_ids.insert(sha))
            {
                return invalid(format!("work item {} has invalid commits", item.id));
            }
            if let Err(error) = validate_tags(&item.tags) {
                return invalid(format!("work item {} has invalid tags: {error}", item.id));
            }
            validate_timestamp("work item created_at", &item.created_at)?;
            validate_timestamp("work item updated_at", &item.updated_at)?;
            validate_optional_timestamp("work item blocked_at", item.blocked_at.as_deref())?;
            validate_ordered_timestamp(
                "project created_at",
                &self.created_at,
                "work item created_at",
                &item.created_at,
            )?;
            validate_ordered_timestamp(
                "work item created_at",
                &item.created_at,
                "work item updated_at",
                &item.updated_at,
            )?;
            validate_ordered_timestamp(
                "work item updated_at",
                &item.updated_at,
                "project updated_at",
                &self.updated_at,
            )?;
            if let Some(blocked_at) = item.blocked_at.as_deref() {
                validate_ordered_timestamp(
                    "work item created_at",
                    &item.created_at,
                    "work item blocked_at",
                    blocked_at,
                )?;
                validate_ordered_timestamp(
                    "work item blocked_at",
                    blocked_at,
                    "work item updated_at",
                    &item.updated_at,
                )?;
            }
            validate_aggregate_project_id(&mut aggregate_project_id, &item.project_id)?;
        }
        if let Some(current) = &self.current_next_action_id {
            if !work_item_ids.contains(current) {
                return invalid(format!("current next action {current} does not exist"));
            }
        }

        let mut transition_ids = HashSet::new();
        let mut transitions_by_id: HashMap<CommitmentTransitionId, &CommitmentTransition> =
            HashMap::new();
        let mut corrected_ids = HashSet::new();
        let mut previous_occurred_at: Option<&str> = None;
        let mut previous_document_revision = 0;
        for (index, transition) in self.commitment_transitions.iter().enumerate() {
            if !transition_ids.insert(transition.id.clone()) {
                return invalid(format!("duplicate transition id {}", transition.id));
            }
            validate_timestamp("transition occurred_at", &transition.occurred_at)?;
            validate_ordered_timestamp(
                "project created_at",
                &self.created_at,
                "transition occurred_at",
                &transition.occurred_at,
            )?;
            validate_ordered_timestamp(
                "transition occurred_at",
                &transition.occurred_at,
                "project updated_at",
                &self.updated_at,
            )?;
            validate_aggregate_project_id(&mut aggregate_project_id, &transition.project_id)?;
            if transition.document_revision == 0
                || transition.document_revision <= previous_document_revision
                || transition.document_revision > self.revision
            {
                return invalid(format!(
                    "transition {} has invalid document revision {} for project revision {}",
                    transition.id, transition.document_revision, self.revision
                ));
            }
            previous_document_revision = transition.document_revision;
            if let Some(previous) = previous_occurred_at {
                validate_ordered_timestamp(
                    "previous transition occurred_at",
                    previous,
                    "transition occurred_at",
                    &transition.occurred_at,
                )?;
            }
            previous_occurred_at = Some(&transition.occurred_at);
            for referenced in [
                transition.previous_work_item_id.as_ref(),
                transition.next_work_item_id.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if !work_item_ids.contains(referenced) {
                    return invalid(format!(
                        "transition references missing work item {referenced}"
                    ));
                }
            }
            validate_transition_static_shape(transition)?;

            match (&transition.kind, &transition.corrects_transition_id) {
                (CommitmentTransitionKind::Correction, Some(corrected)) => {
                    let target = transitions_by_id.get(corrected).ok_or_else(|| {
                        ProjectStateError::InvalidDocument(format!(
                            "correction references missing or later transition {corrected}"
                        ))
                    })?;
                    if target.kind == CommitmentTransitionKind::Correction {
                        return invalid(format!(
                            "correction cannot target correction transition {corrected}"
                        ));
                    }
                    let previous = index
                        .checked_sub(1)
                        .and_then(|index| self.commitment_transitions.get(index));
                    if previous.map(|transition| &transition.id) != Some(corrected) {
                        return invalid(format!(
                            "correction {} must immediately follow transition {corrected}",
                            transition.id
                        ));
                    }
                    if transition.document_revision != target.document_revision + 1 {
                        return invalid(format!(
                            "correction {} revision must immediately follow target revision",
                            transition.id
                        ));
                    }
                    if !corrected_ids.insert(corrected.clone()) {
                        return invalid(format!(
                            "transition {corrected} has already been corrected"
                        ));
                    }
                }
                (CommitmentTransitionKind::Correction, None) => {
                    return invalid(
                        "correction transition is missing corrects_transition_id".into(),
                    );
                }
                (_, Some(_)) => {
                    return invalid(
                        "only correction transitions may have corrects_transition_id".into(),
                    );
                }
                (_, None) => {}
            }
            transitions_by_id.insert(transition.id.clone(), transition);
        }

        let replayed = replay_and_validate_commitment_history(&self.commitment_transitions)?;
        if replayed.current_next_action_id != self.current_next_action_id {
            return invalid(format!(
                "stored current next action {:?} differs from replayed history {:?}",
                self.current_next_action_id, replayed.current_next_action_id
            ));
        }
        // Only the item the commitment points at right now has a status the log fixes.
        // Items it merely touched in the past are ordinary user state again.
        if let Some(work_item_id) = &replayed.current_next_action_id {
            let item = self
                .work_items
                .iter()
                .find(|item| &item.id == work_item_id)
                .ok_or_else(|| {
                    ProjectStateError::InvalidDocument(format!(
                        "current commitment references missing work item {work_item_id}"
                    ))
                })?;
            if item.status != WorkItemStatus::Doing {
                return invalid(format!(
                    "current commitment {work_item_id} has status {:?}, expected Doing",
                    item.status
                ));
            }
        }
        Ok(())
    }

    fn validate_for_project(&self, project_id: &ProjectId) -> Result<(), ProjectStateError> {
        for item in &self.work_items {
            if &item.project_id != project_id {
                return invalid(format!(
                    "work item {} belongs to project {}, expected {project_id}",
                    item.id, item.project_id
                ));
            }
        }
        for transition in &self.commitment_transitions {
            if &transition.project_id != project_id {
                return invalid(format!(
                    "transition {} belongs to project {}, expected {project_id}",
                    transition.id, transition.project_id
                ));
            }
        }
        Ok(())
    }
}

pub fn apply_project_command(
    project_id: &ProjectId,
    expected_revision: u64,
    command: ProjectCommand,
    occurred_at: &str,
) -> Result<ProjectMutation, ProjectStateError> {
    validate_command_timestamp("occurred_at", occurred_at)?;
    ensure_home_then_write(|| {
        let home = omniproj_home();
        let path = state_path(project_id);
        let prior_state = ProjectStateDoc::load(project_id)?;
        prior_state.validate_for_project(project_id)?;
        if prior_state.revision != expected_revision {
            return Err(ProjectStateError::RevisionConflict {
                expected: expected_revision,
                actual: prior_state.revision,
            });
        }
        ensure_not_before("occurred_at", occurred_at, &prior_state.updated_at)?;

        let accepted_revision = prior_state
            .revision
            .checked_add(1)
            .ok_or_else(|| ProjectStateError::InvalidCommand("revision overflow".into()))?;
        let mut state = prior_state.clone();
        apply_command_in_memory(
            &mut state,
            project_id,
            command,
            occurred_at,
            accepted_revision,
        )?;
        state.updated_at = occurred_at.to_owned();
        state.revision = accepted_revision;
        state.validate()?;
        state.validate_for_project(project_id)?;

        let rendered = state.render()?;
        let relative_path =
            PathBuf::from(format!("projects/{}/notes/project.md", project_id.as_str()));
        let targets = [audit_target_snapshot(
            &home,
            relative_path,
            rendered.as_bytes(),
        )?];
        begin_pending_audit(&home, "project: update human state", &targets)?;
        atomic_write_store(&home, &path, rendered.as_bytes())?;
        if let Err(source) = mark_pending_audit_applied(&home) {
            return Err(ProjectStateError::AuditCommitFailed {
                durable_revision: state.revision,
                source,
            });
        }
        if let Err(source) = finish_pending_audit(&home) {
            return Err(ProjectStateError::AuditCommitFailed {
                durable_revision: state.revision,
                source,
            });
        }

        Ok(ProjectMutation {
            revision: state.revision,
            state,
        })
    })
}

fn apply_command_in_memory(
    state: &mut ProjectStateDoc,
    project_id: &ProjectId,
    command: ProjectCommand,
    occurred_at: &str,
    accepted_revision: u64,
) -> Result<(), ProjectStateError> {
    match command {
        ProjectCommand::SaveFraming {
            objective,
            desired_outcome,
            phase,
        } => {
            require_field("objective", &objective)?;
            require_field("desired_outcome", &desired_outcome)?;
            state.objective = Some(objective);
            state.desired_outcome = Some(desired_outcome);
            state.phase = normalize_optional(phase);
        }
        ProjectCommand::CompleteSetup {
            objective,
            desired_outcome,
            phase,
            first_commitment,
        } => {
            require_field("objective", &objective)?;
            require_field("desired_outcome", &desired_outcome)?;
            require_field("first_commitment", &first_commitment)?;
            if state.status != ProjectStatus::Setup {
                return Err(ProjectStateError::InvalidCommand(
                    "setup has already been completed".into(),
                ));
            }
            if let Some(work_item_id) = state.current_next_action_id.clone() {
                return Err(ProjectStateError::CurrentCommitmentExists { work_item_id });
            }
            state.objective = Some(objective);
            state.desired_outcome = Some(desired_outcome);
            state.phase = normalize_optional(phase);
            state.status = ProjectStatus::Active;
            state.status_reason = None;
            state.review_at = None;
            state.status_changed_at = occurred_at.to_owned();
            set_commitment_in_memory(
                state,
                project_id,
                first_commitment,
                occurred_at,
                accepted_revision,
            )?;
        }
        ProjectCommand::SetCommitment { text } => {
            require_field("text", &text)?;
            if let Some(work_item_id) = state.current_next_action_id.clone() {
                return Err(ProjectStateError::CurrentCommitmentExists { work_item_id });
            }
            set_commitment_in_memory(state, project_id, text, occurred_at, accepted_revision)?;
        }
        ProjectCommand::SetCommitmentFromTask {
            text,
            source_task_id,
            adopted_from_proposal_id,
        } => {
            require_field("text", &text)?;
            require_field("source_task_id", &source_task_id)?;
            if let Some(work_item_id) = state.current_next_action_id.clone() {
                return Err(ProjectStateError::CurrentCommitmentExists { work_item_id });
            }
            let mut item = new_work_item(project_id, text, occurred_at);
            item.source_task_id = Some(source_task_id);
            item.adopted_from_proposal_id = adopted_from_proposal_id;
            let item_id = item.id.clone();
            state.work_items.push(item);
            state.current_next_action_id = Some(item_id.clone());
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Set,
                None,
                Some(item_id),
                None,
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::AddWorkItems { items } => {
            if items.is_empty() {
                return Err(ProjectStateError::InvalidCommand(
                    "at least one work item is required".into(),
                ));
            }
            for draft in items {
                validate_work_item_draft(&draft)?;
                state
                    .work_items
                    .push(work_item_from_draft(project_id, draft, occurred_at));
            }
        }
        ProjectCommand::UpdateWorkItem {
            work_item_id,
            status,
            unclear,
            due,
            note,
            tags,
        } => {
            validate_due(due.as_deref())?;
            let tags = tags.map(normalize_tags).transpose()?;
            // Only the item the commitment points at right now has a status owned by the
            // commitment actions. A past commitment is the user's to reopen or re-plan.
            let is_current = state.current_next_action_id.as_ref() == Some(&work_item_id);
            let item = require_item_mut(state, &work_item_id)?;
            if is_current && item.status != status {
                return Err(ProjectStateError::InvalidCommand(
                    "commitment lifecycle status must be changed through commitment actions".into(),
                ));
            }
            item.status = status;
            item.unclear = unclear;
            item.due = normalize_optional(due);
            item.note = normalize_optional(note);
            if let Some(tags) = tags {
                item.tags = tags;
            }
            item.updated_at = occurred_at.to_owned();
        }
        ProjectCommand::RemoveWorkItem { work_item_id } => {
            if state.current_next_action_id.as_ref() == Some(&work_item_id) {
                return Err(ProjectStateError::InvalidCommand(
                    "the current commitment cannot be removed".into(),
                ));
            }
            // The audit log must keep its subject: every transition names a work item that
            // has to still exist. An item the log mentions is therefore tombstoned rather
            // than deleted — `abandoned` items are filtered out of the task list, so it
            // leaves the user's list either way, and the history stays readable.
            if work_item_is_referenced(state, &work_item_id) {
                let item = require_item_mut(state, &work_item_id)?;
                item.status = WorkItemStatus::Abandoned;
                item.updated_at = occurred_at.to_owned();
                return Ok(());
            }
            let before = state.work_items.len();
            state.work_items.retain(|item| item.id != work_item_id);
            if state.work_items.len() == before {
                return Err(ProjectStateError::WorkItemNotFound(work_item_id));
            }
        }
        ProjectCommand::SetCommitmentFromWorkItem { work_item_id } => {
            if let Some(current) = state.current_next_action_id.clone() {
                return Err(ProjectStateError::CurrentCommitmentExists {
                    work_item_id: current,
                });
            }
            if work_item_is_referenced(state, &work_item_id) {
                return Err(ProjectStateError::InvalidCommand(
                    "a historical commitment cannot be promoted again".into(),
                ));
            }
            let item = require_item_mut(state, &work_item_id)?;
            if matches!(
                item.status,
                WorkItemStatus::Done | WorkItemStatus::Abandoned
            ) {
                return Err(ProjectStateError::InvalidCommand(
                    "a closed work item cannot become the current commitment".into(),
                ));
            }
            item.status = WorkItemStatus::Doing;
            item.updated_at = occurred_at.to_owned();
            state.current_next_action_id = Some(work_item_id.clone());
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Set,
                None,
                Some(work_item_id),
                None,
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::AttributeCommit {
            work_item_id,
            sha,
            attributed,
        } => {
            require_field("sha", &sha)?;
            let item = require_item_mut(state, &work_item_id)?;
            if attributed {
                if !item.commits.iter().any(|existing| existing == &sha) {
                    item.commits.push(sha);
                }
            } else {
                item.commits.retain(|existing| existing != &sha);
            }
            item.updated_at = occurred_at.to_owned();
        }
        ProjectCommand::ImportLegacyWorkItems { items } => {
            for draft in items {
                validate_work_item_draft(&draft)?;
                let existing = draft.source_task_id.as_deref().and_then(|legacy_id| {
                    state
                        .work_items
                        .iter()
                        .position(|item| item.source_task_id.as_deref() == Some(legacy_id))
                });
                if let Some(index) = existing {
                    let is_current =
                        state.current_next_action_id.as_ref() == Some(&state.work_items[index].id);
                    let item = &mut state.work_items[index];
                    item.unclear = draft.unclear;
                    item.due = normalize_optional(draft.due);
                    item.note = normalize_optional(draft.note);
                    item.commits = deduplicated(draft.commits);
                    item.adopted_from_proposal_id = draft.adopted_from_proposal_id;
                    if !is_current {
                        item.status = draft.status;
                    }
                    item.updated_at = occurred_at.to_owned();
                } else {
                    state
                        .work_items
                        .push(work_item_from_draft(project_id, draft, occurred_at));
                }
            }
        }
        ProjectCommand::ConfirmCommitment { work_item_id } => {
            require_current(state, &work_item_id)?;
            require_item(state, &work_item_id)?;
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Confirmed,
                Some(work_item_id.clone()),
                Some(work_item_id),
                None,
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::CompleteCommitment { work_item_id } => {
            require_current(state, &work_item_id)?;
            let item = require_item_mut(state, &work_item_id)?;
            if item.status != WorkItemStatus::Doing {
                return Err(ProjectStateError::InvalidCommand(format!(
                    "work item {work_item_id} must be doing before completion"
                )));
            }
            item.status = WorkItemStatus::Done;
            item.updated_at = occurred_at.to_owned();
            state.current_next_action_id = None;
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Completed,
                Some(work_item_id),
                None,
                None,
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::ReplaceCommitment {
            previous_work_item_id,
            text,
            reason,
        } => {
            require_field("text", &text)?;
            if reason.trim().is_empty() {
                return Err(ProjectStateError::ReasonRequired);
            }
            require_current(state, &previous_work_item_id)?;
            // A replaced step was not finished, so it goes back to the list as planned work.
            // Leaving it at `doing` stranded it: nobody was working on it, yet it read as
            // in progress forever.
            let previous = require_item_mut(state, &previous_work_item_id)?;
            previous.status = WorkItemStatus::Planned;
            previous.updated_at = occurred_at.to_owned();
            let next = new_work_item(project_id, text, occurred_at);
            let next_id = next.id.clone();
            state.work_items.push(next);
            state.current_next_action_id = Some(next_id.clone());
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Replaced,
                Some(previous_work_item_id),
                Some(next_id),
                Some(reason),
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::ClearCommitment {
            work_item_id,
            reason,
        } => {
            require_current(state, &work_item_id)?;
            // Same reasoning as replace: clearing drops the commitment, not the work.
            let item = require_item_mut(state, &work_item_id)?;
            item.status = WorkItemStatus::Planned;
            item.updated_at = occurred_at.to_owned();
            state.current_next_action_id = None;
            push_transition(
                state,
                project_id,
                CommitmentTransitionKind::Cleared,
                Some(work_item_id),
                None,
                normalize_optional(reason),
                occurred_at,
                accepted_revision,
                None,
            );
        }
        ProjectCommand::SetStatus {
            status,
            reason,
            review_at,
        } => {
            if state.status == ProjectStatus::Setup || status == ProjectStatus::Setup {
                return Err(ProjectStateError::InvalidCommand(format!(
                    "status transition {:?} -> {:?} is not allowed",
                    state.status, status
                )));
            }
            let reason = match status {
                ProjectStatus::Waiting | ProjectStatus::Parked => match reason {
                    None => {
                        return Err(ProjectStateError::FieldRequired {
                            field: "status_reason",
                        });
                    }
                    Some(reason) if reason.trim().is_empty() => {
                        return Err(ProjectStateError::ReasonRequired);
                    }
                    Some(reason) => Some(reason),
                },
                _ => normalize_optional(reason),
            };
            if status == ProjectStatus::Waiting && review_at.is_none() {
                return Err(ProjectStateError::FieldRequired { field: "review_at" });
            }
            if let Some(review_at) = review_at.as_deref() {
                validate_command_timestamp("review_at", review_at)?;
                ensure_not_before("review_at", review_at, occurred_at)?;
            }
            state.status = status;
            state.status_reason = reason;
            state.review_at = review_at;
            state.status_changed_at = occurred_at.to_owned();
        }
        ProjectCommand::Undo { transition_id } => {
            undo_transition(
                state,
                project_id,
                transition_id,
                occurred_at,
                accepted_revision,
            )?;
        }
    }
    Ok(())
}

fn set_commitment_in_memory(
    state: &mut ProjectStateDoc,
    project_id: &ProjectId,
    text: String,
    occurred_at: &str,
    accepted_revision: u64,
) -> Result<(), ProjectStateError> {
    if let Some(work_item_id) = state.current_next_action_id.clone() {
        return Err(ProjectStateError::CurrentCommitmentExists { work_item_id });
    }
    let item = new_work_item(project_id, text, occurred_at);
    let item_id = item.id.clone();
    state.work_items.push(item);
    state.current_next_action_id = Some(item_id.clone());
    push_transition(
        state,
        project_id,
        CommitmentTransitionKind::Set,
        None,
        Some(item_id),
        None,
        occurred_at,
        accepted_revision,
        None,
    );
    Ok(())
}

fn undo_transition(
    state: &mut ProjectStateDoc,
    project_id: &ProjectId,
    transition_id: CommitmentTransitionId,
    occurred_at: &str,
    accepted_revision: u64,
) -> Result<(), ProjectStateError> {
    let target = state
        .commitment_transitions
        .last()
        .filter(|transition| transition.id == transition_id)
        .cloned()
        .ok_or_else(|| ProjectStateError::UndoConflict {
            transition_id: transition_id.clone(),
        })?;
    if target.kind == CommitmentTransitionKind::Correction {
        return Err(ProjectStateError::UndoConflict { transition_id });
    }
    if target.document_revision != state.revision {
        return Err(ProjectStateError::UndoConflict { transition_id });
    }

    match target.kind {
        CommitmentTransitionKind::Set => {
            let item_id = target.next_work_item_id.as_ref().ok_or_else(|| {
                ProjectStateError::InvalidCommand("Set transition has no next item".into())
            })?;
            // NOTE: undoing a `set` that promoted a pre-existing task abandons that task,
            // which drops it from the list. Fixing that needs the transition to record
            // whether it introduced the item, because `apply_status_correction` replays from
            // transitions alone and cannot otherwise tell the two cases apart. Left as is.
            let item = require_item_mut(state, item_id)?;
            item.status = WorkItemStatus::Abandoned;
            item.updated_at = occurred_at.to_owned();
        }
        CommitmentTransitionKind::Confirmed => {}
        CommitmentTransitionKind::Completed => {
            let item_id = target.previous_work_item_id.as_ref().ok_or_else(|| {
                ProjectStateError::InvalidCommand("Completed transition has no prior item".into())
            })?;
            let item = require_item_mut(state, item_id)?;
            item.status = WorkItemStatus::Doing;
            item.updated_at = occurred_at.to_owned();
        }
        CommitmentTransitionKind::Replaced => {
            let item_id = target.next_work_item_id.as_ref().ok_or_else(|| {
                ProjectStateError::InvalidCommand("Replaced transition has no next item".into())
            })?;
            let item = require_item_mut(state, item_id)?;
            item.status = WorkItemStatus::Abandoned;
            item.updated_at = occurred_at.to_owned();
            // The pointer replay hands the commitment back to the previous item, so its
            // status has to come back with it.
            let previous_id = target.previous_work_item_id.as_ref().ok_or_else(|| {
                ProjectStateError::InvalidCommand("Replaced transition has no prior item".into())
            })?;
            let previous = require_item_mut(state, previous_id)?;
            previous.status = WorkItemStatus::Doing;
            previous.updated_at = occurred_at.to_owned();
        }
        CommitmentTransitionKind::Cleared => {
            let item_id = target.previous_work_item_id.as_ref().ok_or_else(|| {
                ProjectStateError::InvalidCommand("Cleared transition has no prior item".into())
            })?;
            let item = require_item_mut(state, item_id)?;
            item.status = WorkItemStatus::Doing;
            item.updated_at = occurred_at.to_owned();
        }
        CommitmentTransitionKind::Correction => unreachable!("handled above"),
    }

    let before = state.current_next_action_id.clone();
    let mut corrected_ids: HashSet<_> = state
        .commitment_transitions
        .iter()
        .filter_map(|transition| transition.corrects_transition_id.clone())
        .collect();
    corrected_ids.insert(target.id.clone());
    let after = replay_masked_pointer(&state.commitment_transitions, &corrected_ids)?;
    state.current_next_action_id = after.clone();
    push_transition(
        state,
        project_id,
        CommitmentTransitionKind::Correction,
        before,
        after,
        None,
        occurred_at,
        accepted_revision,
        Some(target.id),
    );
    Ok(())
}

fn new_work_item(project_id: &ProjectId, text: String, occurred_at: &str) -> WorkItem {
    WorkItem {
        id: WorkItemId::new(),
        project_id: project_id.clone(),
        text,
        status: WorkItemStatus::Doing,
        unclear: false,
        due: None,
        note: None,
        tags: Vec::new(),
        commits: Vec::new(),
        blocker: None,
        blocked_at: None,
        created_at: occurred_at.to_owned(),
        updated_at: occurred_at.to_owned(),
        adopted_from_proposal_id: None,
        source_task_id: None,
    }
}

fn work_item_from_draft(
    project_id: &ProjectId,
    draft: WorkItemDraft,
    occurred_at: &str,
) -> WorkItem {
    WorkItem {
        id: WorkItemId::new(),
        project_id: project_id.clone(),
        text: draft.text.trim().to_owned(),
        status: draft.status,
        unclear: draft.unclear,
        due: normalize_optional(draft.due),
        note: normalize_optional(draft.note),
        // Draft tags were validated by `validate_work_item_draft`; this cannot fail.
        tags: normalize_tags(draft.tags).unwrap_or_default(),
        commits: deduplicated(draft.commits),
        blocker: None,
        blocked_at: None,
        created_at: occurred_at.to_owned(),
        updated_at: occurred_at.to_owned(),
        adopted_from_proposal_id: draft.adopted_from_proposal_id,
        source_task_id: draft.source_task_id,
    }
}

fn validate_work_item_draft(draft: &WorkItemDraft) -> Result<(), ProjectStateError> {
    require_field("text", &draft.text)?;
    validate_due(draft.due.as_deref())?;
    normalize_tags(draft.tags.clone())?;
    Ok(())
}

/// Every canonical byte rendering a pristine setup document has ever had, oldest first.
/// Store-migration provenance checks compare on-disk/history bytes against ALL of these,
/// because a state file may have been created by any past OmniProj version (doc schema v1
/// rendered `schema_version = 1`; v2 only adds defaulted fields, so the bodies are equal).
pub fn canonical_setup_renderings(created_at: &str) -> Result<Vec<String>, ProjectStateError> {
    let current = ProjectStateDoc::new_setup(created_at)?.render()?;
    let v1 = current.replacen("schema_version = 2", "schema_version = 1", 1);
    Ok(vec![v1, current])
}

/// Normalize user-entered tags for writing (FR-R5): trim each tag, reject empty or
/// over-long entries, drop case-insensitive duplicates preserving first-seen order and
/// the user's original casing, and cap the count. Chinese has no case; English tags keep
/// their original spelling and compare case-insensitively.
pub fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, ProjectStateError> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(ProjectStateError::InvalidCommand(
                "tags must not be empty".into(),
            ));
        }
        if tag.chars().count() > MAX_TAG_CHARS {
            return Err(ProjectStateError::InvalidCommand(format!(
                "tag {tag:?} exceeds {MAX_TAG_CHARS} characters"
            )));
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_owned());
        }
    }
    if normalized.len() > MAX_TAGS_PER_ITEM {
        return Err(ProjectStateError::InvalidCommand(format!(
            "at most {MAX_TAGS_PER_ITEM} tags per work item"
        )));
    }
    Ok(normalized)
}

/// Invariants over already-persisted tags (checked on load and save).
fn validate_tags(tags: &[String]) -> Result<(), &'static str> {
    if tags.len() > MAX_TAGS_PER_ITEM {
        return Err("too many tags");
    }
    let mut seen = HashSet::new();
    for tag in tags {
        if tag.trim().is_empty() || tag.trim() != tag {
            return Err("tags must be trimmed and non-empty");
        }
        if tag.chars().count() > MAX_TAG_CHARS {
            return Err("tag too long");
        }
        if !seen.insert(tag.to_lowercase()) {
            return Err("duplicate tag");
        }
    }
    Ok(())
}

fn validate_due(value: Option<&str>) -> Result<(), ProjectStateError> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ProjectStateError::InvalidCommand("due date must use YYYY-MM-DD".into()))
}

fn work_item_is_referenced(state: &ProjectStateDoc, work_item_id: &WorkItemId) -> bool {
    state.commitment_transitions.iter().any(|transition| {
        transition.previous_work_item_id.as_ref() == Some(work_item_id)
            || transition.next_work_item_id.as_ref() == Some(work_item_id)
    })
}

fn deduplicated(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn push_transition(
    state: &mut ProjectStateDoc,
    project_id: &ProjectId,
    kind: CommitmentTransitionKind,
    previous_work_item_id: Option<WorkItemId>,
    next_work_item_id: Option<WorkItemId>,
    reason: Option<String>,
    occurred_at: &str,
    document_revision: u64,
    corrects_transition_id: Option<CommitmentTransitionId>,
) {
    state.commitment_transitions.push(CommitmentTransition {
        id: CommitmentTransitionId::new(),
        project_id: project_id.clone(),
        document_revision,
        kind,
        previous_work_item_id,
        next_work_item_id,
        reason,
        occurred_at: occurred_at.to_owned(),
        corrects_transition_id,
    });
}

fn require_item<'a>(
    state: &'a ProjectStateDoc,
    work_item_id: &WorkItemId,
) -> Result<&'a WorkItem, ProjectStateError> {
    state
        .work_items
        .iter()
        .find(|item| &item.id == work_item_id)
        .ok_or_else(|| ProjectStateError::WorkItemNotFound(work_item_id.clone()))
}

fn require_item_mut<'a>(
    state: &'a mut ProjectStateDoc,
    work_item_id: &WorkItemId,
) -> Result<&'a mut WorkItem, ProjectStateError> {
    state
        .work_items
        .iter_mut()
        .find(|item| &item.id == work_item_id)
        .ok_or_else(|| ProjectStateError::WorkItemNotFound(work_item_id.clone()))
}

fn require_current(
    state: &ProjectStateDoc,
    expected: &WorkItemId,
) -> Result<(), ProjectStateError> {
    if state.current_next_action_id.as_ref() == Some(expected) {
        Ok(())
    } else {
        Err(ProjectStateError::CurrentCommitmentMismatch {
            expected: expected.clone(),
            actual: state.current_next_action_id.clone(),
        })
    }
}

fn require_field(field: &'static str, value: &str) -> Result<(), ProjectStateError> {
    if value.trim().is_empty() {
        Err(ProjectStateError::FieldRequired { field })
    } else {
        Ok(())
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn validate_command_timestamp(field: &'static str, value: &str) -> Result<(), ProjectStateError> {
    DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| ProjectStateError::InvalidTimestamp {
            field,
            value: value.to_owned(),
        })
}

fn ensure_not_before(
    field: &'static str,
    value: &str,
    earlier: &str,
) -> Result<(), ProjectStateError> {
    let value_parsed =
        DateTime::parse_from_rfc3339(value).map_err(|_| ProjectStateError::InvalidTimestamp {
            field,
            value: value.to_owned(),
        })?;
    let earlier_parsed = DateTime::parse_from_rfc3339(earlier)
        .expect("stored timestamps were validated before command application");
    if value_parsed < earlier_parsed {
        Err(ProjectStateError::InvalidTimestamp {
            field,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn state_path(project_id: &ProjectId) -> PathBuf {
    notes_dir_for(project_id).join("project.md")
}

fn invalid<T>(message: String) -> Result<T, ProjectStateError> {
    Err(ProjectStateError::InvalidDocument(message))
}

fn validate_optional_timestamp(field: &str, value: Option<&str>) -> Result<(), ProjectStateError> {
    if let Some(value) = value {
        validate_timestamp(field, value)?;
    }
    Ok(())
}

fn validate_timestamp(field: &str, value: &str) -> Result<(), ProjectStateError> {
    DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| ProjectStateError::InvalidDocument(format!("{field} is not RFC3339")))
}

fn validate_ordered_timestamp(
    earlier_field: &str,
    earlier: &str,
    later_field: &str,
    later: &str,
) -> Result<(), ProjectStateError> {
    let earlier = DateTime::parse_from_rfc3339(earlier).map_err(|_| {
        ProjectStateError::InvalidDocument(format!("{earlier_field} is not RFC3339"))
    })?;
    let later = DateTime::parse_from_rfc3339(later)
        .map_err(|_| ProjectStateError::InvalidDocument(format!("{later_field} is not RFC3339")))?;
    if later < earlier {
        return invalid(format!("{later_field} must not precede {earlier_field}"));
    }
    Ok(())
}

fn validate_nonempty_option(field: &str, value: Option<&str>) -> Result<(), ProjectStateError> {
    if value.is_some_and(|value| !value.trim().is_empty()) {
        Ok(())
    } else {
        invalid(format!("{field} must not be empty"))
    }
}

fn validate_aggregate_project_id<'a>(
    aggregate: &mut Option<&'a ProjectId>,
    candidate: &'a ProjectId,
) -> Result<(), ProjectStateError> {
    match aggregate {
        Some(expected) if *expected != candidate => invalid(format!(
            "aggregate contains project ids {expected} and {candidate}"
        )),
        Some(_) => Ok(()),
        None => {
            *aggregate = Some(candidate);
            Ok(())
        }
    }
}

fn validate_transition_static_shape(
    transition: &CommitmentTransition,
) -> Result<(), ProjectStateError> {
    let invalid_shape = || {
        invalid(format!(
            "invalid {:?} transition {}",
            transition.kind, transition.id
        ))
    };
    match transition.kind {
        CommitmentTransitionKind::Set => {
            if transition.previous_work_item_id.is_some()
                || transition.next_work_item_id.is_none()
                || transition.reason.is_some()
                || transition.corrects_transition_id.is_some()
            {
                return invalid_shape();
            }
        }
        CommitmentTransitionKind::Confirmed => {
            if transition.previous_work_item_id.is_none()
                || transition.previous_work_item_id != transition.next_work_item_id
                || transition.reason.is_some()
                || transition.corrects_transition_id.is_some()
            {
                return invalid_shape();
            }
        }
        CommitmentTransitionKind::Completed => {
            if transition.previous_work_item_id.is_none()
                || transition.next_work_item_id.is_some()
                || transition.reason.is_some()
                || transition.corrects_transition_id.is_some()
            {
                return invalid_shape();
            }
        }
        CommitmentTransitionKind::Replaced => {
            if transition.previous_work_item_id.is_none()
                || transition.next_work_item_id.is_none()
                || transition.previous_work_item_id == transition.next_work_item_id
                || !transition
                    .reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty())
                || transition.corrects_transition_id.is_some()
            {
                return invalid_shape();
            }
        }
        CommitmentTransitionKind::Cleared => {
            if transition.previous_work_item_id.is_none()
                || transition.next_work_item_id.is_some()
                || transition
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.trim().is_empty())
                || transition.corrects_transition_id.is_some()
            {
                return invalid_shape();
            }
        }
        CommitmentTransitionKind::Correction => {
            if transition.corrects_transition_id.is_none() || transition.reason.is_some() {
                return invalid_shape();
            }
        }
    }
    Ok(())
}

/// What the transition log says the commitment pointer must be.
///
/// The log is the audit record: it owns *what happened* — every set, confirm, complete,
/// replace and clear, plus the pointer they imply — and all of that is still replayed and
/// compared against the stored document. It deliberately does NOT own the current status of
/// items it has touched. Reopening a step you once completed is ordinary work, not a forged
/// history; pinning those statuses forever made every past commitment unmovable and
/// undeletable. The one status the log still fixes is the item it points at right now, which
/// must be `doing` — that is checked directly from the pointer.
struct CommitmentReplay {
    current_next_action_id: Option<WorkItemId>,
}

fn replay_and_validate_commitment_history(
    transitions: &[CommitmentTransition],
) -> Result<CommitmentReplay, ProjectStateError> {
    let mut current = None;
    let mut corrected_ids = HashSet::new();
    for (index, transition) in transitions.iter().enumerate() {
        if transition.kind == CommitmentTransitionKind::Correction {
            if transition.previous_work_item_id != current {
                return invalid(format!(
                    "correction {} has forged before pointer",
                    transition.id
                ));
            }
            let target = transitions
                .get(index.saturating_sub(1))
                .filter(|target| Some(&target.id) == transition.corrects_transition_id.as_ref())
                .ok_or_else(|| {
                    ProjectStateError::InvalidDocument(format!(
                        "correction {} does not target its immediate predecessor",
                        transition.id
                    ))
                })?;
            corrected_ids.insert(target.id.clone());
            let after = replay_masked_pointer(&transitions[..index], &corrected_ids)?;
            if transition.next_work_item_id != after {
                return invalid(format!(
                    "correction {} has forged after pointer",
                    transition.id
                ));
            }
            current = after;
        } else {
            apply_pointer_transition(&mut current, transition)?;
        }
    }
    Ok(CommitmentReplay {
        current_next_action_id: current,
    })
}

fn replay_masked_pointer(
    transitions: &[CommitmentTransition],
    corrected_ids: &HashSet<CommitmentTransitionId>,
) -> Result<Option<WorkItemId>, ProjectStateError> {
    let mut current: Option<WorkItemId> = None;
    for transition in transitions {
        if transition.kind == CommitmentTransitionKind::Correction
            || corrected_ids.contains(&transition.id)
        {
            continue;
        }
        apply_pointer_transition(&mut current, transition)?;
    }
    Ok(current)
}

fn apply_pointer_transition(
    current: &mut Option<WorkItemId>,
    transition: &CommitmentTransition,
) -> Result<(), ProjectStateError> {
    match transition.kind {
        CommitmentTransitionKind::Set => {
            if current.is_some() {
                return invalid(format!("invalid Set transition {}", transition.id));
            }
            *current = transition.next_work_item_id.clone();
        }
        CommitmentTransitionKind::Confirmed => {
            if current.is_none() || transition.previous_work_item_id != *current {
                return invalid(format!("invalid Confirmed transition {}", transition.id));
            }
        }
        CommitmentTransitionKind::Completed | CommitmentTransitionKind::Cleared => {
            if current.is_none() || transition.previous_work_item_id != *current {
                return invalid(format!(
                    "invalid {:?} transition {}",
                    transition.kind, transition.id
                ));
            }
            *current = None;
        }
        CommitmentTransitionKind::Replaced => {
            if current.is_none() || transition.previous_work_item_id != *current {
                return invalid(format!("invalid Replaced transition {}", transition.id));
            }
            *current = transition.next_work_item_id.clone();
        }
        CommitmentTransitionKind::Correction => {
            return invalid(format!(
                "unexpected Correction transition {}",
                transition.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATED_AT: &str = "2026-08-10T12:00:00Z";

    /// A legacy v1 document: still accepted as input (upgraded in memory on parse).
    const SETUP: &str = "+++\nschema_version = 1\nrevision = 0\nstatus = \"setup\"\nstatus_changed_at = \"2026-08-10T12:00:00Z\"\ncreated_at = \"2026-08-10T12:00:00Z\"\nupdated_at = \"2026-08-10T12:00:00Z\"\nwork_items = []\ncommitment_transitions = []\n+++\n\n# Project notes\n";

    /// The canonical current-version render of the same setup document.
    const SETUP_V2: &str = "+++\nschema_version = 2\nrevision = 0\nstatus = \"setup\"\nstatus_changed_at = \"2026-08-10T12:00:00Z\"\ncreated_at = \"2026-08-10T12:00:00Z\"\nupdated_at = \"2026-08-10T12:00:00Z\"\nwork_items = []\ncommitment_transitions = []\n+++\n\n# Project notes\n";

    fn valid_with(front_matter_suffix: &str, body: &str) -> String {
        format!(
            "+++\nschema_version = 1\nrevision = 3\nstatus = \"active\"\nstatus_changed_at = {CREATED_AT:?}\ncreated_at = {CREATED_AT:?}\nupdated_at = {CREATED_AT:?}\n{front_matter_suffix}+++\n{body}"
        )
    }

    #[test]
    fn new_setup_renders_the_canonical_fixture() {
        let document = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
        assert_eq!(document.render().unwrap(), SETUP_V2);
    }

    #[test]
    fn parser_preserves_markdown_body_bytes() {
        let input = valid_with(
            "work_items = []\ncommitment_transitions = []\n",
            "\n# Human heading\r\n\r\nUnknown *Markdown* bytes.  \r\n",
        );

        let parsed = ProjectStateDoc::parse(&input).unwrap();

        assert_eq!(
            parsed.markdown_body(),
            "\n# Human heading\r\n\r\nUnknown *Markdown* bytes.  \r\n"
        );
        assert_eq!(
            ProjectStateDoc::parse(&parsed.render().unwrap())
                .unwrap()
                .markdown_body(),
            parsed.markdown_body()
        );
    }

    #[test]
    fn parser_rejects_missing_or_misplaced_delimiters() {
        for input in ["", "schema_version = 1\n+++\n", "+++\nschema_version = 1\n"] {
            assert!(matches!(
                ProjectStateDoc::parse(input),
                Err(ProjectStateError::InvalidDocument(_))
            ));
        }
    }

    #[test]
    fn parser_rejects_unsupported_document_schema() {
        let input = SETUP.replacen("schema_version = 1", "schema_version = 3", 1);
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::UnsupportedSchema(3))
        ));
    }

    #[test]
    fn v1_documents_parse_and_upgrade_in_memory_to_v2() {
        let document = ProjectStateDoc::parse(SETUP).unwrap();
        assert_eq!(document.schema_version, 2);
        assert!(document.work_items.iter().all(|item| item.tags.is_empty()));
        // The next render persists the current version.
        assert!(document.render().unwrap().contains("schema_version = 2"));
    }

    #[test]
    fn newer_schema_reports_version_not_unknown_field_noise() {
        // A v3 document with a field this build does not know must fail on the version,
        // not on deny_unknown_fields.
        let input = SETUP.replacen(
            "schema_version = 1",
            "schema_version = 3\nfuture_field = true",
            1,
        );
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::UnsupportedSchema(3))
        ));
    }

    #[test]
    fn normalize_tags_trims_dedupes_case_insensitively_and_bounds() {
        assert_eq!(
            normalize_tags(vec![" 论文 ".into(), "infra".into(), "Infra".into()]).unwrap(),
            vec!["论文".to_owned(), "infra".to_owned()]
        );
        assert!(normalize_tags(vec!["".into()]).is_err());
        assert!(normalize_tags(vec!["   ".into()]).is_err());
        assert!(normalize_tags(vec!["长".repeat(25)]).is_err());
        let too_many: Vec<String> = (0..9).map(|i| format!("tag-{i}")).collect();
        assert!(normalize_tags(too_many).is_err());
        let exactly_eight: Vec<String> = (0..8).map(|i| format!("tag-{i}")).collect();
        assert_eq!(
            normalize_tags(exactly_eight.clone()).unwrap(),
            exactly_eight
        );
    }

    #[test]
    fn parser_requires_persisted_collection_fields() {
        for required in ["work_items = []\n", "commitment_transitions = []\n"] {
            let input = SETUP.replacen(required, "", 1);
            assert!(
                matches!(
                    ProjectStateDoc::parse(&input),
                    Err(ProjectStateError::InvalidDocument(_))
                ),
                "missing {required:?} was accepted"
            );
        }
    }

    #[test]
    fn parser_rejects_duplicate_ids_and_dangling_work_item_references() {
        let duplicate = valid_with(
            concat!(
                "current_next_action_id = \"work-1\"\n",
                "commitment_transitions = []\n",
                "[[work_items]]\n",
                "id = \"work-1\"\n",
                "project_id = \"project-1\"\n",
                "text = \"First\"\n",
                "status = \"doing\"\n",
                "created_at = \"2026-08-10T12:00:00Z\"\n",
                "updated_at = \"2026-08-10T12:00:00Z\"\n",
                "[[work_items]]\n",
                "id = \"work-1\"\n",
                "project_id = \"project-1\"\n",
                "text = \"Duplicate\"\n",
                "status = \"planned\"\n",
                "created_at = \"2026-08-10T12:00:00Z\"\n",
                "updated_at = \"2026-08-10T12:00:00Z\"\n"
            ),
            "\nbody\n",
        );
        assert!(matches!(
            ProjectStateDoc::parse(&duplicate),
            Err(ProjectStateError::InvalidDocument(_))
        ));

        let dangling = valid_with(
            "current_next_action_id = \"missing-work\"\nwork_items = []\ncommitment_transitions = []\n",
            "\nbody\n",
        );
        assert!(matches!(
            ProjectStateDoc::parse(&dangling),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }

    #[test]
    fn parser_rejects_invalid_timestamps() {
        let input = SETUP.replacen(CREATED_AT, "not-a-timestamp", 1);
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }

    #[test]
    fn parser_rejects_duplicate_transition_ids_and_dangling_transition_pointers() {
        let duplicate = valid_with(
            concat!(
                "work_items = []\n",
                "[[commitment_transitions]]\n",
                "id = \"transition-1\"\n",
                "project_id = \"project-1\"\n",
                "type = \"cleared\"\n",
                "occurred_at = \"2026-08-10T12:00:00Z\"\n",
                "[[commitment_transitions]]\n",
                "id = \"transition-1\"\n",
                "project_id = \"project-1\"\n",
                "type = \"cleared\"\n",
                "occurred_at = \"2026-08-10T12:00:00Z\"\n"
            ),
            "\nbody\n",
        );
        assert!(matches!(
            ProjectStateDoc::parse(&duplicate),
            Err(ProjectStateError::InvalidDocument(_))
        ));

        let dangling = valid_with(
            concat!(
                "work_items = []\n",
                "[[commitment_transitions]]\n",
                "id = \"transition-1\"\n",
                "project_id = \"project-1\"\n",
                "type = \"set\"\n",
                "next_work_item_id = \"missing-work\"\n",
                "occurred_at = \"2026-08-10T12:00:00Z\"\n"
            ),
            "\nbody\n",
        );
        assert!(matches!(
            ProjectStateDoc::parse(&dangling),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }

    #[test]
    fn complete_command_rejects_blocked_current_item_without_in_memory_mutation() {
        let project_id = ProjectId::parse("project-blocked-complete").unwrap();
        let mut state = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
        let mut item = new_work_item(&project_id, "Blocked action".into(), CREATED_AT);
        let item_id = item.id.clone();
        item.status = WorkItemStatus::Blocked;
        item.blocker = Some("External dependency".into());
        item.blocked_at = Some(CREATED_AT.into());
        state.current_next_action_id = Some(item_id.clone());
        state.work_items.push(item);
        let before = state.clone();

        let error = apply_command_in_memory(
            &mut state,
            &project_id,
            ProjectCommand::CompleteCommitment {
                work_item_id: item_id,
            },
            CREATED_AT,
            1,
        )
        .unwrap_err();

        assert!(matches!(error, ProjectStateError::InvalidCommand(_)));
        assert_eq!(state, before);
    }

    #[test]
    fn canonical_work_item_becomes_commitment_without_creating_a_duplicate() {
        let project_id = ProjectId::parse("project-canonical-work").unwrap();
        let mut state = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
        state.status = ProjectStatus::Active;
        state.objective = Some("Ship the re-entry loop".into());
        state.desired_outcome = Some("Resume work without reconstructing history".into());

        apply_command_in_memory(
            &mut state,
            &project_id,
            ProjectCommand::AddWorkItems {
                items: vec![WorkItemDraft {
                    text: "Validate one real re-entry".into(),
                    status: WorkItemStatus::Planned,
                    unclear: false,
                    due: Some("2026-08-12".into()),
                    note: Some("Use a project untouched for 24 hours".into()),
                    tags: Vec::new(),
                    commits: Vec::new(),
                    adopted_from_proposal_id: None,
                    source_task_id: None,
                }],
            },
            CREATED_AT,
            1,
        )
        .unwrap();
        let work_item_id = state.work_items[0].id.clone();

        apply_command_in_memory(
            &mut state,
            &project_id,
            ProjectCommand::SetCommitmentFromWorkItem {
                work_item_id: work_item_id.clone(),
            },
            CREATED_AT,
            2,
        )
        .unwrap();

        assert_eq!(state.work_items.len(), 1);
        assert_eq!(state.current_next_action_id, Some(work_item_id));
        assert_eq!(state.work_items[0].status, WorkItemStatus::Doing);
    }

    #[test]
    fn legacy_import_enriches_an_existing_linked_work_item_once() {
        let project_id = ProjectId::parse("project-legacy-import").unwrap();
        let mut state = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
        state.status = ProjectStatus::Active;
        let mut linked = new_work_item(&project_id, "Existing commitment".into(), CREATED_AT);
        linked.source_task_id = Some("legacy-task".into());
        let linked_id = linked.id.clone();
        state.work_items.push(linked);

        let draft = WorkItemDraft {
            text: "Existing commitment".into(),
            status: WorkItemStatus::Planned,
            unclear: true,
            due: Some("2026-08-20".into()),
            note: Some("Imported context".into()),
            tags: Vec::new(),
            commits: vec!["abc123".into(), "abc123".into()],
            adopted_from_proposal_id: None,
            source_task_id: Some("legacy-task".into()),
        };
        apply_command_in_memory(
            &mut state,
            &project_id,
            ProjectCommand::ImportLegacyWorkItems {
                items: vec![draft.clone()],
            },
            CREATED_AT,
            1,
        )
        .unwrap();
        apply_command_in_memory(
            &mut state,
            &project_id,
            ProjectCommand::ImportLegacyWorkItems { items: vec![draft] },
            CREATED_AT,
            2,
        )
        .unwrap();

        assert_eq!(state.work_items.len(), 1);
        assert_eq!(state.work_items[0].id, linked_id);
        assert!(state.work_items[0].unclear);
        assert_eq!(state.work_items[0].commits, vec!["abc123"]);
    }

    #[test]
    fn load_reports_not_found_and_save_atomically_round_trips() {
        let _guard = crate::env_guard();
        let home =
            std::env::temp_dir().join(format!("omniproj-project-state-{}", uuid::Uuid::now_v7()));
        std::env::set_var("OMNIPROJ_HOME", &home);
        let project_id = ProjectId::parse("project-state-test").unwrap();
        assert!(matches!(
            ProjectStateDoc::load(&project_id),
            Err(ProjectStateError::NotFound(_))
        ));
        let document = ProjectStateDoc::new_setup(CREATED_AT).unwrap();

        document.save(&project_id).unwrap();

        assert_eq!(ProjectStateDoc::load(&project_id).unwrap(), document);
        let state_dir = notes_dir_for(&project_id);
        assert_eq!(std::fs::read_dir(state_dir).unwrap().count(), 1);
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}
