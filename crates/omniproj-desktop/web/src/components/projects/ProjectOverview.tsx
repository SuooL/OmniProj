// The full-page Project Overview content. DOM order is
// the spec's fixed sequence (9.3): identity + lifecycle -> all review reasons -> current
// commitment (or the atomic Complete-setup framing in `setup`) -> observed actual -> recent
// transition rail. The full source path appears ONLY here, never in the Index.

import type { Ref } from "react";

import type { ProjectOverview as ProjectOverviewDto } from "../../domain/project";
import { ProjectStateTag } from "../semantic/ProjectStateTag";
import { CommitmentHistory } from "./CommitmentHistory";
import { CurrentCommitment } from "./CurrentCommitment";
import { ObservedActual } from "./ObservedActual";
import { ProjectFramingForm } from "./ProjectFramingForm";
import { ProjectLifecycleControl } from "./ProjectLifecycleControl";
import { ReviewReasons } from "./ReviewReasons";
import { SourceRecovery } from "./SourceRecovery";
import { useI18n } from "../../i18n/I18nProvider";
import { TaskBoard } from "./TaskBoard";
import { CommitTimeline } from "./CommitTimeline";
import { PlanLog } from "./PlanLog";
import { GitFlowGraph } from "./GitFlowGraph";
import { ReentryContext } from "./ReentryContext";

export type ProjectWorkspaceView = "reentry" | "plan" | "activity" | "project";

export interface ProjectOverviewProps {
  overview: ProjectOverviewDto;
  now: Date;
  headingRef?: Ref<HTMLHeadingElement>;
  view?: ProjectWorkspaceView;
  onViewChange?: (view: ProjectWorkspaceView) => void;
}

export function ProjectOverview({
  overview,
  now,
  headingRef,
  view = "reentry",
  onViewChange = () => {},
}: ProjectOverviewProps) {
  const { t } = useI18n();
  const isSetup = overview.status === "setup";

  return (
    <article data-testid="project-overview" className="op-overview">
      {/* 1. Identity + lifecycle */}
      <header className="op-overview__hero" data-testid="overview-identity">
        <div className="op-overview__hero-main">
          <p className="op-overview__eyebrow">{t("overview.title")}</p>
          <div className="op-overview__title-row">
            <h2 ref={headingRef} tabIndex={-1} data-testid="overview-heading" className="op-overview__name">
              {overview.name}
            </h2>
            <ProjectStateTag status={overview.status} />
          </div>
        </div>
        {overview.source && (view === "activity" || view === "project") && (
          <p data-testid="source-path" className="op-source-path">
            {overview.source.location}
          </p>
        )}
      </header>

      {isSetup ? (
        <div className="op-overview__primary"><ProjectFramingForm overview={overview} /></div>
      ) : (
        <>
          <nav className="op-workspace-tabs" aria-label={t("workspace.label")}>
            {(["reentry", "plan", "activity", "project"] as const).map((candidate) => (
              <button
                key={candidate}
                type="button"
                aria-current={view === candidate ? "page" : undefined}
                onClick={() => onViewChange(candidate)}
              >
                {t(`workspace.${candidate}`)}
              </button>
            ))}
          </nav>

          {view === "reentry" && (
            <div className="op-overview__primary" data-testid="reentry-view">
              <ReentryContext overview={overview} />
              <ReviewReasons reasons={overview.review_reasons} />
              <CurrentCommitment overview={overview} />
              <SourceRecovery overview={overview} />
            </div>
          )}

          {view === "plan" && (
            <div className="op-overview__secondary" data-testid="plan-view">
              <TaskBoard projectId={overview.project_id} hasCurrentCommitment={overview.current_commitment !== null} />
              <PlanLog projectId={overview.project_id} />
              <CommitmentHistory transitions={overview.recent_transitions} now={now} />
            </div>
          )}

          {view === "activity" && (
            <div className="op-overview__secondary" data-testid="activity-view">
              <ObservedActual observed={overview.observed_actual} source={overview.source} now={now} />
              <SourceRecovery overview={overview} />
              <CommitTimeline projectId={overview.project_id} />
              <GitFlowGraph projectId={overview.project_id} />
            </div>
          )}

          {view === "project" && (
            <div className="op-overview__settings" data-testid="project-view">
              <ProjectFramingForm overview={overview} />
              <ProjectLifecycleControl overview={overview} />
            </div>
          )}
        </>
      )}
    </article>
  );
}
