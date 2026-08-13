// The shared Project Overview content, identical for the Peek and the full page. DOM order is
// the spec's fixed sequence (9.3): identity + lifecycle -> all review reasons -> current
// commitment (or the atomic Complete-setup framing in `setup`) -> observed actual -> recent
// transition rail -> Open as page (peek only). The full source path appears ONLY here, never in
// the Index. The wrapper (page or Peek aside) adds navigation/focus; content lives here.

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
  variant: "peek" | "page";
  onOpenAsPage?: () => void;
  headingRef?: Ref<HTMLHeadingElement>;
}

export function ProjectOverview({
  overview,
  now,
  variant,
  onOpenAsPage,
  headingRef,
}: ProjectOverviewProps) {
  const isSetup = overview.status === "setup";

  return (
    <article data-testid="project-overview" className="op-overview">
      {/* 1. Identity + lifecycle */}
      <section data-testid="overview-identity">
        <h2 ref={headingRef} tabIndex={-1} data-testid="overview-heading" className="op-overview__name">
          {overview.name}
        </h2>
        <ProjectStateTag status={overview.status} />
        {overview.source && (
          <p data-testid="source-path" className="op-source-path">
            {overview.source.location}
          </p>
        )}
        {!isSetup && (
          <>
            <ProjectFramingForm overview={overview} />
            <ProjectLifecycleControl overview={overview} />
          </>
        )}
      </section>

      {/* 2. Expanded review reasons */}
      <ReviewReasons reasons={overview.review_reasons} />

      {/* 3. Current commitment actions — or the atomic Complete-setup framing */}
      {isSetup ? (
        <ProjectFramingForm overview={overview} />
      ) : (
        <CurrentCommitment overview={overview} />
      )}

      {/* 4. Observed actual + source recovery */}
      <ObservedActual observed={overview.observed_actual} source={overview.source} now={now} />
      <SourceRecovery overview={overview} />

      {/* 5. Recent commitment transition rail */}
      <CommitmentHistory transitions={overview.recent_transitions} now={now} />

      {/* 6. Open as page (peek only) */}
      {variant === "peek" && onOpenAsPage && (
        <button type="button" data-testid="open-as-page" onClick={onOpenAsPage}>
          Open as page
        </button>
      )}
    </article>
  );
}
