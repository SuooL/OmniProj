// The one step being worked on, at the head of the list it belongs to. It is not a separate
// surface: there is no way to type a step in here, only to act on the one already marked.

import type { CurrentCommitment } from "../../../domain/project";
import type { MutationOutcome } from "../../../hooks/useOverviewMutation";
import { localizeError, useI18n } from "../../../i18n/I18nProvider";

export interface NowDoingCardProps {
  commitment: CurrentCommitment | null;
  /** False for a `set`, whose undo abandons the item — see TaskBoard. */
  canUndo: boolean;
  pending: boolean;
  outcome: MutationOutcome | null;
  onConfirm: () => void;
  onComplete: () => void;
  onSwitchAway: () => void;
  onUndo: () => void;
  onRetry: () => void;
}

export function NowDoingCard({
  commitment,
  canUndo,
  pending,
  outcome,
  onConfirm,
  onComplete,
  onSwitchAway,
  onUndo,
  onRetry,
}: NowDoingCardProps) {
  const { locale, t } = useI18n();

  return (
    <div className="op-now-doing" data-testid="now-doing">
      <p className="op-section__kicker">{t("task.currentCommitment")}</p>
      {commitment
        ? <p className="op-now-doing__text">{commitment.text}</p>
        : <p className="op-muted">{t("task.nowDoingEmpty")}</p>}

      <div className="op-task-actions">
        {commitment && commitment.confirmed_at === null && (
          <button className="op-button op-button--secondary" type="button" disabled={pending} onClick={onConfirm}>
            {t("task.stillThis")}
          </button>
        )}
        {commitment && (
          <button className="op-button op-button--primary" type="button" disabled={pending} onClick={onComplete}>
            {t("task.complete")}
          </button>
        )}
        {commitment && (
          <button className="op-button op-button--ghost" type="button" disabled={pending} title={t("task.switchAwayHint")} onClick={onSwitchAway}>
            {t("task.switchAway")}
          </button>
        )}
        {canUndo && (
          <button className="op-button op-button--ghost" type="button" data-testid="undo-button" disabled={pending} onClick={onUndo}>
            {t("task.undo")}
          </button>
        )}
      </div>

      {outcome?.status === "durable_audit_failed" && (
        <p role="status" className="op-mutation-note" data-testid="audit-failed-note">{t("commitment.auditFailed")}</p>
      )}
      {outcome?.status === "conflict" && (
        <p role="alert" className="op-mutation-error" data-testid="conflict-note">{t("commitment.conflict")}</p>
      )}
      {outcome?.status === "error" && (
        <div role="alert" className="op-mutation-error" data-testid="write-error">
          <p>{localizeError(outcome.error, locale)}</p>
          {outcome.error.recovery === "retry" && (
            <button className="op-button op-button--secondary" type="button" disabled={pending} onClick={onRetry}>
              {t("common.retry")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
