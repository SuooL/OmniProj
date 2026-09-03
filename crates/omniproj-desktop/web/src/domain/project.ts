// The R0 wire contract, mirroring the Rust DTOs in `crates/omniproj-desktop/src/dto.rs`
// exactly — snake_case field names, snake_case enum values. This is the single place the
// frontend describes the backend shape; nothing here derives review semantics in the
// browser (order and reasons are computed in core).

// --- Branded ids -----------------------------------------------------------
// Opaque strings so an unrelated id can never be passed where a specific one is expected.
declare const brand: unique symbol;
type Branded<Name extends string> = string & { readonly [brand]: Name };

export type ProjectId = Branded<"ProjectId">;
export type ProjectSourceId = Branded<"ProjectSourceId">;
export type WorkItemId = Branded<"WorkItemId">;
export type TransitionId = Branded<"TransitionId">;

export const projectId = (value: string): ProjectId => value as ProjectId;
export const workItemId = (value: string): WorkItemId => value as WorkItemId;
export const transitionId = (value: string): TransitionId => value as TransitionId;

// --- Enums (snake_case values, verbatim from core) -------------------------
export type ProjectStatus = "setup" | "active" | "waiting" | "parked" | "archived";

export type WorkItemStatus =
  | "planned"
  | "doing"
  | "blocked"
  | "done"
  | "abandoned";

export type CommitmentTransitionKind =
  | "set"
  | "confirmed"
  | "completed"
  | "replaced"
  | "cleared"
  | "correction";

export type ProjectSourceKind = "git_repo" | "session" | "document_path";

export type ProjectSourceStatus = "available" | "moved" | "unreadable" | "missing";

export type ReviewReasonCode =
  | "source_unavailable"
  | "complete_setup"
  | "needs_commitment"
  | "overdue_work"
  | "review_action"
  | "scheduled_review";

// --- Value objects ---------------------------------------------------------
export interface ReviewPolicy {
  commitment_review_days: number;
  rule_version: string;
}

export interface ReviewReason {
  code: ReviewReasonCode;
  label: string;
  evidence: string[];
  rule_version: string;
}

export type HeadState =
  | { kind: "attached"; branch: string }
  | { kind: "detached" }
  | { kind: "unborn"; branch: string | null };

export interface Commit {
  sha: string;
  short_sha: string;
  subject: string;
  committed_at: string;
}

export interface ObservedActual {
  observed_at: string;
  head: HeadState;
  last_commit: Commit | null;
  changed_files: number;
  staged_files: number;
  unstaged_files: number;
  untracked_files: number;
  status_digest: string;
  commits_since_commitment: number | null;
  commit_activity_weeks: number[];
  silent_days: number | null;
}

export interface CurrentCommitment {
  work_item_id: WorkItemId;
  text: string;
  status: WorkItemStatus;
  set_at: string;
  confirmed_at: string | null;
}

export interface CommitmentTransition {
  id: TransitionId;
  type: CommitmentTransitionKind;
  previous_work_item_id: WorkItemId | null;
  next_work_item_id: WorkItemId | null;
  reason: string | null;
  occurred_at: string;
  corrects_transition_id: TransitionId | null;
}

export interface ProjectSource {
  source_id: ProjectSourceId;
  kind: ProjectSourceKind;
  location: string;
  is_primary: boolean;
  status: ProjectSourceStatus;
  last_observed_at: string | null;
  last_successful_refresh_at: string | null;
  last_error_category: string | null;
  revision: number;
}

// --- Index (dense operating index) -----------------------------------------
export interface ProjectIndexItem {
  project_id: ProjectId;
  name: string;
  status: ProjectStatus;
  current_commitment: CurrentCommitment | null;
  observed_actual: ObservedActual | null;
  review_reasons: ReviewReason[];
  source_status: ProjectSourceStatus;
  revision: number;
  source_revision: number;
}

export interface ProjectIndexResponse {
  projects: ProjectIndexItem[];
  review_policy: ReviewPolicy;
}

// --- Overview --------------------------------------------------------------
export interface ProjectOverview {
  project_id: ProjectId;
  name: string;
  created_at: string;
  status: ProjectStatus;
  status_reason: string | null;
  phase: string | null;
  objective: string | null;
  desired_outcome: string | null;
  review_at: string | null;
  source: ProjectSource | null;
  current_commitment: CurrentCommitment | null;
  observed_actual: ObservedActual | null;
  review_reasons: ReviewReason[];
  recent_transitions: CommitmentTransition[];
  last_transition: CommitmentTransition | null;
  undoable_transition_id: TransitionId | null;
  review_policy: ReviewPolicy;
  revision: number;
}

export interface Task {
  id: string;
  text: string;
  status: "open" | "doing" | "done";
  unclear: boolean;
  due: string | null;
  note: string | null;
  tags: string[];
  commits: string[];
  adopted_from_proposal_id: string | null;
  /** History only: this task has been the commitment at some point. Never gate editing on it. */
  was_committed: boolean;
  is_current_commitment: boolean;
  /** RFC3339 instant of the last mutation, for deterministic board ordering. */
  updated_at: string;
}
export interface TaskList { revision: string; tasks: Task[]; }

// Cross-project focus strip (R1e): read-only aggregate of overdue + due-today tasks.
export interface FocusItem { id: string; text: string; due: string; overdue_days: number; }
export interface FocusProject { project_id: ProjectId; name: string; items: FocusItem[]; }
export interface FocusAgenda { total_items: number; projects: FocusProject[]; }
export interface AdvanceProposal { proposal_id: string; candidates: string[]; }

export interface TimelineCommit {
  sha: string;
  short_sha: string;
  committed_at: string;
  author: string;
  subject: string;
  attributed_task_ids: string[];
}
export interface GraphCommit { sha: string; short_sha: string; parents: string[]; refs: string[]; committed_at: string; author: string; subject: string; }

export type PlanStatus = "planned" | "doing" | "done" | "abandoned";
export interface PlanEntry { id: string | null; date: string; title: string; status: PlanStatus; commit: string | null; body: string; }
export interface PlanList { revision: string; entries: PlanEntry[]; }
export interface ReminderSettings { enabled: boolean; cadence: "daily" | "off"; silent_days_threshold: number; revision: string; }
export interface DogfoodSummary { event_count: number; project_count: number; median_duration_seconds: number | null; meets_event_threshold: boolean; meets_project_threshold: boolean; }
export interface AgentProvider { name: string; kind: string; local: boolean; key_required: boolean; key_present: boolean; }
export interface AgentSettings { default_model: string; selected_provider: string; selected_model: string; remote_consent: boolean; ready: boolean; providers: AgentProvider[]; }

// --- Source validation ------------------------------------------------------
// `validate_project_source` returns a typed state for BOTH the valid preview (`ok`) and
// the recoverable rejections (missing / unreadable / non-Git / bare / observation failed /
// duplicate). Only unexpected/internal failures reject as a `CommandError`.
export type SourceValidation =
  | { state: "ok"; location: string; head: HeadState; last_commit: Commit | null }
  | { state: "missing"; location: string }
  | { state: "unreadable"; location: string }
  | { state: "not_git_repository"; location: string }
  | { state: "bare_repository"; location: string }
  | { state: "observation_failed"; location: string; message: string }
  | {
      state: "duplicate";
      location: string;
      existing_project_id: ProjectId;
      existing_name: string;
    };

// --- Refresh ---------------------------------------------------------------
export type RefreshOutcome =
  | "refreshed"
  | "refresh_in_progress"
  | "source_failed"
  | "stale";

export interface RefreshResult {
  project_id: ProjectId;
  outcome: RefreshOutcome;
  item: ProjectIndexItem | null;
  error_category?: string;
}

// --- Command input DTOs (each command takes one top-level `input`) ----------
export interface RegisterProjectInput {
  location: string;
  name: string;
}

export interface RelinkProjectInput {
  project_id: ProjectId;
  expected_source_revision: number;
  expected_location: string;
  new_location: string;
}

export interface CompleteProjectSetupInput {
  project_id: ProjectId;
  expected_revision: number;
  objective: string;
  desired_outcome: string;
  phase?: string | null;
  first_commitment: string;
}

export interface SaveProjectFramingInput {
  project_id: ProjectId;
  expected_revision: number;
  objective: string;
  desired_outcome: string;
  phase?: string | null;
}

export interface SetProjectStatusInput {
  project_id: ProjectId;
  expected_revision: number;
  status: ProjectStatus;
  reason?: string | null;
  review_at?: string | null;
}

export interface SetCommitmentInput {
  project_id: ProjectId;
  expected_revision: number;
  text: string;
}

export interface ConfirmCommitmentInput {
  project_id: ProjectId;
  expected_revision: number;
  work_item_id: WorkItemId;
}

export interface CompleteCommitmentInput {
  project_id: ProjectId;
  expected_revision: number;
  work_item_id: WorkItemId;
}

export interface ReplaceCommitmentInput {
  project_id: ProjectId;
  expected_revision: number;
  previous_work_item_id: WorkItemId;
  text: string;
  reason: string;
}

export interface ClearCommitmentInput {
  project_id: ProjectId;
  expected_revision: number;
  work_item_id: WorkItemId;
  reason?: string | null;
}

export interface UndoCommitmentTransitionInput {
  project_id: ProjectId;
  expected_revision: number;
  transition_id: TransitionId;
}
