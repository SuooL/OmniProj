// The typed thin client over the reviewed desktop backend (Tauri IPC). Commands are
// invoked with a single top-level `input` argument whose fields are
// snake_case — matching `crates/omniproj-desktop/src/commands.rs`. Pull-only: nothing here
// polls or pushes. Every rejection is normalized into a typed `AppError`.

import { invoke } from "@tauri-apps/api/core";

import { AppError, classifyError } from "./domain/errors";
import type {
  CompleteProjectSetupInput,
  FocusAgenda,
  ProjectId,
  ProjectIndexResponse,
  ProjectOverview,
  RefreshResult,
  RegisterProjectInput,
  RelinkProjectInput,
  SaveProjectFramingInput,
  SetProjectStatusInput,
  SourceValidation,
  TaskList,
  AdvanceProposal,
  TimelineCommit,
  GraphCommit,
  CommitHeatmap,
  ReminderSettings,
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
  "complete_project_setup", "set_project_status", "add_task",
  "update_task", "move_task", "remove_task", "adopt_subtasks",
  "set_reminder_settings",
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

  getTasks: (project_id: ProjectId) => call<TaskList>("get_tasks", { project_id }),
  getAttentionSummary: () => call<{ count: number; project_ids: ProjectId[] }>("get_attention_summary"),
  getFocusAgenda: () => call<FocusAgenda>("get_focus_agenda"),
  addTask: (input: { project_id: ProjectId; expected_revision: string; text: string; unclear: boolean; parent_id?: string | null; after_id?: string | null }) => call<TaskList>("add_task", input),
  updateTask: (input: { project_id: ProjectId; expected_revision: string; id: string; text?: string | null; status: string; due: string | null; note: string | null; tags?: string[] }) => call<TaskList>("update_task", input),
  /** Indent, outdent and reorder in one call: put `id` under `parent_id`, after `after_id`. */
  moveTask: (input: { project_id: ProjectId; expected_revision: string; id: string; parent_id: string | null; after_id: string | null }) => call<TaskList>("move_task", input),
  removeTask: (input: { project_id: ProjectId; expected_revision: string; id: string }) => call<TaskList>("remove_task", input),
  getCommitTimeline: (project_id: ProjectId, limit = 50) => call<TimelineCommit[]>("get_commit_timeline", { project_id, limit }),
  getGitGraph: (project_id: ProjectId, limit = 40) => call<GraphCommit[]>("get_git_graph", { project_id, limit }),
  getCommitHeatmap: (project_id: ProjectId, days = 371) => call<CommitHeatmap>("get_commit_heatmap", { project_id, days }),
  attributeCommit: (input: { project_id: ProjectId; expected_revision: string; id: string; sha: string }) => call<TaskList>("attribute_commit", input),
  unattributeCommit: (input: { project_id: ProjectId; expected_revision: string; id: string; sha: string }) => call<TaskList>("unattribute_commit", input),
  advanceTask: (input: { project_id: ProjectId; id: string }) => call<AdvanceProposal>("advance_task", input),
  adoptSubtasks: (input: { project_id: ProjectId; expected_revision: string; proposal_id: string; texts: string[]; parent_id?: string | null }) => call<TaskList>("adopt_subtasks", input),
  getReminderSettings: () => call<ReminderSettings>("get_reminder_settings"),
  setReminderSettings: (settings: ReminderSettings) => call<ReminderSettings>("set_reminder_settings", { settings }),
  testReminder: () => call<void>("test_reminder"),
  getAgentSettings: () => call<AgentSettings>("get_agent_settings"),
  setAgentSettings: (input: { default_model: string; api_key: string | null; remote_consent: boolean }) => call<AgentSettings>("set_agent_settings", input),
  testAgentProvider: () => call<void>("test_agent_provider"),
} as const;

export { AppError };
