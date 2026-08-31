// The recent commitment transition rail: an ordered list of what happened to the commitment,
// most recent first, as neutral event stamps. It is history, not an action surface.

import type {
  CommitmentTransition,
} from "../../domain/project";
import { formatRelativeTime } from "../../domain/projectPresentation";
import { ActivityStamp } from "../semantic/ActivityStamp";
import { transitionLabel, useI18n } from "../../i18n/I18nProvider";

export interface CommitmentHistoryProps {
  transitions: CommitmentTransition[];
  now: Date;
}

export function CommitmentHistory({ transitions, now }: CommitmentHistoryProps) {
  const { locale, t } = useI18n();
  if (transitions.length === 0) return null;

  return (
    <section className="op-section op-section--history" aria-labelledby="history-heading" data-testid="commitment-history">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("history.kicker")}</p>
          <h3 id="history-heading">{t("history.title")}</h3>
        </div>
      </div>
      <ol className="op-history-rail">
        {transitions.map((t) => {
          const time = formatRelativeTime(t.occurred_at, now, locale);
          return (
            <li key={t.id}>
              <ActivityStamp
                verb={transitionLabel(t.type, locale)}
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
