// The single primary review signal for a row. Tone is fixed per deterministic reason code
// (danger = unavailable source, info = observed change, warning = review/action required).
// Extra reasons are a PLAIN, uncontained `+N` whose accessible name enumerates them — never a
// second enclosed badge.

import type { ReviewReasonCode } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";

const REASON_TONE: Record<ReviewReasonCode, StatusTone> = {
  source_unavailable: "danger",
  complete_setup: "warning",
  needs_commitment: "warning",
  review_action: "info",
  scheduled_review: "warning",
};

export interface ReviewSignalBadgeProps {
  reason: { code: ReviewReasonCode; label: string };
  /** The lower-priority reasons folded into `+N`, most-urgent first. */
  hidden?: ReadonlyArray<{ label: string }>;
}

export function ReviewSignalBadge({ reason, hidden = [] }: ReviewSignalBadgeProps) {
  const count = hidden.length;
  const noun = count === 1 ? "review reason" : "review reasons";
  const accessibleName =
    count > 0
      ? `${count} more ${noun}: ${hidden.map((h) => h.label).join(", ")}`
      : undefined;

  return (
    <span className="op-review-signal" style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
      <span className="op-badge" style={toneStyle(REASON_TONE[reason.code])} data-reason={reason.code}>
        {reason.label}
      </span>
      {count > 0 && (
        <span className="op-plusn" aria-label={accessibleName}>
          +{count}
        </span>
      )}
    </span>
  );
}
