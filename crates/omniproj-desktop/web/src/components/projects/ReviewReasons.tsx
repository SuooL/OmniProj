// The expanded review reasons shown in the project Overview. Unlike the Index
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
      <section className="op-section op-section--quiet" aria-labelledby="review-reasons-heading" data-testid="review-reasons">
        <div className="op-section__header">
          <div>
            <p className="op-section__kicker">Review state</p>
            <h3 id="review-reasons-heading">No review needed</h3>
          </div>
          <span className="op-section__indicator op-section__indicator--clear" aria-hidden="true" />
        </div>
        <p className="op-muted">This project has no deterministic review signal right now.</p>
      </section>
    );
  }

  return (
    <section className="op-section op-section--review" aria-labelledby="review-reasons-heading" data-testid="review-reasons">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">Needs attention</p>
          <h3 id="review-reasons-heading">Review reasons</h3>
        </div>
        <span className="op-section__count">{reasons.length}</span>
      </div>
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
