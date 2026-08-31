// One dense Index row: four fields behind a single canonical project link. It obeys the badge
// budget (<=1 ProjectStateTag, <=1 ReviewSignalBadge, <=3 FactLabels, and NO CommitmentStateTag
// — that tag is history-only, so the row stays within <=2 enclosed badges). It renders no
// full path, sparkline, health/priority, Git graph, Agent control, full task list, or any
// activity-derived ranking.

import { Link } from "react-router-dom";

import { saveIndexViewState } from "../../domain/navigationSession";
import type { HeadState, ProjectIndexItem } from "../../domain/project";
import { projectOverviewPath } from "../../domain/routes";
import {
  formatRelativeTime,
  hiddenReasons,
  primaryReason,
} from "../../domain/projectPresentation";
import { FactLabel } from "../semantic/FactLabel";
import { ChevronRightIcon, FolderIcon } from "../Icons";
import { ProjectStateTag } from "../semantic/ProjectStateTag";
import { ReviewSignalBadge } from "../semantic/ReviewSignalBadge";
import {
  projectStatusLabel,
  reviewReasonLabel,
  useI18n,
  type Locale,
  type Translate,
} from "../../i18n/I18nProvider";

function headText(head: HeadState, locale: Locale): string {
  switch (head.kind) {
    case "attached":
      return head.branch;
    case "detached":
      return locale === "zh-CN" ? "游离 HEAD" : "detached HEAD";
    case "unborn":
      return head.branch
        ? `${head.branch} (${locale === "zh-CN" ? "尚无提交" : "unborn"})`
        : locale === "zh-CN" ? "尚无提交" : "unborn";
  }
}

/**
 * The row is a single link, so its accessible name must convey the whole row — otherwise an
 * assistive-tech user hears only the project name. This composes the four fields into one
 * spoken summary; the visible cells and their field labels remain for sighted users.
 */
function rowAccessibleName(item: ProjectIndexItem, locale: Locale, t: Translate): string {
  const parts = [item.name];
  const state = item.status === "active" ? null : projectStatusLabel(item.status, locale);
  if (state) parts.push(state);
  parts.push(
    item.current_commitment
      ? t("row.commitment", { text: item.current_commitment.text })
      : t("row.noCommitment"),
  );
  parts.push(item.observed_actual ? t("row.observed", { head: headText(item.observed_actual.head, locale) }) : t("row.notObserved"));
  const primary = item.review_reasons[0];
  if (primary) {
    const more = item.review_reasons.length - 1;
    parts.push(t("row.review", {
      label: reviewReasonLabel(primary.code, locale),
      more: more > 0 ? t("row.more", { count: more }) : "",
    }));
  }
  return `${parts.join(". ")}.`;
}

/** A short natural-language note about working-tree changes; empty when the tree is clean. */
function changeNote(item: ProjectIndexItem, t: Translate): string | null {
  const o = item.observed_actual;
  if (!o) return null;
  const dirty = o.changed_files + o.untracked_files;
  return dirty > 0 ? t("row.changed", { count: dirty }) : t("row.clean");
}

export interface ProjectRowProps {
  item: ProjectIndexItem;
  now: Date;
}

export function ProjectRow({ item, now }: ProjectRowProps) {
  const { locale, t } = useI18n();
  const observed = item.observed_actual;
  const commitment = item.current_commitment;
  const primary = primaryReason(item);
  const hidden = hiddenReasons(item);
  const observedTime = observed ? formatRelativeTime(observed.observed_at, now, locale) : null;
  const sinceCommitment = observed?.commits_since_commitment ?? null;

  const reviewText = primary ? reviewReasonLabel(primary.code, locale) : t("row.noReview");

  return (
    <li className="op-row">
      <Link
        className="op-row__link"
        aria-label={rowAccessibleName(item, locale, t)}
        to={projectOverviewPath(item.project_id)}
        data-focus-id={item.project_id}
        onClick={() =>
          saveIndexViewState({
            scrollY: typeof document !== "undefined"
              ? document.querySelector<HTMLElement>(".app-shell__content")?.scrollTop ?? 0
              : 0,
            focusId: item.project_id,
          })
        }
      >
        <span className="op-row__folder"><FolderIcon /></span>
        <span className="op-row__body">
          <span className="op-row__title-line">
            <span className="op-row__name">{item.name}</span>
            <ProjectStateTag status={item.status} />
          </span>
          <span className="op-row__commitment op-row__commitment--none">
            {commitment ? commitment.text : t("row.noCommitment")}
          </span>
          <span className="op-row__metadata">
            {observed ? (
              <>
                <FactLabel value={headText(observed.head, locale)} />
                {observed.last_commit ? (
                  <FactLabel
                    value={`${observed.last_commit.short_sha} ${observed.last_commit.subject}`}
                    title={observed.last_commit.sha}
                  />
                ) : (
                  <span>{t("row.noCommits")}</span>
                )}
              </>
            ) : (
              <span>{t("row.notObserved")}</span>
            )}
            {observedTime && <FactLabel value={observedTime.text} title={observedTime.title} />}
            {sinceCommitment !== null && sinceCommitment > 0 && (
              <span>{locale === "en"
                ? `${sinceCommitment} commit${sinceCommitment === 1 ? "" : "s"} since`
                : t("row.commitsSince", { count: sinceCommitment })}</span>
            )}
            {observed && <span>{changeNote(item, t)}</span>}
          </span>
        </span>
        <span className="op-row__review-text">
          {primary && <ReviewSignalBadge reason={primary} hidden={hidden} />}
          {!primary && reviewText}
        </span>
        <span className="op-row__chevron"><ChevronRightIcon /></span>
      </Link>
    </li>
  );
}
