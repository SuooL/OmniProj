// The recent commitment transition rail: an ordered list of what happened to the commitment,
// most recent first, as neutral event stamps. It is history, not an action surface.

import type {
  CommitmentTransition,
  CommitmentTransitionKind,
} from "../../domain/project";
import { formatRelativeTime } from "../../domain/projectPresentation";
import { ActivityStamp } from "../semantic/ActivityStamp";

const VERB: Record<CommitmentTransitionKind, string> = {
  set: "Set",
  confirmed: "Confirmed",
  completed: "Completed",
  replaced: "Replaced",
  cleared: "Cleared",
  correction: "Correction",
};

export interface CommitmentHistoryProps {
  transitions: CommitmentTransition[];
  now: Date;
}

export function CommitmentHistory({ transitions, now }: CommitmentHistoryProps) {
  if (transitions.length === 0) return null;

  return (
    <section aria-labelledby="history-heading" data-testid="commitment-history">
      <h3 id="history-heading">Recent commitment history</h3>
      <ol className="op-history-rail">
        {transitions.map((t) => {
          const time = formatRelativeTime(t.occurred_at, now);
          return (
            <li key={t.id}>
              <ActivityStamp
                verb={VERB[t.type]}
                text={time ? time.text : t.occurred_at}
                title={t.occurred_at}
              />
              {t.reason && <span className="op-history-reason"> — {t.reason}</span>}
            </li>
          );
        })}
      </ol>
    </section>
  );
}
