// The single mutation engine for every Human write on the Project Overview. It centralizes the
// two rules the UI must never get wrong (mirroring domain/errors.ts):
//
//   - success  -> fold the returned Overview into both caches (no refetch wave) + announce politely;
//   - revision_conflict -> refetch the Overview so the form rebuilds on the fresh revision, keep
//     the draft, announce a comparison message (never resend verbatim);
//   - audit_commit_failed (state_applied) -> the change IS durable: refetch the durable revision,
//     announce "State saved; audit commit failed", and NEVER resend the Human mutation.
//
// Drafts live in the calling component; this hook only reports outcome so the component can
// decide whether to clear or retain its draft.

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { AppError, classifyError } from "../domain/errors";
import type { ProjectId, ProjectOverview } from "../domain/project";
import { applyOverviewToCaches } from "../queryClient";
import { queryKeys } from "../queryKeys";
import { useAnnouncer } from "../components/AppShell";

export type MutationOutcome =
  | { status: "success"; overview: ProjectOverview }
  | { status: "durable_audit_failed"; error: AppError }
  | { status: "conflict"; error: AppError }
  | { status: "error"; error: AppError };

export interface OverviewMutation {
  /** Run one command; folds success into caches, classifies + routes failures. */
  run: (
    projectId: ProjectId,
    action: () => Promise<ProjectOverview>,
    successMessage: string,
  ) => Promise<MutationOutcome>;
  pending: boolean;
  /** The last error, or null after a success/reset. */
  error: AppError | null;
  reset: () => void;
}

export function useOverviewMutation(): OverviewMutation {
  const queryClient = useQueryClient();
  const announce = useAnnouncer();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const run = useCallback<OverviewMutation["run"]>(
    async (projectId, action, successMessage) => {
      setPending(true);
      setError(null);
      try {
        const overview = await action();
        applyOverviewToCaches(queryClient, overview);
        announce("polite", successMessage);
        return { status: "success", overview };
      } catch (raw) {
        const err = raw instanceof AppError ? raw : classifyError(raw);
        setError(err);

        if (err.recovery === "refetch") {
          // Reload the durable/current state; the mutation is never resent.
          await queryClient.refetchQueries({
            queryKey: queryKeys.projectOverview(projectId),
          });
          if (err.stateApplied) {
            announce("assertive", "State saved; audit commit failed.");
            return { status: "durable_audit_failed", error: err };
          }
          announce("assertive", "This project changed since you started. Review the latest and resubmit.");
          return { status: "conflict", error: err };
        }

        announce("assertive", err.message);
        return { status: "error", error: err };
      } finally {
        setPending(false);
      }
    },
    [announce, queryClient],
  );

  const reset = useCallback(() => setError(null), []);

  return { run, pending, error, reset };
}
