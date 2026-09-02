// The focus-first project page. The current next step stays the visual endpoint and is always
// visible above the workspace; the supporting work (planning, observed change, project
// settings) sits in a segmented control rather than accordions.
//
// Accordions were the wrong idiom for daily desktop use: they defaulted closed and reset on
// every navigation, so the task list — the surface the user actually lives in — cost a click
// every single time. Tabs keep one pane open and remember the choice per session.

import { useEffect, useState, type Ref } from "react";

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

type WorkspaceTab = "plan" | "activity" | "project";
const WORKSPACE_TAB_STORAGE_KEY = "omniproj.workspace-tab";

function storedTab(): WorkspaceTab {
  if (typeof window === "undefined") return "plan";
  try {
    const raw = window.localStorage.getItem(WORKSPACE_TAB_STORAGE_KEY);
    return raw === "activity" || raw === "project" ? raw : "plan";
  } catch {
    return "plan";
  }
}

export function ProjectOverview({
  overview,
  now,
  headingRef,
}: ProjectOverviewProps) {
  const { t } = useI18n();
  const isSetup = overview.status === "setup";
  const [tab, setTab] = useState<WorkspaceTab>(storedTab);

  useEffect(() => {
    try { window.localStorage.setItem(WORKSPACE_TAB_STORAGE_KEY, tab); } catch { /* best effort */ }
  }, [tab]);

  const TABS: Array<{ id: WorkspaceTab; label: string }> = [
    { id: "plan", label: t("workspace.plan") },
    { id: "activity", label: t("workspace.activity") },
    { id: "project", label: t("workspace.project") },
  ];

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

          <div className="op-workspace">
            <div className="op-workspace__tabs" role="tablist" aria-label={t("workspace.label")}>
              {TABS.map((entry) => (
                <button
                  key={entry.id}
                  type="button"
                  role="tab"
                  id={`workspace-tab-${entry.id}`}
                  aria-selected={tab === entry.id}
                  aria-controls={`workspace-panel-${entry.id}`}
                  tabIndex={tab === entry.id ? 0 : -1}
                  className="op-workspace__tab"
                  onClick={() => setTab(entry.id)}
                  onKeyDown={(event) => {
                    const index = TABS.findIndex((candidate) => candidate.id === tab);
                    if (event.key === "ArrowRight") setTab(TABS[(index + 1) % TABS.length].id);
                    if (event.key === "ArrowLeft") setTab(TABS[(index - 1 + TABS.length) % TABS.length].id);
                  }}
                >
                  {entry.label}
                </button>
              ))}
            </div>

            {/* Only the selected panel mounts, preserving the lazy-mount contract. */}
            {tab === "plan" && (
              <div role="tabpanel" id="workspace-panel-plan" aria-labelledby="workspace-tab-plan" data-testid="plan-view">
                <TaskBoard projectId={overview.project_id} hasCurrentCommitment={overview.current_commitment !== null} />
                <PlanLog projectId={overview.project_id} />
                <CommitmentHistory transitions={overview.recent_transitions} now={now} />
              </div>
            )}
            {tab === "activity" && (
              <div role="tabpanel" id="workspace-panel-activity" aria-labelledby="workspace-tab-activity" data-testid="activity-view">
                {overview.source && <p data-testid="source-path" className="op-source-path">{overview.source.location}</p>}
                <ObservedActual observed={overview.observed_actual} source={overview.source} now={now} />
                <CommitTimeline projectId={overview.project_id} />
                <GitFlowGraph projectId={overview.project_id} />
              </div>
            )}
            {tab === "project" && (
              <div role="tabpanel" id="workspace-panel-project" aria-labelledby="workspace-tab-project" data-testid="project-view">
                <ProjectFramingForm overview={overview} />
                <ProjectLifecycleControl overview={overview} />
              </div>
            )}
          </div>
        </>
      )}
    </article>
  );
}
