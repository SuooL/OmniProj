// Explicit lifecycle changes. Each target state has its own required inputs, enforced before
// Save: waiting needs a reason and a review date; parked needs a reason and allows an optional
// review date; archived needs confirmation; returning to active carries no reason and never
// fabricates a current commitment. Unsaved reason/review-date input is retained on failure.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview, ProjectStatus } from "../../domain/project";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";
import { localizeError, projectStatusLabel, useI18n } from "../../i18n/I18nProvider";

function toReviewAt(date: string): string | null {
  return date.trim() === "" ? null : `${date}T00:00:00Z`;
}

export interface ProjectLifecycleControlProps {
  overview: ProjectOverview;
}

export function ProjectLifecycleControl({ overview }: ProjectLifecycleControlProps) {
  const { locale, t } = useI18n();
  const targets: ProjectStatus[] = ["active", "waiting", "parked", "archived"];
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
      t("lifecycle.success"),
    );
    // On success the input is done; on any failure keep the reason/date so nothing is lost.
    if (result.status !== "success") setFailed(true);
  }

  return (
    <section className="op-section op-form-section" aria-labelledby="lifecycle-heading" data-testid="lifecycle-control">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("lifecycle.kicker")}</p>
          <h3 id="lifecycle-heading">{t("lifecycle.title")}</h3>
        </div>
      </div>
      <div className="op-form-grid">
        <label className="op-field">
          <span>{t("lifecycle.setStatus")}</span>
          <select
            aria-label={t("lifecycle.setStatus")}
            value={target}
            onChange={(e) => setTarget(e.target.value as ProjectStatus)}
          >
            {targets.map((status) => (
              <option key={status} value={status}>
                {projectStatusLabel(status, locale)}
              </option>
            ))}
          </select>
        </label>

        {needsReason && (
          <label className="op-field op-field--wide">
            <span>{t("lifecycle.reason")}</span>
            <input
              aria-label={t("lifecycle.statusReason")}
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            />
          </label>
        )}
        {(target === "waiting" || target === "parked") && (
          <label className="op-field">
            <span>{needsReviewDate ? t("lifecycle.reviewDate") : t("lifecycle.reviewDateOptional")}</span>
            <input
              aria-label={t("lifecycle.reviewDate")}
              type="date"
              value={reviewDate}
              onChange={(e) => setReviewDate(e.target.value)}
            />
          </label>
        )}
        {needsConfirm && (
          <label className="op-check-field">
            <input
              type="checkbox"
              aria-label={t("lifecycle.confirmArchive")}
              checked={confirmArchive}
              onChange={(e) => setConfirmArchive(e.target.checked)}
            />
            {t("lifecycle.archiveNotice")}
          </label>
        )}
      </div>
      <div className="op-section__footer">
        <button
          className="op-button op-button--secondary"
          type="button"
          disabled={!canSave}
          onClick={save}
        >
          {t("lifecycle.update")}
        </button>
      </div>
      {failed && mutation.error && (
        <p role="alert" className="op-mutation-error" data-testid="lifecycle-error">
          {localizeError(mutation.error, locale)}
        </p>
      )}
    </section>
  );
}
