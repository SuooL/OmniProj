// The typed thin client over the reviewed desktop backend (Tauri IPC). Commands are
// invoked with a single top-level `input` argument whose fields are
// snake_case — matching `crates/omniproj-desktop/src/commands.rs`. Pull-only: nothing here
// polls or pushes. Every rejection is normalized into a typed `AppError`.

import { invoke } from "@tauri-apps/api/core";

import { AppError, classifyError } from "./domain/errors";
import type {
  ClearCommitmentInput,
  CompleteCommitmentInput,
  CompleteProjectSetupInput,
  ConfirmCommitmentInput,
  ProjectId,
  ProjectIndexResponse,
  ProjectOverview,
  RefreshResult,
  RegisterProjectInput,
  RelinkProjectInput,
  ReplaceCommitmentInput,
  SaveProjectFramingInput,
  SetCommitmentInput,
  SetProjectStatusInput,
  SourceValidation,
  UndoCommitmentTransitionInput,
  TaskList,
  AdvanceProposal,
  TimelineCommit,
  GraphCommit,
  PlanList,
  ReminderSettings,
  DogfoodSummary,
  AgentSettings,
} from "./domain/project";

/** Invoke one command, wrapping args in the single `input` key and typing the rejection. */
async function call<T>(command: string, input?: object): Promise<T> {
  try {
    const result = input === undefined
      ? await invoke<T>(command)
      : await invoke<T>(command, { input });
    if (ATTENTION_MUTATIONS.has(command)) {
      // The domain mutation already succeeded. Indicator refresh is derived UI state
      // and must never turn a durable success into a displayed write failure.
      try { await invoke("refresh_attention_indicator"); } catch { /* next hourly/startup sync repairs it */ }
    }
    return result;
  } catch (raw) {
    throw classifyError(raw);
  }
}

const ATTENTION_MUTATIONS = new Set([
  "register_project", "relink_project_source", "refresh_projects",
  "complete_project_setup", "set_project_status", "set_commitment",
  "confirm_commitment", "complete_commitment", "replace_commitment",
  "clear_commitment", "undo_commitment_transition", "add_task",
  "update_task", "remove_task", "set_reminder_settings",
]);

export const api = {
  // --- Reads ---------------------------------------------------------------
  listProjectIndex: () => call<ProjectIndexResponse>("list_project_index"),
  getProjectOverview: (project_id: ProjectId) =>
    call<ProjectOverview>("get_project_overview", { project_id }),
  validateProjectSource: (location: string) =>
    call<SourceValidation>("validate_project_source", { location }),

  // --- Source lifecycle ----------------------------------------------------
  registerProject: (input: RegisterProjectInput) =>
    call<ProjectOverview>("register_project", input),
  relinkProjectSource: (input: RelinkProjectInput) =>
    call<ProjectOverview>("relink_project_source", input),
  refreshProjects: (project_ids: ProjectId[] | null) =>
    call<RefreshResult[]>("refresh_projects", { project_ids }),

  // --- Setup + framing -----------------------------------------------------
  completeProjectSetup: (input: CompleteProjectSetupInput) =>
    call<ProjectOverview>("complete_project_setup", input),
  saveProjectFraming: (input: SaveProjectFramingInput) =>
    call<ProjectOverview>("save_project_framing", input),
  setProjectStatus: (input: SetProjectStatusInput) =>
    call<ProjectOverview>("set_project_status", input),

  // --- Commitment lifecycle ------------------------------------------------
  setCommitment: (input: SetCommitmentInput) =>
    call<ProjectOverview>("set_commitment", input),
  confirmCommitment: (input: ConfirmCommitmentInput) =>
    call<ProjectOverview>("confirm_commitment", input),
  completeCommitment: (input: CompleteCommitmentInput) =>
    call<ProjectOverview>("complete_commitment", input),
  replaceCommitment: (input: ReplaceCommitmentInput) =>
    call<ProjectOverview>("replace_commitment", input),
  clearCommitment: (input: ClearCommitmentInput) =>
    call<ProjectOverview>("clear_commitment", input),
  undoCommitmentTransition: (input: UndoCommitmentTransitionInput) =>
    call<ProjectOverview>("undo_commitment_transition", input),

  getTasks: (project_id: ProjectId) => call<TaskList>("get_tasks", { project_id }),
  getAttentionSummary: () => call<{ count: number; project_ids: ProjectId[] }>("get_attention_summary"),
  addTask: (input: { project_id: ProjectId; expected_revision: string; text: string; unclear: boolean }) => call<TaskList>("add_task", input),
  updateTask: (input: { project_id: ProjectId; expected_revision: string; id: string; status: string; due: string | null; note: string | null }) => call<TaskList>("update_task", input),
  removeTask: (input: { project_id: ProjectId; expected_revision: string; id: string }) => call<TaskList>("remove_task", input),
  getCommitTimeline: (project_id: ProjectId, limit = 50) => call<TimelineCommit[]>("get_commit_timeline", { project_id, limit }),
  getGitGraph: (project_id: ProjectId, limit = 40) => call<GraphCommit[]>("get_git_graph", { project_id, limit }),
  attributeCommit: (input: { project_id: ProjectId; expected_revision: string; id: string; sha: string }) => call<TaskList>("attribute_commit", input),
  unattributeCommit: (input: { project_id: ProjectId; expected_revision: string; id: string; sha: string }) => call<TaskList>("unattribute_commit", input),
  advanceTask: (input: { project_id: ProjectId; id: string }) => call<AdvanceProposal>("advance_task", input),
  adoptSubtasks: (input: { project_id: ProjectId; expected_revision: string; proposal_id: string; texts: string[] }) => call<TaskList>("adopt_subtasks", input),
  promoteTaskToCommitment: (input: { project_id: ProjectId; task_id: string; expected_task_revision: string; expected_project_revision: number }) => call<ProjectOverview>("promote_task_to_commitment", input),
  getPlan: (project_id: ProjectId) => call<PlanList>("get_plan", { project_id }),
  addPlanEntry: (input: { project_id: ProjectId; expected_revision: string; title: string; body: string }) => call<PlanList>("add_plan_entry", input),
  setPlanStatus: (input: { project_id: ProjectId; expected_revision: string; id: string; status: string }) => call<PlanList>("set_plan_status", input),
  setPlanCommit: (input: { project_id: ProjectId; expected_revision: string; id: string; commit: string | null }) => call<PlanList>("set_plan_commit", input),
  getReminderSettings: () => call<ReminderSettings>("get_reminder_settings"),
  setReminderSettings: (settings: ReminderSettings) => call<ReminderSettings>("set_reminder_settings", { settings }),
  testReminder: () => call<void>("test_reminder"),
  getDogfoodSummary: () => call<DogfoodSummary>("get_dogfood_summary"),
  recordReentryEvent: (input: { project_id: ProjectId; duration_seconds: number }) => call<DogfoodSummary>("record_reentry_event", input),
  getAgentSettings: () => call<AgentSettings>("get_agent_settings"),
  setAgentSettings: (input: { default_model: string; api_key: string | null; remote_consent: boolean }) => call<AgentSettings>("set_agent_settings", input),
  testAgentProvider: () => call<void>("test_agent_provider"),
} as const;

export { AppError };
