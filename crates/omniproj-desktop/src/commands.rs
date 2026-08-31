//! The R0 Tauri command surface — exactly the 15 approved commands, each taking a single
//! top-level `input` argument with snake-case JSON fields. Every command is a thin
//! adapter over `DesktopService`: it deserializes its request DTO, maps it to a service
//! call, and returns a DTO or a `CommandError`. No domain logic lives here.

use serde::Deserialize;
use tauri::State;

use omniproj_core::ids::{CommitmentTransitionId, ProjectId, WorkItemId};
use omniproj_core::project_state::ProjectStatus;

use crate::dto::{
    CompleteProjectSetupInput, MutationCommand, ProjectIndexResponseDto, ProjectMutationInput,
    ProjectOverviewDto, RefreshResultDto, RegisterProjectInput, RelinkProjectInput,
    SourceValidationDto,
};
use crate::error::CommandResult;
use crate::service::{DesktopService, R0Service, SystemClock};
use crate::mvp::{TaskDto, TimelineCommitDto};

/// The concrete production service type managed by Tauri.
pub type Service = DesktopService<SystemClock>;

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_project_index(service: State<'_, Service>) -> CommandResult<ProjectIndexResponseDto> {
    service.list_project_index()
}

#[derive(Debug, Deserialize)]
pub struct GetProjectOverviewInput {
    pub project_id: ProjectId,
}

#[tauri::command]
pub fn get_project_overview(
    service: State<'_, Service>,
    input: GetProjectOverviewInput,
) -> CommandResult<ProjectOverviewDto> {
    service.get_project_overview(input.project_id)
}

#[derive(Debug, Deserialize)]
pub struct ValidateProjectSourceInput {
    pub location: String,
}

#[tauri::command]
pub async fn validate_project_source(
    service: State<'_, Service>,
    input: ValidateProjectSourceInput,
) -> CommandResult<SourceValidationDto> {
    service.validate_project_source(input.location).await
}

// ---------------------------------------------------------------------------
// Source lifecycle
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn register_project(
    service: State<'_, Service>,
    input: RegisterProjectInput,
) -> CommandResult<ProjectOverviewDto> {
    service.register_project(input).await
}

#[tauri::command]
pub async fn relink_project_source(
    service: State<'_, Service>,
    input: RelinkProjectInput,
) -> CommandResult<ProjectOverviewDto> {
    service.relink_project_source(input).await
}

#[derive(Debug, Deserialize)]
pub struct RefreshProjectsInput {
    #[serde(default)]
    pub project_ids: Option<Vec<ProjectId>>,
}

#[tauri::command]
pub async fn refresh_projects(
    service: State<'_, Service>,
    input: RefreshProjectsInput,
) -> CommandResult<Vec<RefreshResultDto>> {
    service.refresh_projects(input.project_ids).await
}

// ---------------------------------------------------------------------------
// Setup completion + framing
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn complete_project_setup(
    service: State<'_, Service>,
    input: CompleteProjectSetupInput,
) -> CommandResult<ProjectOverviewDto> {
    service.complete_project_setup(input)
}

#[derive(Debug, Deserialize)]
pub struct SaveProjectFramingInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub objective: String,
    pub desired_outcome: String,
    #[serde(default)]
    pub phase: Option<String>,
}

#[tauri::command]
pub fn save_project_framing(
    service: State<'_, Service>,
    input: SaveProjectFramingInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::SaveFraming {
            objective: input.objective,
            desired_outcome: input.desired_outcome,
            phase: input.phase,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct SetProjectStatusInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub status: ProjectStatus,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub review_at: Option<String>,
}

#[tauri::command]
pub fn set_project_status(
    service: State<'_, Service>,
    input: SetProjectStatusInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::SetStatus {
            status: input.status,
            reason: input.reason,
            review_at: input.review_at,
        },
    })
}

// ---------------------------------------------------------------------------
// Commitment lifecycle
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetCommitmentInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub text: String,
}

#[tauri::command]
pub fn set_commitment(
    service: State<'_, Service>,
    input: SetCommitmentInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::SetCommitment { text: input.text },
    })
}

#[derive(Debug, Deserialize)]
pub struct ConfirmCommitmentInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub work_item_id: WorkItemId,
}

#[tauri::command]
pub fn confirm_commitment(
    service: State<'_, Service>,
    input: ConfirmCommitmentInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::ConfirmCommitment {
            work_item_id: input.work_item_id,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct CompleteCommitmentInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub work_item_id: WorkItemId,
}

#[tauri::command]
pub fn complete_commitment(
    service: State<'_, Service>,
    input: CompleteCommitmentInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::CompleteCommitment {
            work_item_id: input.work_item_id,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct ReplaceCommitmentInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub previous_work_item_id: WorkItemId,
    pub text: String,
    pub reason: String,
}

#[tauri::command]
pub fn replace_commitment(
    service: State<'_, Service>,
    input: ReplaceCommitmentInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::ReplaceCommitment {
            previous_work_item_id: input.previous_work_item_id,
            text: input.text,
            reason: input.reason,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct ClearCommitmentInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub work_item_id: WorkItemId,
    #[serde(default)]
    pub reason: Option<String>,
}

#[tauri::command]
pub fn clear_commitment(
    service: State<'_, Service>,
    input: ClearCommitmentInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::ClearCommitment {
            work_item_id: input.work_item_id,
            reason: input.reason,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct UndoCommitmentTransitionInput {
    pub project_id: ProjectId,
    pub expected_revision: u64,
    pub transition_id: CommitmentTransitionId,
}

#[tauri::command]
pub fn undo_commitment_transition(
    service: State<'_, Service>,
    input: UndoCommitmentTransitionInput,
) -> CommandResult<ProjectOverviewDto> {
    service.apply_project_mutation(ProjectMutationInput {
        project_id: input.project_id,
        expected_revision: input.expected_revision,
        command: MutationCommand::Undo {
            transition_id: input.transition_id,
        },
    })
}

// ---------------------------------------------------------------------------
// MVP Record / Advance
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProjectTaskInput { pub project_id: ProjectId }

#[tauri::command]
pub fn get_tasks(input: ProjectTaskInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::get_tasks(input.project_id) }

#[tauri::command]
pub fn get_attention_summary(service: State<'_, Service>) -> CommandResult<crate::mvp::AttentionSummaryDto> {
    let response = service.list_project_index()?;
    let project_ids = response.projects.into_iter().filter(|p| !p.review_reasons.is_empty()).map(|p| p.project_id).collect::<Vec<_>>();
    Ok(crate::mvp::AttentionSummaryDto { count: project_ids.len(), project_ids })
}

#[derive(Debug, Deserialize)]
pub struct AddTaskInput { pub project_id: ProjectId, pub text: String, #[serde(default)] pub unclear: bool }
#[tauri::command]
pub fn add_task(input: AddTaskInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::add_task(input.project_id, input.text, input.unclear) }

#[derive(Debug, Deserialize)]
pub struct UpdateTaskInput { pub project_id: ProjectId, pub id: String, pub status: String, pub due: Option<String>, pub note: Option<String> }
#[tauri::command]
pub fn update_task(input: UpdateTaskInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::update_task(input.project_id, input.id, input.status, input.due, input.note) }

#[derive(Debug, Deserialize)]
pub struct TaskIdInput { pub project_id: ProjectId, pub id: String }
#[tauri::command]
pub fn remove_task(input: TaskIdInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::remove_task(input.project_id, input.id) }

#[derive(Debug, Deserialize)]
pub struct AttributeCommitInput { pub project_id: ProjectId, pub id: String, pub sha: String }
#[tauri::command]
pub fn attribute_commit(input: AttributeCommitInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::attribute_commit(input.project_id, input.id, input.sha) }
#[tauri::command]
pub fn unattribute_commit(input: AttributeCommitInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::unattribute_commit(input.project_id, input.id, input.sha) }

#[derive(Debug, Deserialize)]
pub struct TimelineInput { pub project_id: ProjectId, #[serde(default = "default_timeline_limit")] pub limit: usize }
fn default_timeline_limit() -> usize { 50 }
#[tauri::command]
pub fn get_commit_timeline(input: TimelineInput) -> CommandResult<Vec<TimelineCommitDto>> { crate::mvp::get_timeline(input.project_id, input.limit) }

#[derive(Debug, Deserialize)]
pub struct AdvanceInput { pub project_id: ProjectId, pub id: String }
#[tauri::command]
pub async fn advance_task(input: AdvanceInput) -> CommandResult<Vec<String>> { crate::mvp::advance_task(input.project_id, input.id).await }

#[derive(Debug, Deserialize)]
pub struct AdoptInput { pub project_id: ProjectId, pub texts: Vec<String> }
#[tauri::command]
pub fn adopt_subtasks(input: AdoptInput) -> CommandResult<Vec<TaskDto>> { crate::mvp::adopt_subtasks(input.project_id, input.texts) }
