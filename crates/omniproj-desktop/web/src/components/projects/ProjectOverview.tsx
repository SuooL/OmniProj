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

export interface ProjectOverviewProps {
  overview: ProjectOverviewDto;
  now: Date;
  headingRef?: Ref<HTMLHeadingElement>;
}

export function ProjectOverview({
  overview,
  now,
  headingRef,
}: ProjectOverviewProps) {
  const isSetup = overview.status === "setup";

  return (
    <article data-testid="project-overview" className="op-overview">
      {/* 1. Identity + lifecycle */}
      <header className="op-overview__hero" data-testid="overview-identity">
        <div className="op-overview__hero-main">
          <p className="op-overview__eyebrow">Project overview</p>
          <div className="op-overview__title-row">
            <h2 ref={headingRef} tabIndex={-1} data-testid="overview-heading" className="op-overview__name">
              {overview.name}
            </h2>
            <ProjectStateTag status={overview.status} />
          </div>
        </div>
        {overview.source && (
          <p data-testid="source-path" className="op-source-path">
            {overview.source.location}
          </p>
        )}
      </header>

      <div className="op-overview__primary">
        {/* 2. Expanded review reasons */}
        <ReviewReasons reasons={overview.review_reasons} />

        {/* 3. Current commitment actions — or the atomic Complete-setup framing */}
        {isSetup ? (
          <ProjectFramingForm overview={overview} />
        ) : (
          <CurrentCommitment overview={overview} />
        )}
      </div>

      <div className="op-overview__secondary">
        {/* 4. Observed actual + source recovery */}
        <ObservedActual observed={overview.observed_actual} source={overview.source} now={now} />
        <SourceRecovery overview={overview} />

        {/* 5. Recent commitment transition rail */}
        <CommitmentHistory transitions={overview.recent_transitions} now={now} />

        {!isSetup && (
          <div className="op-overview__settings">
            <ProjectFramingForm overview={overview} />
            <ProjectLifecycleControl overview={overview} />
          </div>
        )}
      </div>
    </article>
  );
}
