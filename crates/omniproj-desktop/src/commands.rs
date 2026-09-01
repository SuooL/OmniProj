//! The reviewed desktop Tauri command surface — each command takes a single
//! top-level `input` argument with snake-case JSON fields. Every command is a thin
//! adapter over `DesktopService`: it deserializes its request DTO, maps it to a service
//! call, and returns a DTO or a `CommandError`. No domain logic lives here.

use serde::Deserialize;
use tauri::{AppHandle, Runtime, State};

use omniproj_core::ids::{CommitmentTransitionId, ProjectId, WorkItemId};
use omniproj_core::project_state::ProjectStatus;

use crate::dto::{
    CompleteProjectSetupInput, MutationCommand, ProjectIndexResponseDto, ProjectMutationInput,
    ProjectOverviewDto, RefreshResultDto, RegisterProjectInput, RelinkProjectInput,
    SourceValidationDto,
};
use crate::error::CommandResult;
use crate::mvp::{TaskListDto, TimelineCommitDto};
use crate::service::{DesktopService, R0Service, SystemClock};

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
pub struct ProjectTaskInput {
    pub project_id: ProjectId,
}

#[tauri::command]
pub fn get_tasks(input: ProjectTaskInput) -> CommandResult<TaskListDto> {
    crate::mvp::get_tasks(input.project_id)
}

#[tauri::command]
pub fn get_attention_summary() -> CommandResult<crate::mvp::AttentionSummaryDto> {
    let settings = crate::mvp::load_reminder_settings();
    Ok(crate::mvp::attention_summary_with_threshold(
        settings.silent_days_threshold,
    ))
}

#[tauri::command]
pub fn refresh_attention_indicator(
    ui: State<'_, crate::AttentionTrayUi>,
) -> CommandResult<crate::mvp::AttentionSummaryDto> {
    Ok(crate::sync_attention_ui(&ui))
}

#[derive(Debug, Deserialize)]
pub struct AddTaskInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub text: String,
    #[serde(default)]
    pub unclear: bool,
}
#[tauri::command]
pub fn add_task(input: AddTaskInput) -> CommandResult<TaskListDto> {
    crate::mvp::add_task(
        input.project_id,
        input.expected_revision,
        input.text,
        input.unclear,
    )
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub id: String,
    pub status: String,
    pub due: Option<String>,
    pub note: Option<String>,
}
#[tauri::command]
pub fn update_task(input: UpdateTaskInput) -> CommandResult<TaskListDto> {
    crate::mvp::update_task(
        input.project_id,
        input.expected_revision,
        input.id,
        input.status,
        input.due,
        input.note,
    )
}

#[derive(Debug, Deserialize)]
pub struct TaskIdInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub id: String,
}
#[tauri::command]
pub fn remove_task(input: TaskIdInput) -> CommandResult<TaskListDto> {
    crate::mvp::remove_task(input.project_id, input.expected_revision, input.id)
}

#[derive(Debug, Deserialize)]
pub struct AttributeCommitInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub id: String,
    pub sha: String,
}
#[tauri::command]
pub fn attribute_commit(input: AttributeCommitInput) -> CommandResult<TaskListDto> {
    crate::mvp::attribute_commit(
        input.project_id,
        input.expected_revision,
        input.id,
        input.sha,
    )
}
#[tauri::command]
pub fn unattribute_commit(input: AttributeCommitInput) -> CommandResult<TaskListDto> {
    crate::mvp::unattribute_commit(
        input.project_id,
        input.expected_revision,
        input.id,
        input.sha,
    )
}

#[derive(Debug, Deserialize)]
pub struct TimelineInput {
    pub project_id: ProjectId,
    #[serde(default = "default_timeline_limit")]
    pub limit: usize,
}
fn default_timeline_limit() -> usize {
    50
}
#[tauri::command]
pub fn get_commit_timeline(input: TimelineInput) -> CommandResult<Vec<TimelineCommitDto>> {
    crate::mvp::get_timeline(input.project_id, input.limit)
}

#[tauri::command]
pub fn get_git_graph(input: TimelineInput) -> CommandResult<Vec<crate::mvp::GraphCommitDto>> {
    crate::mvp::get_graph(input.project_id, input.limit)
}

#[derive(Debug, Deserialize)]
pub struct AdvanceInput {
    pub project_id: ProjectId,
    pub id: String,
}
#[tauri::command]
pub async fn advance_task(input: AdvanceInput) -> CommandResult<crate::mvp::AdvanceProposalDto> {
    crate::mvp::advance_task(input.project_id, input.id).await
}

#[derive(Debug, Deserialize)]
pub struct AdoptInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub proposal_id: String,
    pub texts: Vec<String>,
}
#[tauri::command]
pub fn adopt_subtasks(input: AdoptInput) -> CommandResult<TaskListDto> {
    crate::mvp::adopt_subtasks(
        input.project_id,
        input.expected_revision,
        input.proposal_id,
        input.texts,
    )
}

#[derive(Debug, Deserialize)]
pub struct PromoteTaskInput {
    pub project_id: ProjectId,
    pub task_id: String,
    pub expected_task_revision: String,
    pub expected_project_revision: u64,
}

#[tauri::command]
pub fn promote_task_to_commitment(
    service: State<'_, Service>,
    input: PromoteTaskInput,
) -> CommandResult<ProjectOverviewDto> {
    if input.expected_task_revision != input.expected_project_revision.to_string() {
        return Err(crate::error::CommandError::invalid_input(
            "task and project revisions must match",
        ));
    }
    crate::mvp::promote_work_item_to_commitment(
        &input.project_id,
        &input.task_id,
        &input.expected_task_revision,
    )?;
    service.get_project_overview(input.project_id)
}

#[derive(Debug, Deserialize)]
pub struct PlanInput {
    pub project_id: ProjectId,
}

#[tauri::command]
pub fn get_plan(input: PlanInput) -> CommandResult<crate::mvp::PlanListDto> {
    crate::mvp::get_plan(input.project_id)
}

#[derive(Debug, Deserialize)]
pub struct AddPlanEntryInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[tauri::command]
pub fn add_plan_entry(input: AddPlanEntryInput) -> CommandResult<crate::mvp::PlanListDto> {
    crate::mvp::add_plan_entry(
        input.project_id,
        input.expected_revision,
        input.title,
        input.body,
    )
}

#[derive(Debug, Deserialize)]
pub struct SetPlanStatusInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub id: String,
    pub status: String,
}

#[tauri::command]
pub fn set_plan_status(input: SetPlanStatusInput) -> CommandResult<crate::mvp::PlanListDto> {
    crate::mvp::set_plan_status(
        input.project_id,
        input.expected_revision,
        input.id,
        input.status,
    )
}

#[derive(Debug, Deserialize)]
pub struct SetPlanCommitInput {
    pub project_id: ProjectId,
    pub expected_revision: String,
    pub id: String,
    pub commit: Option<String>,
}

#[tauri::command]
pub fn set_plan_commit(input: SetPlanCommitInput) -> CommandResult<crate::mvp::PlanListDto> {
    crate::mvp::set_plan_commit(
        input.project_id,
        input.expected_revision,
        input.id,
        input.commit,
    )
}

#[tauri::command]
pub fn get_reminder_settings() -> crate::error::CommandResult<crate::mvp::ReminderSettingsDto> {
    Ok(crate::mvp::load_reminder_settings())
}

#[derive(Debug, Deserialize)]
pub struct SetReminderSettingsInput {
    pub settings: crate::mvp::ReminderSettingsDto,
}

#[tauri::command]
pub fn set_reminder_settings(
    input: SetReminderSettingsInput,
) -> crate::error::CommandResult<crate::mvp::ReminderSettingsDto> {
    crate::mvp::save_reminder_settings(input.settings)
}

#[tauri::command]
pub fn test_reminder<R: Runtime>(app: AppHandle<R>) -> CommandResult<()> {
    use tauri_plugin_notification::NotificationExt;
    let settings = crate::mvp::load_reminder_settings();
    let count = crate::mvp::attention_count_with_threshold(settings.silent_days_threshold);
    app.notification()
        .builder()
        .title("OmniProj 待关注提醒")
        .body(format!("有 {} 个项目需要关注。", count))
        .show()
        .map_err(|e| {
            crate::error::CommandError::new(
                crate::error::ErrorCode::StoreWriteFailed,
                e.to_string(),
            )
            .retryable()
        })
}

#[tauri::command]
pub fn get_dogfood_summary() -> CommandResult<crate::mvp::DogfoodSummaryDto> {
    crate::mvp::dogfood_summary()
}

#[derive(Debug, Deserialize)]
pub struct RecordReentryEventInput {
    pub project_id: ProjectId,
    pub duration_seconds: u64,
}

#[tauri::command]
pub fn record_reentry_event(
    input: RecordReentryEventInput,
) -> CommandResult<crate::mvp::DogfoodSummaryDto> {
    crate::mvp::record_reentry_event(input.project_id, input.duration_seconds)
}

#[tauri::command]
pub fn get_agent_settings() -> CommandResult<crate::agent_settings::AgentSettingsDto> {
    crate::agent_settings::get_agent_settings()
}

#[tauri::command]
pub fn set_agent_settings(
    input: crate::agent_settings::SaveAgentSettingsInput,
) -> CommandResult<crate::agent_settings::AgentSettingsDto> {
    crate::agent_settings::save_agent_settings(input)
}

#[tauri::command]
pub async fn test_agent_provider() -> CommandResult<()> {
    crate::agent_settings::test_agent_provider().await
}
