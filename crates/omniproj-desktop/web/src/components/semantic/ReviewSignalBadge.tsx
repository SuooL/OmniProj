// The single primary review signal for a row. Tone is fixed per deterministic reason code:
// danger = unavailable source; info = observed change (review_action); warning = an action the
// Human still owes (complete_setup, needs_commitment, overdue_work, scheduled_review). Extra
// reasons are a PLAIN, uncontained `+N` whose accessible name enumerates them — never a second
// enclosed badge.

import type { ReviewReasonCode } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";
import { reviewReasonLabel, useI18n } from "../../i18n/I18nProvider";

const REASON_TONE: Record<ReviewReasonCode, StatusTone> = {
  source_unavailable: "danger",
  complete_setup: "warning",
  needs_commitment: "warning",
  overdue_work: "warning",
  review_action: "info",
  scheduled_review: "warning",
};

export interface ReviewSignalBadgeProps {
  reason: { code: ReviewReasonCode; label: string };
  /** The lower-priority reasons folded into `+N`, most-urgent first. */
  hidden?: ReadonlyArray<{ code?: ReviewReasonCode; label: string }>;
}

export function ReviewSignalBadge({ reason, hidden = [] }: ReviewSignalBadgeProps) {
  const { locale, t } = useI18n();
  const count = hidden.length;
  const labels = hidden.map((h) => h.code ? reviewReasonLabel(h.code, locale) : h.label).join(", ");
  const accessibleName =
    count > 0
      ? t(count === 1 ? "review.moreOne" : "review.moreMany", { count, labels })
      : undefined;

  return (
    <span className="op-review-signal" style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
      <span className="op-badge" style={toneStyle(REASON_TONE[reason.code])} data-reason={reason.code}>
        {reviewReasonLabel(reason.code, locale)}
      </span>
      {count > 0 && (
        <span className="op-plusn" aria-label={accessibleName}>
          +{count}
        </span>
      )}
    </span>
  );
}
