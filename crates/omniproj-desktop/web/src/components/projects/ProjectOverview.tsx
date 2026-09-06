// One project, one page — the notebook's facing page.
//
// The name, a year of commits, and the steps. Everything else (project settings, the
// commit log, the branch graph) is reference material behind a disclosure, because it is
// read occasionally and the list is read every time.
//
// What used to be here and is gone on purpose: the four-tab workspace, the re-entry
// context block, the separate decision log, and the commitment card with its
// confirm/complete/switch-away lifecycle. A step is done when its checkbox is ticked.

import type { Ref } from "react";

import type { ProjectOverview as ProjectOverviewDto } from "../../domain/project";
import { formatRelativeTime } from "../../domain/projectPresentation";
import { useI18n } from "../../i18n/I18nProvider";
import { CommitHeatmap } from "./CommitHeatmap";
import { CommitTimeline } from "./CommitTimeline";
import { GitFlowGraph } from "./GitFlowGraph";
import { ProjectFramingForm } from "./ProjectFramingForm";
import { ProjectLifecycleControl } from "./ProjectLifecycleControl";
import { SourceRecovery } from "./SourceRecovery";
import { TaskOutline } from "./TaskOutline";

export interface ProjectOverviewProps {
  overview: ProjectOverviewDto;
  now: Date;
  headingRef?: Ref<HTMLHeadingElement>;
}

export function ProjectOverview({ overview, now, headingRef }: ProjectOverviewProps) {
  const { locale, t } = useI18n();
  const observed = overview.observed_actual;
  const lastCommit = observed?.last_commit;
  const activity = lastCommit ? formatRelativeTime(lastCommit.committed_at, now, locale) : null;

  if (overview.status === "setup") {
    return (
      <article data-testid="project-overview" className="op-project">
        <h1 ref={headingRef} tabIndex={-1} data-testid="overview-heading" className="op-project__name">
          {overview.name}
        </h1>
        <ProjectFramingForm overview={overview} />
      </article>
    );
  }

  return (
    <article data-testid="project-overview" className="op-project">
      <header className="op-project__head" data-testid="overview-identity">
        <h1 ref={headingRef} tabIndex={-1} data-testid="overview-heading" className="op-project__name">
          {overview.name}
        </h1>
        {overview.objective && <p className="op-project__note">{overview.objective}</p>}
        {observed && (
          <p className="op-project__facts">
            {observed.head.kind === "attached" && <span className="op-project__branch">{observed.head.branch}</span>}
            {observed.changed_files > 0 && <span>{t("row.changed", { count: observed.changed_files })}</span>}
            {activity && <span>{t("project.lastCommit", { time: activity.text })}</span>}
          </p>
        )}
      </header>

      <CommitHeatmap projectId={overview.project_id} />

      <SourceRecovery overview={overview} />

      <TaskOutline projectId={overview.project_id} />

      <details className="op-project__more">
        <summary>{t("project.commits")}</summary>
        <div className="op-project__more-body">
          {overview.source && <p className="op-source-path">{overview.source.location}</p>}
          <CommitTimeline projectId={overview.project_id} />
          <GitFlowGraph projectId={overview.project_id} />
        </div>
      </details>

      <details className="op-project__more">
        <summary>{t("project.settings")}</summary>
        <div className="op-project__more-body">
          <ProjectFramingForm overview={overview} />
          <ProjectLifecycleControl overview={overview} />
        </div>
      </details>
    </article>
  );
}
