use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::ids::{CommitmentTransitionId, ProjectId, WorkItemId};
use crate::paths::notes_dir_for;
use crate::store::{atomic_write_store, StoreError};

const DOCUMENT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_MARKDOWN_BODY: &str = "\n# Project notes\n";

#[derive(Debug)]
pub enum ProjectStateError {
    NotFound(PathBuf),
    Io(std::io::Error),
    InvalidDocument(String),
    UnsupportedSchema(u32),
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
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProjectStateError {}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopted_from_proposal_id: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentTransition {
    pub id: CommitmentTransitionId,
    pub project_id: ProjectId,
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
        let mut document: Self = toml::from_str(toml_text)
            .map_err(|error| ProjectStateError::InvalidDocument(error.to_string()))?;
        if document.schema_version != DOCUMENT_SCHEMA_VERSION {
            return Err(ProjectStateError::UnsupportedSchema(
                document.schema_version,
            ));
        }
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
        Self::parse(&input)
    }

    pub fn save(&self, project_id: &ProjectId) -> Result<(), ProjectStateError> {
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

        let mut work_item_ids = HashSet::new();
        for item in &self.work_items {
            if !work_item_ids.insert(item.id.clone()) {
                return invalid(format!("duplicate work item id {}", item.id));
            }
            validate_timestamp("work item created_at", &item.created_at)?;
            validate_timestamp("work item updated_at", &item.updated_at)?;
            validate_optional_timestamp("work item blocked_at", item.blocked_at.as_deref())?;
        }
        if let Some(current) = &self.current_next_action_id {
            if !work_item_ids.contains(current) {
                return invalid(format!("current next action {current} does not exist"));
            }
        }

        let mut transition_ids = HashSet::new();
        for transition in &self.commitment_transitions {
            if !transition_ids.insert(transition.id.clone()) {
                return invalid(format!("duplicate transition id {}", transition.id));
            }
            validate_timestamp("transition occurred_at", &transition.occurred_at)?;
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
        }
        for transition in &self.commitment_transitions {
            if let Some(corrected) = &transition.corrects_transition_id {
                if !transition_ids.contains(corrected) {
                    return invalid(format!(
                        "transition references missing corrected transition {corrected}"
                    ));
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const CREATED_AT: &str = "2026-08-10T12:00:00Z";

    const SETUP: &str = "+++\nschema_version = 1\nrevision = 0\nstatus = \"setup\"\nstatus_changed_at = \"2026-08-10T12:00:00Z\"\ncreated_at = \"2026-08-10T12:00:00Z\"\nupdated_at = \"2026-08-10T12:00:00Z\"\nwork_items = []\ncommitment_transitions = []\n+++\n\n# Project notes\n";

    fn valid_with(front_matter_suffix: &str, body: &str) -> String {
        format!(
            "+++\nschema_version = 1\nrevision = 3\nstatus = \"active\"\nstatus_changed_at = {CREATED_AT:?}\ncreated_at = {CREATED_AT:?}\nupdated_at = {CREATED_AT:?}\n{front_matter_suffix}+++\n{body}"
        )
    }

    #[test]
    fn new_setup_renders_the_canonical_fixture() {
        let document = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
        assert_eq!(document.render().unwrap(), SETUP);
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
        let input = SETUP.replacen("schema_version = 1", "schema_version = 2", 1);
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::UnsupportedSchema(2))
        ));
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
