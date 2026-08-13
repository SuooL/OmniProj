// Explicit lifecycle changes. Each target state has its own required inputs, enforced before
// Save: waiting needs a reason and a review date; parked needs a reason and allows an optional
// review date; archived needs confirmation; returning to active carries no reason and never
// fabricates a current commitment. Unsaved reason/review-date input is retained on failure.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview, ProjectStatus } from "../../domain/project";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";

const TARGETS: Array<{ value: ProjectStatus; label: string }> = [
  { value: "active", label: "Active" },
  { value: "waiting", label: "Waiting" },
  { value: "parked", label: "Parked" },
  { value: "archived", label: "Archived" },
];

function toReviewAt(date: string): string | null {
  return date.trim() === "" ? null : `${date}T00:00:00Z`;
}

export interface ProjectLifecycleControlProps {
  overview: ProjectOverview;
}

export function ProjectLifecycleControl({ overview }: ProjectLifecycleControlProps) {
  const mutation = useOverviewMutation();
  const [target, setTarget] = useState<ProjectStatus>(overview.status);
  const [reason, setReason] = useState(overview.status_reason ?? "");
  const [reviewDate, setReviewDate] = useState("");
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [failed, setFailed] = useState(false);

  const needsReason = target === "waiting" || target === "parked";
  const needsReviewDate = target === "waiting"; // required for waiting, optional for parked
  const needsConfirm = target === "archived";

  const canSave =
    !mutation.pending &&
    target !== overview.status &&
    (!needsReason || reason.trim() !== "") &&
    (!needsReviewDate || reviewDate.trim() !== "") &&
    (!needsConfirm || confirmArchive);

  async function save() {
    setFailed(false);
    const result = await mutation.run(
      overview.project_id,
      () =>
        api.setProjectStatus({
          project_id: overview.project_id,
          expected_revision: overview.revision,
          status: target,
          reason: needsReason ? reason.trim() : null,
          review_at: target === "waiting" || target === "parked" ? toReviewAt(reviewDate) : null,
        }),
      "Project status updated.",
    );
    // On success the input is done; on any failure keep the reason/date so nothing is lost.
    if (result.status !== "success") setFailed(true);
  }

  return (
    <section aria-labelledby="lifecycle-heading" data-testid="lifecycle-control">
      <h3 id="lifecycle-heading">Lifecycle</h3>
      <label>
        Set status
        <select
          aria-label="Set status"
          value={target}
          onChange={(e) => setTarget(e.target.value as ProjectStatus)}
        >
          {TARGETS.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </select>
      </label>

      {needsReason && (
        <label>
          Reason
          <input aria-label="Status reason" value={reason} onChange={(e) => setReason(e.target.value)} />
        </label>
      )}
      {(target === "waiting" || target === "parked") && (
        <label>
          Review date{needsReviewDate ? "" : " (optional)"}
          <input
            aria-label="Review date"
            type="date"
            value={reviewDate}
            onChange={(e) => setReviewDate(e.target.value)}
          />
        </label>
      )}
      {needsConfirm && (
        <label>
          <input
            type="checkbox"
            aria-label="Confirm archive"
            checked={confirmArchive}
            onChange={(e) => setConfirmArchive(e.target.checked)}
          />
          I understand archiving removes this project from the operating index.
        </label>
      )}

      <button type="button" disabled={!canSave} onClick={save}>
        Update status
      </button>
      {failed && mutation.error && (
        <p role="alert" className="op-mutation-error" data-testid="lifecycle-error">
          {mutation.error.message}
        </p>
      )}
    </section>
  );
}
