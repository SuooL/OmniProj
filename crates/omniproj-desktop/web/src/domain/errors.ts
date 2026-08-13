// The fixed error contract, mirroring `crates/omniproj-desktop/src/error.rs`, plus the
// browser-side classification into a recovery affordance. The two facts the UI must never
// confuse:
//   - a durable mutation whose audit failed (`state_applied: true`) must be REFETCHED and
//     never resent;
//   - a `revision_conflict` must be refetched-and-rebuilt, not retried verbatim.
// An unknown/unexpected rejection is flattened to a safe generic message — a raw error or
// stack is never surfaced to the UI.

export type ErrorCode =
  | "project_not_found"
  | "invalid_input"
  | "invalid_path"
  | "source_missing"
  | "source_unreadable"
  | "not_git_repository"
  | "bare_repository"
  | "duplicate_source"
  | "source_observation_failed"
  | "store_read_failed"
  | "store_write_failed"
  | "audit_commit_failed"
  | "revision_conflict"
  | "current_commitment_exists"
  | "no_current_commitment"
  | "current_commitment_changed"
  | "reason_required"
  | "transition_not_found"
  | "undo_not_available"
  | "undo_conflict";

/** The serialized backend error, exactly as it crosses the IPC boundary. */
export interface CommandError {
  code: ErrorCode;
  message: string;
  retryable: boolean;
  state_applied: boolean;
  field?: string;
  project_id?: string;
  existing_project_id?: string;
  durable_revision?: number;
}

/** How the UI may recover from an error. */
export type Recovery = "retry" | "refetch" | "none";

const ERROR_CODES: ReadonlySet<string> = new Set<ErrorCode>([
  "project_not_found",
  "invalid_input",
  "invalid_path",
  "source_missing",
  "source_unreadable",
  "not_git_repository",
  "bare_repository",
  "duplicate_source",
  "source_observation_failed",
  "store_read_failed",
  "store_write_failed",
  "audit_commit_failed",
  "revision_conflict",
  "current_commitment_exists",
  "no_current_commitment",
  "current_commitment_changed",
  "reason_required",
  "transition_not_found",
  "undo_not_available",
  "undo_conflict",
]);

const GENERIC_MESSAGE = "Something went wrong. Please try again.";

/** The single error type the UI works with. Carries the typed code when known. */
export class AppError extends Error {
  readonly code: ErrorCode | "unknown";
  readonly retryable: boolean;
  /** True only for `audit_commit_failed`: the change is durable, the UI must refetch. */
  readonly stateApplied: boolean;
  readonly field?: string;
  readonly projectId?: string;
  readonly existingProjectId?: string;
  readonly durableRevision?: number;
  /** The recovery affordance the UI should offer. */
  readonly recovery: Recovery;

  constructor(init: {
    code: ErrorCode | "unknown";
    message: string;
    retryable: boolean;
    stateApplied: boolean;
    field?: string;
    projectId?: string;
    existingProjectId?: string;
    durableRevision?: number;
  }) {
    super(init.message);
    this.name = "AppError";
    this.code = init.code;
    this.retryable = init.retryable;
    this.stateApplied = init.stateApplied;
    this.field = init.field;
    this.projectId = init.projectId;
    this.existingProjectId = init.existingProjectId;
    this.durableRevision = init.durableRevision;
    this.recovery = deriveRecovery(init.code, init.retryable, init.stateApplied);
  }
}

function deriveRecovery(
  code: ErrorCode | "unknown",
  retryable: boolean,
  stateApplied: boolean,
): Recovery {
  // A durable-but-unaudited change must never be resent — reload it.
  if (stateApplied) return "refetch";
  // A stale revision must be refetched and the request rebuilt, not retried verbatim.
  if (code === "revision_conflict") return "refetch";
  if (retryable) return "retry";
  return "none";
}

function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.code === "string" &&
    ERROR_CODES.has(candidate.code) &&
    typeof candidate.message === "string" &&
    typeof candidate.retryable === "boolean" &&
    typeof candidate.state_applied === "boolean"
  );
}

/** Normalize any thrown IPC rejection into a typed `AppError`, never leaking internals. */
export function classifyError(raw: unknown): AppError {
  if (raw instanceof AppError) return raw;
  if (isCommandError(raw)) {
    return new AppError({
      code: raw.code,
      message: raw.message,
      retryable: raw.retryable,
      stateApplied: raw.state_applied,
      field: raw.field,
      projectId: raw.project_id,
      existingProjectId: raw.existing_project_id,
      durableRevision: raw.durable_revision,
    });
  }
  return new AppError({
    code: "unknown",
    message: GENERIC_MESSAGE,
    retryable: false,
    stateApplied: false,
  });
}
