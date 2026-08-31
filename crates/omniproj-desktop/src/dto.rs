//! The typed R0 wire contract: snake-case DTOs the React surface consumes, plus the
//! pure assemblers that build them from core domain values. No IO, no Git, no clock —
//! everything here is a deterministic function of already-loaded state.
//!
//! Index rows are deliberately lighter than the Overview: they exclude the full source
//! path and the transition rail. Both carry the Human-state `revision` (for mutation
//! conflict detection) and the fixed `review_policy`.

use serde::{Deserialize, Serialize};

use omniproj_core::ids::{CommitmentTransitionId, ProjectId, ProjectSourceId, WorkItemId};
use omniproj_core::project::{
    ProjectRecord, ProjectSource, ProjectSourceKind, ProjectSourceStatus,
};
use omniproj_core::project_state::{
    CommitmentTransition, CommitmentTransitionKind, ProjectStateDoc, ProjectStatus, WorkItem,
    WorkItemStatus,
};
use omniproj_core::review::{
    ReviewReason, ReviewReasonCode, DEFAULT_COMMITMENT_REVIEW_DAYS, REVIEW_RULE_VERSION,
};

/// The fixed review policy echoed on every Index response and Overview so the UI can
/// state the seven-day rule without hardcoding it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewPolicyDto {
    pub commitment_review_days: i64,
    pub rule_version: String,
}

impl ReviewPolicyDto {
    pub fn r0() -> Self {
        Self {
            commitment_review_days: DEFAULT_COMMITMENT_REVIEW_DAYS,
            rule_version: REVIEW_RULE_VERSION.to_owned(),
        }
    }
}

/// One deterministic review signal, already priority-sorted in core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewReasonDto {
    pub code: String,
    pub label: String,
    pub evidence: Vec<String>,
    pub rule_version: String,
}

/// Stable snake_case name for a review reason code (wire contract).
pub fn review_reason_code_name(code: ReviewReasonCode) -> &'static str {
    match code {
        ReviewReasonCode::SourceUnavailable => "source_unavailable",
        ReviewReasonCode::CompleteSetup => "complete_setup",
        ReviewReasonCode::NeedsCommitment => "needs_commitment",
        ReviewReasonCode::ReviewAction => "review_action",
        ReviewReasonCode::ScheduledReview => "scheduled_review",
    }
}

/// Fixed display/sort priority for a review code (lower is more urgent).
fn review_reason_priority(code: ReviewReasonCode) -> u8 {
    match code {
        ReviewReasonCode::SourceUnavailable => 0,
        ReviewReasonCode::CompleteSetup => 1,
        ReviewReasonCode::NeedsCommitment => 2,
        ReviewReasonCode::ReviewAction => 3,
        ReviewReasonCode::ScheduledReview => 4,
    }
}

fn review_reason_dtos(reasons: &[ReviewReason]) -> Vec<ReviewReasonDto> {
    let mut ordered: Vec<&ReviewReason> = reasons.iter().collect();
    ordered.sort_by_key(|reason| review_reason_priority(reason.code));
    ordered
        .into_iter()
        .map(|reason| ReviewReasonDto {
            code: review_reason_code_name(reason.code).to_owned(),
            label: reason.label.clone(),
            evidence: reason.evidence.clone(),
            rule_version: reason.rule_version.clone(),
        })
        .collect()
}

/// HEAD position at the last successful observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HeadStateDto {
    Attached { branch: String },
    Detached,
    Unborn { branch: Option<String> },
}

/// One observed commit (neutral fact, never a judgement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDto {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub committed_at: String,
}

/// The machine-observed actual, rebuilt from the last successful observation cache.
/// `commits_since_commitment` is present only when a current commitment exists AND the
/// cached count was computed against that same commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedActualDto {
    pub observed_at: String,
    pub head: HeadStateDto,
    pub last_commit: Option<CommitDto>,
    pub changed_files: u32,
    pub staged_files: u32,
    pub unstaged_files: u32,
    pub untracked_files: u32,
    pub status_digest: String,
    pub commits_since_commitment: Option<u32>,
    /// Sixteen UTC calendar-week commit counts, oldest to newest.
    pub commit_activity_weeks: Vec<u32>,
    /// Whole days since the last commit at response time. `None` means the
    /// repository has no commit or activity could not be observed.
    pub silent_days: Option<u32>,
}

/// The Human's current explicit commitment (the one action).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentCommitmentDto {
    pub work_item_id: WorkItemId,
    pub text: String,
    pub status: WorkItemStatus,
    /// When this commitment was set/replaced (the work item's creation instant).
    pub set_at: String,
    /// When it was last confirmed, if ever.
    pub confirmed_at: Option<String>,
}

/// One commitment lifecycle transition, for the recent-transition rail / Undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitmentTransitionDto {
    pub id: CommitmentTransitionId,
    #[serde(rename = "type")]
    pub kind: CommitmentTransitionKind,
    pub previous_work_item_id: Option<WorkItemId>,
    pub next_work_item_id: Option<WorkItemId>,
    pub reason: Option<String>,
    pub occurred_at: String,
    pub corrects_transition_id: Option<CommitmentTransitionId>,
}

impl From<&CommitmentTransition> for CommitmentTransitionDto {
    fn from(transition: &CommitmentTransition) -> Self {
        Self {
            id: transition.id.clone(),
            kind: transition.kind,
            previous_work_item_id: transition.previous_work_item_id.clone(),
            next_work_item_id: transition.next_work_item_id.clone(),
            reason: transition.reason.clone(),
            occurred_at: transition.occurred_at.clone(),
            corrects_transition_id: transition.corrects_transition_id.clone(),
        }
    }
}

/// A project source with its full location (Overview only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSourceDto {
    pub source_id: ProjectSourceId,
    pub kind: ProjectSourceKind,
    pub location: String,
    pub is_primary: bool,
    pub status: ProjectSourceStatus,
    pub last_observed_at: Option<String>,
    pub last_successful_refresh_at: Option<String>,
    pub last_error_category: Option<String>,
    /// Source-envelope revision, for refresh/relink optimistic concurrency.
    pub revision: u64,
}

impl From<&ProjectSource> for ProjectSourceDto {
    fn from(source: &ProjectSource) -> Self {
        Self {
            source_id: source.id.clone(),
            kind: source.kind,
            location: source.location.clone(),
            is_primary: source.is_primary,
            status: source.status,
            last_observed_at: source.last_observed_at.clone(),
            last_successful_refresh_at: source.last_successful_refresh_at.clone(),
            last_error_category: source.last_error_category.clone(),
            revision: source.revision,
        }
    }
}

/// One dense Index row. Excludes the full source path (Overview carries it) but keeps
/// the source `status` and both revisions the UI needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIndexItemDto {
    pub project_id: ProjectId,
    pub name: String,
    pub status: ProjectStatus,
    pub current_commitment: Option<CurrentCommitmentDto>,
    pub observed_actual: Option<ObservedActualDto>,
    pub review_reasons: Vec<ReviewReasonDto>,
    pub source_status: ProjectSourceStatus,
    /// Human-state revision (mutation conflict detection).
    pub revision: u64,
    /// Source-envelope revision (refresh/relink conflict detection).
    pub source_revision: u64,
}

/// The dense cross-project operating index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIndexResponseDto {
    pub projects: Vec<ProjectIndexItemDto>,
    pub review_policy: ReviewPolicyDto,
}

/// The full project Overview, shared verbatim by Peek and full-page rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectOverviewDto {
    pub project_id: ProjectId,
    pub name: String,
    pub created_at: String,
    pub status: ProjectStatus,
    pub status_reason: Option<String>,
    pub phase: Option<String>,
    pub objective: Option<String>,
    pub desired_outcome: Option<String>,
    pub review_at: Option<String>,
    pub source: Option<ProjectSourceDto>,
    pub current_commitment: Option<CurrentCommitmentDto>,
    pub observed_actual: Option<ObservedActualDto>,
    pub review_reasons: Vec<ReviewReasonDto>,
    pub recent_transitions: Vec<CommitmentTransitionDto>,
    pub last_transition: Option<CommitmentTransitionDto>,
    pub undoable_transition_id: Option<CommitmentTransitionId>,
    pub review_policy: ReviewPolicyDto,
    pub revision: u64,
}

/// Typed preview/validation states for a candidate source path (Add / Relink).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SourceValidationDto {
    Ok {
        location: String,
        head: HeadStateDto,
        last_commit: Option<CommitDto>,
    },
    Missing {
        location: String,
    },
    Unreadable {
        location: String,
    },
    NotGitRepository {
        location: String,
    },
    BareRepository {
        location: String,
    },
    ObservationFailed {
        location: String,
        message: String,
    },
    Duplicate {
        location: String,
        existing_project_id: ProjectId,
        existing_name: String,
    },
}

/// The outcome of one project's refresh within a (possibly partial) batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshOutcome {
    /// A fresh observation was recorded.
    Refreshed,
    /// A concurrent refresh for this project is already running; the cached row is returned.
    RefreshInProgress,
    /// The source could not be observed; cached facts are preserved and the row still returned.
    SourceFailed,
    /// A relink won the race and moved the source; this stale result was discarded.
    Stale,
}

/// One result per requested project. Never rejects the batch on a single failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshResultDto {
    pub project_id: ProjectId,
    pub outcome: RefreshOutcome,
    pub item: Option<ProjectIndexItemDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

// ---------------------------------------------------------------------------
// Input DTOs (deserialized from IPC request bodies)
// ---------------------------------------------------------------------------

/// `register_project` input: a candidate location and a display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterProjectInput {
    pub location: String,
    pub name: String,
}

/// `relink_project_source` input, carrying the expected source revision + location so a
/// concurrent relink/observation cannot be silently clobbered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelinkProjectInput {
    pub project_id: ProjectId,
    pub expected_source_revision: u64,
    pub expected_location: String,
    pub new_location: String,
}

/// The specific Human mutation to apply, mirroring the core `ProjectCommand` set (minus
/// setup completion, which is its own atomic command).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MutationCommand {
    SaveFraming {
        objective: String,
        desired_outcome: String,
        phase: Option<String>,
    },
    SetStatus {
        status: ProjectStatus,
        reason: Option<String>,
        review_at: Option<String>,
    },
    SetCommitment {
        text: String,
    },
    SetCommitmentFromTask {
        text: String,
        source_task_id: String,
        adopted_from_proposal_id: Option<String>,
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
    Undo {
        transition_id: CommitmentTransitionId,
    },
}

/// A Human mutation request: which project, at which expected revision, doing what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMutationInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    #[serde(flatten)]
    pub command: MutationCommand,
}

/// `complete_project_setup` input: the atomic first-framing-plus-first-commitment command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteProjectSetupInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub objective: String,
    pub desired_outcome: String,
    pub phase: Option<String>,
    pub first_commitment: String,
}

// ---------------------------------------------------------------------------
// Pure assemblers
// ---------------------------------------------------------------------------

/// The current commitment, if any, derived from Human state + transition history.
pub fn current_commitment_dto(state: &ProjectStateDoc) -> Option<CurrentCommitmentDto> {
    let work_item_id = state.current_next_action_id.as_ref()?;
    let item: &WorkItem = state
        .work_items
        .iter()
        .find(|item| &item.id == work_item_id)?;
    // Last confirmation for this specific item, if any.
    let confirmed_at = state
        .commitment_transitions
        .iter()
        .filter(|transition| transition.kind == CommitmentTransitionKind::Confirmed)
        .filter(|transition| transition.previous_work_item_id.as_ref() == Some(work_item_id))
        .map(|transition| transition.occurred_at.clone())
        .next_back();
    Some(CurrentCommitmentDto {
        work_item_id: work_item_id.clone(),
        text: item.text.clone(),
        status: item.status,
        set_at: item.created_at.clone(),
        confirmed_at,
    })
}

/// The most-recent transitions, newest first, capped for the rail.
fn recent_transition_dtos(state: &ProjectStateDoc, cap: usize) -> Vec<CommitmentTransitionDto> {
    state
        .commitment_transitions
        .iter()
        .rev()
        .take(cap)
        .map(CommitmentTransitionDto::from)
        .collect()
}

/// The id the UI may offer as "Undo", or `None`. Mirrors the core Undo guard: only the
/// single newest transition is undoable, it must not itself be a correction, and its
/// document revision must match the current revision (i.e. it produced this revision).
pub fn undoable_transition_id(state: &ProjectStateDoc) -> Option<CommitmentTransitionId> {
    let last = state.commitment_transitions.last()?;
    if last.kind == CommitmentTransitionKind::Correction {
        return None;
    }
    if last.document_revision != state.revision {
        return None;
    }
    Some(last.id.clone())
}

/// Assemble one dense Index row.
pub fn assemble_index_item(
    record: &ProjectRecord,
    state: &ProjectStateDoc,
    source: Option<&ProjectSource>,
    reasons: &[ReviewReason],
    observed_actual: Option<ObservedActualDto>,
) -> ProjectIndexItemDto {
    ProjectIndexItemDto {
        project_id: record.id.clone(),
        name: record.name.clone(),
        status: state.status,
        current_commitment: current_commitment_dto(state),
        observed_actual,
        review_reasons: review_reason_dtos(reasons),
        source_status: source
            .map(|source| source.status)
            .unwrap_or(ProjectSourceStatus::Missing),
        revision: state.revision,
        source_revision: source.map(|source| source.revision).unwrap_or(0),
    }
}

/// Assemble the full Overview shared by Peek and full-page.
pub fn assemble_overview(
    record: &ProjectRecord,
    state: &ProjectStateDoc,
    source: Option<&ProjectSource>,
    reasons: &[ReviewReason],
    observed_actual: Option<ObservedActualDto>,
) -> ProjectOverviewDto {
    let recent_transitions = recent_transition_dtos(state, 12);
    let last_transition = state
        .commitment_transitions
        .last()
        .map(CommitmentTransitionDto::from);
    ProjectOverviewDto {
        project_id: record.id.clone(),
        name: record.name.clone(),
        created_at: record.created_at.clone(),
        status: state.status,
        status_reason: state.status_reason.clone(),
        phase: state.phase.clone(),
        objective: state.objective.clone(),
        desired_outcome: state.desired_outcome.clone(),
        review_at: state.review_at.clone(),
        source: source.map(ProjectSourceDto::from),
        current_commitment: current_commitment_dto(state),
        observed_actual,
        review_reasons: review_reason_dtos(reasons),
        recent_transitions,
        last_transition,
        undoable_transition_id: undoable_transition_id(state),
        review_policy: ReviewPolicyDto::r0(),
        revision: state.revision,
    }
}

/// Deterministic factual-attention order. Active/setup projects with observable
/// activity are ordered by silent days (most silent first); an empty repository is
/// treated as maximally silent. Unobservable sources follow rather than receiving a
/// fabricated inactivity value. Non-operating lifecycle states remain last.
pub fn index_sort_key(item: &ProjectIndexItemDto) -> (u8, u32, String, String) {
    let operating = matches!(item.status, ProjectStatus::Setup | ProjectStatus::Active);
    let observable =
        item.source_status == ProjectSourceStatus::Available && item.observed_actual.is_some();
    let bucket = if operating && observable {
        0
    } else if operating {
        1
    } else {
        2
    };
    let silent = item
        .observed_actual
        .as_ref()
        .map(|actual| actual.silent_days.unwrap_or(u32::MAX))
        .unwrap_or(0);
    (
        bucket,
        u32::MAX - silent,
        item.name.clone(),
        item.project_id.as_str().to_owned(),
    )
}
