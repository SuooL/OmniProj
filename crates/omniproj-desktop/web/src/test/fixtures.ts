// Deterministic fixtures mirroring Task 7's DTO snapshots. Factory functions take overrides
// so tests state only what they care about.

import type {
  CommitmentTransition,
  CurrentCommitment,
  ObservedActual,
  ProjectIndexItem,
  ProjectIndexResponse,
  ProjectOverview,
  ProjectSource,
  ReviewPolicy,
  ReviewReason,
  ReviewReasonCode,
} from "../domain/project";
import {
  projectId,
  transitionId,
  workItemId,
} from "../domain/project";

export const reviewPolicy: ReviewPolicy = {
  commitment_review_days: 7,
  rule_version: "r0-v1",
};

const REASON_LABELS: Record<ReviewReasonCode, string> = {
  source_unavailable: "Source unavailable",
  complete_setup: "Complete setup",
  needs_commitment: "Needs commitment",
  review_action: "Review action",
  scheduled_review: "Scheduled review",
};

export function reviewReason(
  code: ReviewReasonCode,
  evidence: string[] = [],
): ReviewReason {
  return {
    code,
    label: REASON_LABELS[code],
    evidence,
    rule_version: "r0-v1",
  };
}

export function observedActual(
  overrides: Partial<ObservedActual> = {},
): ObservedActual {
  return {
    observed_at: "2026-08-12T09:00:00Z",
    head: { kind: "attached", branch: "main" },
    last_commit: {
      sha: "a".repeat(40),
      short_sha: "aaaaaaa",
      subject: "initial",
      committed_at: "2026-08-01T00:00:00Z",
    },
    changed_files: 0,
    staged_files: 0,
    unstaged_files: 0,
    untracked_files: 0,
    status_digest: "0123456789abcdef",
    commits_since_commitment: null,
    commit_activity_weeks: [0, 0, 1, 0, 2, 0, 0, 1, 0, 0, 3, 0, 1, 0, 0, 2],
    silent_days: 11,
    ...overrides,
  };
}

export function currentCommitment(
  overrides: Partial<CurrentCommitment> = {},
): CurrentCommitment {
  return {
    work_item_id: workItemId("work-1"),
    text: "Wire the service",
    status: "doing",
    set_at: "2026-08-10T12:00:00Z",
    confirmed_at: null,
    ...overrides,
  };
}

export function commitmentTransition(
  overrides: Partial<CommitmentTransition> = {},
): CommitmentTransition {
  return {
    id: transitionId("transition-1"),
    type: "set",
    previous_work_item_id: null,
    next_work_item_id: workItemId("work-1"),
    reason: null,
    occurred_at: "2026-08-10T12:00:00Z",
    corrects_transition_id: null,
    ...overrides,
  };
}

export function projectSource(
  overrides: Partial<ProjectSource> = {},
): ProjectSource {
  return {
    source_id: "source-1" as ProjectSource["source_id"],
    kind: "git_repo",
    location: "/Users/dev/projects/omni",
    is_primary: true,
    status: "available",
    last_observed_at: "2026-08-12T09:00:00Z",
    last_successful_refresh_at: "2026-08-12T09:00:00Z",
    last_error_category: null,
    revision: 1,
    ...overrides,
  };
}

export function indexItem(
  overrides: Partial<ProjectIndexItem> = {},
): ProjectIndexItem {
  return {
    project_id: projectId("project-1"),
    name: "Omni",
    status: "active",
    current_commitment: currentCommitment(),
    observed_actual: observedActual(),
    review_reasons: [],
    source_status: "available",
    revision: 1,
    source_revision: 1,
    ...overrides,
  };
}

export function indexResponse(
  projects: ProjectIndexItem[],
): ProjectIndexResponse {
  return { projects, review_policy: reviewPolicy };
}

export function overview(
  overrides: Partial<ProjectOverview> = {},
): ProjectOverview {
  const set = commitmentTransition();
  return {
    project_id: projectId("project-1"),
    name: "Omni",
    created_at: "2026-08-10T12:00:00Z",
    status: "active",
    status_reason: null,
    phase: null,
    objective: "Ship R0",
    desired_outcome: "Dogfood",
    review_at: null,
    source: projectSource(),
    current_commitment: currentCommitment(),
    observed_actual: observedActual(),
    review_reasons: [],
    recent_transitions: [set],
    last_transition: set,
    undoable_transition_id: set.id,
    review_policy: reviewPolicy,
    revision: 1,
    ...overrides,
  };
}
