// The machine-observed actual, rendered as a definition list. When the source last failed we
// still show the last successful observation with its exact timestamp and point to recovery —
// never inactivity wording, because a failed read is not the same as "no work happened".

import type {
  HeadState,
  ObservedActual as ObservedActualDto,
  ProjectSource,
} from "../../domain/project";
import { formatRelativeTime } from "../../domain/projectPresentation";
import { useI18n, type Translate } from "../../i18n/I18nProvider";

function headText(head: HeadState, t: Translate): string {
  switch (head.kind) {
    case "attached":
      return t("head.onBranch", { branch: head.branch });
    case "detached":
      return t("head.detached");
    case "unborn":
      return head.branch ? t("head.branchUnborn", { branch: head.branch }) : t("head.unborn");
  }
}

const SOURCE_FAILED: ReadonlySet<ProjectSource["status"]> = new Set([
  "missing",
  "moved",
  "unreadable",
]);

export interface ObservedActualProps {
  observed: ObservedActualDto | null;
  source: ProjectSource | null;
  now: Date;
}

export function ObservedActual({ observed, source, now }: ObservedActualProps) {
  const { locale, t } = useI18n();
  const sourceFailed = source !== null && SOURCE_FAILED.has(source.status);

  if (!observed) {
    return (
      <section className="op-section op-section--facts" aria-labelledby="observed-heading" data-testid="observed-actual">
        <div className="op-section__header">
          <div>
            <p className="op-section__kicker">{t("observed.kicker")}</p>
            <h3 id="observed-heading">{t("observed.title")}</h3>
          </div>
        </div>
        <p className="op-muted">
          {sourceFailed
            ? t("observed.sourceNoHistory")
            : t("observed.notYet")}
        </p>
      </section>
    );
  }

  const observedTime = formatRelativeTime(observed.observed_at, now, locale);

  return (
    <section className="op-section op-section--facts" aria-labelledby="observed-heading" data-testid="observed-actual">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("observed.kicker")}</p>
          <h3 id="observed-heading">{t("observed.title")}</h3>
        </div>
        {observedTime && <span className="op-section__meta">{observedTime.text}</span>}
      </div>
      {sourceFailed && (
        <p className="op-observed-stale" data-testid="observed-stale">
          {t("observed.stale", { time: observedTime ? t("observed.fromTime", { time: observedTime.text }) : "" })}
        </p>
      )}
      <dl className="op-dl">
        <dt>{t("observed.head")}</dt>
        <dd>{headText(observed.head, t)}</dd>

        <dt>{t("observed.lastCommit")}</dt>
        <dd>
          {observed.last_commit ? (
            <span title={observed.last_commit.sha}>
              {observed.last_commit.short_sha} {observed.last_commit.subject}
            </span>
          ) : (
            t("row.noCommits")
          )}
        </dd>

        <dt>{t("observed.workingTree")}</dt>
        <dd>
          {t("observed.workingTreeValue", {
            changed: observed.changed_files,
            staged: observed.staged_files,
            untracked: observed.untracked_files,
          })}
        </dd>

        {observed.commits_since_commitment !== null && (
          <>
            <dt>{t("observed.sinceCommitment")}</dt>
            <dd>
              {t("observed.commitsSince", { count: observed.commits_since_commitment })}
            </dd>
          </>
        )}

        <dt>{t("observed.observedAt")}</dt>
        <dd title={observed.observed_at}>{observedTime ? observedTime.text : observed.observed_at}</dd>
      </dl>
    </section>
  );
}
