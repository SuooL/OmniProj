import type { ProjectOverview } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function ReentryContext({ overview }: { overview: ProjectOverview }) {
  const { t } = useI18n();
  const actual = overview.observed_actual;
  const commits = actual?.commits_since_commitment;
  const changed = actual?.changed_files ?? 0;

  return (
    <section className="op-section op-reentry-context" aria-labelledby="reentry-context-heading" data-testid="reentry-context">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("reentry.kicker")}</p>
          <h3 id="reentry-context-heading">{t("reentry.title")}</h3>
        </div>
      </div>
      <dl className="op-reentry-context__grid">
        <div>
          <dt>{t("framing.objective")}</dt>
          <dd>{overview.objective || t("reentry.missingObjective")}</dd>
        </div>
        <div>
          <dt>{t("framing.desiredOutcome")}</dt>
          <dd>{overview.desired_outcome || t("reentry.missingOutcome")}</dd>
        </div>
        <div>
          <dt>{t("reentry.sinceCommitment")}</dt>
          <dd>
            {actual
              ? t("reentry.delta", {
                  commits: commits ?? 0,
                  changed,
                })
              : t("reentry.noActual")}
          </dd>
        </div>
      </dl>
      {actual?.last_commit && (
        <p className="op-reentry-context__latest">
          <span>{t("reentry.latest")}</span>
          <code>{actual.last_commit.short_sha}</code>
          <strong>{actual.last_commit.subject}</strong>
        </p>
      )}
    </section>
  );
}
