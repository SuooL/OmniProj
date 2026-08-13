// One dense Index row: four fields behind a single canonical project link. It obeys the badge
// budget (<=1 ProjectStateTag, <=1 ReviewSignalBadge, <=3 FactLabels, and NO CommitmentStateTag
// — that tag is Peek/history only, so the row stays within <=2 enclosed badges). It renders no
// full path, sparkline, health/priority, Git graph, Agent control, full task list, or any
// activity-derived ranking.

import { Link, useLocation } from "react-router-dom";

import { saveIndexViewState } from "../../domain/navigationSession";
import type { HeadState, ProjectIndexItem } from "../../domain/project";
import { projectOverviewPath } from "../../domain/routes";
import {
  formatRelativeTime,
  hiddenReasons,
  primaryReason,
} from "../../domain/projectPresentation";
import { FactLabel } from "../semantic/FactLabel";
import { ProjectStateTag } from "../semantic/ProjectStateTag";
import { ReviewSignalBadge } from "../semantic/ReviewSignalBadge";

function headText(head: HeadState): string {
  switch (head.kind) {
    case "attached":
      return head.branch;
    case "detached":
      return "detached HEAD";
    case "unborn":
      return head.branch ? `${head.branch} (unborn)` : "unborn";
  }
}

const STATE_WORD: Partial<Record<ProjectIndexItem["status"], string>> = {
  setup: "Setup",
  waiting: "Waiting",
  parked: "Parked",
  archived: "Archived",
};

/**
 * The row is a single link, so its accessible name must convey the whole row — otherwise an
 * assistive-tech user hears only the project name. This composes the four fields into one
 * spoken summary; the visible cells and their field labels remain for sighted users.
 */
function rowAccessibleName(item: ProjectIndexItem): string {
  const parts = [item.name];
  const state = STATE_WORD[item.status];
  if (state) parts.push(state);
  parts.push(
    item.current_commitment
      ? `Commitment: ${item.current_commitment.text}`
      : "No current commitment",
  );
  parts.push(item.observed_actual ? `Observed ${headText(item.observed_actual.head)}` : "Not yet observed");
  const primary = item.review_reasons[0];
  if (primary) {
    const more = item.review_reasons.length - 1;
    parts.push(`Review: ${primary.label}${more > 0 ? `, +${more} more` : ""}`);
  }
  return `${parts.join(". ")}.`;
}

/** A short natural-language note about working-tree changes; empty when the tree is clean. */
function changeNote(item: ProjectIndexItem): string | null {
  const o = item.observed_actual;
  if (!o) return null;
  const dirty = o.changed_files + o.untracked_files;
  return dirty > 0 ? `${dirty} changed` : "clean";
}

export interface ProjectRowProps {
  item: ProjectIndexItem;
  now: Date;
}

export function ProjectRow({ item, now }: ProjectRowProps) {
  const location = useLocation();
  const observed = item.observed_actual;
  const commitment = item.current_commitment;
  const primary = primaryReason(item);
  const hidden = hiddenReasons(item);
  const observedTime = observed ? formatRelativeTime(observed.observed_at, now) : null;
  const sinceCommitment = observed?.commits_since_commitment ?? null;

  return (
    <li className="op-row">
      <Link
        className="op-row__link"
        aria-label={rowAccessibleName(item)}
        to={projectOverviewPath(item.project_id)}
        state={{ backgroundLocation: location }}
        data-focus-id={item.project_id}
        onClick={() =>
          saveIndexViewState({
            scrollY: typeof window !== "undefined" ? window.scrollY : 0,
            focusId: item.project_id,
          })
        }
      >
        {/* Project */}
        <span className="op-cell op-cell--project">
          <span className="op-field-label" aria-hidden="true">
            Project
          </span>
          <span className="op-row__name">{item.name}</span>
          <ProjectStateTag status={item.status} />
        </span>

        {/* Current commitment */}
        <span className="op-cell op-cell--commitment">
          <span className="op-field-label" aria-hidden="true">
            Current commitment
          </span>
          {commitment ? (
            <span className="op-row__commitment">{commitment.text}</span>
          ) : (
            <span className="op-row__commitment op-row__commitment--none">
              No current commitment
            </span>
          )}
          {sinceCommitment !== null && sinceCommitment > 0 && (
            <span className="op-observed-note">
              {sinceCommitment} commit{sinceCommitment === 1 ? "" : "s"} since
            </span>
          )}
        </span>

        {/* Observed actual */}
        <span className="op-cell op-cell--observed">
          <span className="op-field-label" aria-hidden="true">
            Observed actual
          </span>
          {observed ? (
            <>
              <FactLabel value={headText(observed.head)} />
              {observed.last_commit ? (
                <FactLabel
                  value={`${observed.last_commit.short_sha} ${observed.last_commit.subject}`}
                  title={observed.last_commit.sha}
                />
              ) : (
                <span className="op-observed-note">no commits</span>
              )}
              {observedTime && (
                <FactLabel label="observed" value={observedTime.text} title={observedTime.title} />
              )}
              {changeNote(item) && <span className="op-observed-note">{changeNote(item)}</span>}
            </>
          ) : (
            <span className="op-observed-note">Not yet observed</span>
          )}
        </span>

        {/* Review */}
        <span className="op-cell op-cell--review">
          <span className="op-field-label" aria-hidden="true">
            Review
          </span>
          {primary ? (
            <ReviewSignalBadge reason={primary} hidden={hidden} />
          ) : (
            <span className="op-row__review-clear">No review needed</span>
          )}
        </span>
      </Link>
    </li>
  );
}
