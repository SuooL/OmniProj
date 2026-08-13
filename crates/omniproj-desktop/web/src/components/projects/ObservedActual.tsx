// The machine-observed actual, rendered as a definition list. When the source last failed we
// still show the last successful observation with its exact timestamp and point to recovery —
// never inactivity wording, because a failed read is not the same as "no work happened".

import type {
  HeadState,
  ObservedActual as ObservedActualDto,
  ProjectSource,
} from "../../domain/project";
import { formatRelativeTime } from "../../domain/projectPresentation";

function headText(head: HeadState): string {
  switch (head.kind) {
    case "attached":
      return `On ${head.branch}`;
    case "detached":
      return "Detached HEAD";
    case "unborn":
      return head.branch ? `${head.branch} (unborn, no commits yet)` : "Unborn (no commits yet)";
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
  const sourceFailed = source !== null && SOURCE_FAILED.has(source.status);

  if (!observed) {
    return (
      <section aria-labelledby="observed-heading" data-testid="observed-actual">
        <h3 id="observed-heading">Observed actual</h3>
        <p className="op-muted">
          {sourceFailed
            ? "The source could not be read; there is no earlier observation to show."
            : "Not yet observed."}
        </p>
      </section>
    );
  }

  const observedTime = formatRelativeTime(observed.observed_at, now);

  return (
    <section aria-labelledby="observed-heading" data-testid="observed-actual">
      <h3 id="observed-heading">Observed actual</h3>
      {sourceFailed && (
        <p className="op-observed-stale" data-testid="observed-stale">
          Source currently unavailable — showing the last successful observation
          {observedTime ? ` from ${observedTime.text}` : ""}.
        </p>
      )}
      <dl className="op-dl">
        <dt>Head</dt>
        <dd>{headText(observed.head)}</dd>

        <dt>Last commit</dt>
        <dd>
          {observed.last_commit ? (
            <span title={observed.last_commit.sha}>
              {observed.last_commit.short_sha} {observed.last_commit.subject}
            </span>
          ) : (
            "No commits yet"
          )}
        </dd>

        <dt>Working tree</dt>
        <dd>
          {observed.changed_files} changed, {observed.staged_files} staged,{" "}
          {observed.untracked_files} untracked
        </dd>

        {observed.commits_since_commitment !== null && (
          <>
            <dt>Since this commitment</dt>
            <dd>
              {observed.commits_since_commitment} repository commit
              {observed.commits_since_commitment === 1 ? "" : "s"} observed since it was set
            </dd>
          </>
        )}

        <dt>Observed</dt>
        <dd title={observed.observed_at}>{observedTime ? observedTime.text : observed.observed_at}</dd>
      </dl>
    </section>
  );
}
