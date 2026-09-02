// The focus-first project page. The current next step is the visual endpoint; direction,
// review evidence, and low-frequency tools support it without competing as equal tabs.

import { useState, type ReactNode, type Ref } from "react";

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

export interface ProjectOverviewProps {
  overview: ProjectOverviewDto;
  now: Date;
  headingRef?: Ref<HTMLHeadingElement>;
}

function WorkspaceDisclosure({ label, testId, children }: { label: string; testId: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <details className="op-workspace-disclosure" data-testid={testId} open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>{label}</summary>
      {open && <div className="op-workspace-disclosure__body">{children}</div>}
    </details>
  );
}

export function ProjectOverview({
  overview,
  now,
  headingRef,
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
      </header>

      {isSetup ? (
        <div className="op-overview__primary"><ProjectFramingForm overview={overview} /></div>
      ) : (
        <>
          <div className="op-overview__primary" data-testid="reentry-view">
            <CurrentCommitment overview={overview} />
            <ReviewReasons reasons={overview.review_reasons} />
            <ReentryContext overview={overview} />
            <SourceRecovery overview={overview} />
          </div>

          <div className="op-workspace-disclosures">
            <WorkspaceDisclosure label={t("workspace.plan")} testId="plan-view">
              <TaskBoard projectId={overview.project_id} hasCurrentCommitment={overview.current_commitment !== null} />
              <PlanLog projectId={overview.project_id} />
              <CommitmentHistory transitions={overview.recent_transitions} now={now} />
            </WorkspaceDisclosure>

            <WorkspaceDisclosure label={t("workspace.activity")} testId="activity-view">
              {overview.source && <p data-testid="source-path" className="op-source-path">{overview.source.location}</p>}
              <ObservedActual observed={overview.observed_actual} source={overview.source} now={now} />
              <CommitTimeline projectId={overview.project_id} />
              <GitFlowGraph projectId={overview.project_id} />
            </WorkspaceDisclosure>

            <WorkspaceDisclosure label={t("workspace.project")} testId="project-view">
              <ProjectFramingForm overview={overview} />
              <ProjectLifecycleControl overview={overview} />
            </WorkspaceDisclosure>
          </div>
        </>
      )}
    </article>
  );
}
