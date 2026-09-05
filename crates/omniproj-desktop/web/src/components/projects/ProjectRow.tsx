// One Index row: what the project is, how much of it is left, and whether it has been
// touched lately. Four facts, one link.
//
// What is deliberately not here: a review badge, a "needs a decision" signal, the current
// commitment text, or any ranking. The row reports; it does not tell the user which
// project to care about.

import { Link } from "react-router-dom";

import { saveIndexViewState } from "../../domain/navigationSession";
import type { HeadState, ProjectIndexItem } from "../../domain/project";
import { projectOverviewPath } from "../../domain/routes";
import { formatRelativeTime } from "../../domain/projectPresentation";
import { ActivitySparkline } from "./ActivitySparkline";
import { ChevronRightIcon, FolderIcon } from "../Icons";
import { ProjectStateTag } from "../semantic/ProjectStateTag";
import {
  projectStatusLabel,
  useI18n,
  type Locale,
  type Translate,
} from "../../i18n/I18nProvider";

function headText(head: HeadState, t: Translate): string {
  switch (head.kind) {
    case "attached":
      return head.branch;
    case "detached":
      return t("head.detached");
    case "unborn":
      return head.branch ? t("head.branchUnborn", { branch: head.branch }) : t("head.unborn");
  }
}

function activityNote(item: ProjectIndexItem, now: Date, locale: Locale, t: Translate): string | null {
  const committed = item.observed_actual?.last_commit?.committed_at;
  if (!committed) return null;
  const time = formatRelativeTime(committed, now, locale);
  return time ? t("row.lastActivity", { time: time.text }) : null;
}

/**
 * The row is a single link, so its accessible name has to convey the whole row —
 * otherwise an assistive-tech user hears only the project name.
 */
function rowAccessibleName(item: ProjectIndexItem, locale: Locale, t: Translate, now: Date): string {
  const parts = [item.name];
  if (item.status !== "active") parts.push(projectStatusLabel(item.status, locale));
  parts.push(t("row.steps", { open: item.open_steps, total: item.total_steps }));
  if (item.observed_actual) parts.push(headText(item.observed_actual.head, t));
  else parts.push(t("row.notObserved"));
  const activity = activityNote(item, now, locale, t);
  if (activity) parts.push(activity);
  return `${parts.join(". ")}.`;
}

export interface ProjectRowProps {
  item: ProjectIndexItem;
  now: Date;
}

export function ProjectRow({ item, now }: ProjectRowProps) {
  const { locale, t } = useI18n();
  const observed = item.observed_actual;
  const activityText = activityNote(item, now, locale, t);

  return (
    <li className="op-row">
      <Link
        className="op-row__link"
        aria-label={rowAccessibleName(item, locale, t, now)}
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
          <span className="op-row__metadata" title={observed?.observed_at}>
            <span className="op-row__steps">
              {item.total_steps === 0
                ? t("row.noSteps")
                : t("row.steps", { open: item.open_steps, total: item.total_steps })}
            </span>
            {observed ? (
              <>
                <span className="op-row__branch">{headText(observed.head, t)}</span>
                {observed.changed_files > 0 && (
                  <span>{t("row.changed", { count: observed.changed_files })}</span>
                )}
                {activityText && <span>{activityText}</span>}
              </>
            ) : (
              <span>{t("row.notObserved")}</span>
            )}
          </span>
        </span>
        {observed && <ActivitySparkline weeks={observed.commit_activity_weeks} />}
        <span className="op-row__chevron"><ChevronRightIcon /></span>
      </Link>
    </li>
  );
}
