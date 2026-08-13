// The expanded review reasons, shown in the Overview (Peek and full page). Unlike the Index
// (one primary badge + N), here EVERY reason is a full text row with its server-provided
// evidence — including, for a review action, the seven-day interval and the last effective
// set/confirmed timestamp. Evidence strings come from core; the browser never fabricates them.

import type { ReviewReason } from "../../domain/project";
import { ReviewSignalBadge } from "../semantic/ReviewSignalBadge";

export interface ReviewReasonsProps {
  reasons: ReviewReason[];
}

export function ReviewReasons({ reasons }: ReviewReasonsProps) {
  if (reasons.length === 0) {
    return (
      <section aria-labelledby="review-reasons-heading" data-testid="review-reasons">
        <h3 id="review-reasons-heading">Review</h3>
        <p className="op-muted">No review needed.</p>
      </section>
    );
  }

  return (
    <section aria-labelledby="review-reasons-heading" data-testid="review-reasons">
      <h3 id="review-reasons-heading">Review reasons</h3>
      <ul className="op-review-reasons">
        {reasons.map((reason) => (
          <li key={reason.code} className="op-review-reason">
            <ReviewSignalBadge reason={reason} />
            {reason.evidence.length > 0 && (
              <ul className="op-review-reason__evidence">
                {reason.evidence.map((line, i) => (
                  <li key={i}>{line}</li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
