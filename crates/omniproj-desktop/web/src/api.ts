// The typed thin client over the R0 desktop backend (Tauri IPC). Exactly the 15 approved
// commands, each invoked with a single top-level `input` argument whose fields are
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
  Task,
  TimelineCommit,
} from "./domain/project";

/** Invoke one command, wrapping args in the single `input` key and typing the rejection. */
async function call<T>(command: string, input?: object): Promise<T> {
  try {
    return input === undefined
      ? await invoke<T>(command)
      : await invoke<T>(command, { input });
  } catch (raw) {
    throw classifyError(raw);
  }
}

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

  getTasks: (project_id: ProjectId) => call<Task[]>("get_tasks", { project_id }),
  getAttentionSummary: () => call<{ count: number; project_ids: ProjectId[] }>("get_attention_summary"),
  addTask: (input: { project_id: ProjectId; text: string; unclear: boolean }) => call<Task[]>("add_task", input),
  updateTask: (input: { project_id: ProjectId; id: string; status: string; due: string | null; note: string | null }) => call<Task[]>("update_task", input),
  removeTask: (input: { project_id: ProjectId; id: string }) => call<Task[]>("remove_task", input),
  getCommitTimeline: (project_id: ProjectId, limit = 50) => call<TimelineCommit[]>("get_commit_timeline", { project_id, limit }),
  attributeCommit: (input: { project_id: ProjectId; id: string; sha: string }) => call<Task[]>("attribute_commit", input),
  unattributeCommit: (input: { project_id: ProjectId; id: string; sha: string }) => call<Task[]>("unattribute_commit", input),
  advanceTask: (input: { project_id: ProjectId; id: string }) => call<string[]>("advance_task", input),
  adoptSubtasks: (input: { project_id: ProjectId; texts: string[] }) => call<Task[]>("adopt_subtasks", input),
} as const;

export { AppError };
