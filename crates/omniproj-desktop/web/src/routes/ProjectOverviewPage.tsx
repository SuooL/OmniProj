// The L2 project Overview. One component renders the same content whether it is shown as a
// Peek over the Index or as a direct full page — the canonical Overview URL is identical in
// both. Task 9 wires the routing shell and the "Open as page" affordance; the review reasons,
// commitment actions, observed-actual list, and transition rail are Task 11.

import { useNavigate, useParams } from "react-router-dom";

import { projectId as brandProjectId } from "../domain/project";
import { projectOverviewPath } from "../domain/routes";

export interface ProjectOverviewPageProps {
  /** "peek" renders over the still-mounted Index; "page" is the direct full-page render. */
  variant: "peek" | "page";
}

export function ProjectOverviewPage({ variant }: ProjectOverviewPageProps) {
  const params = useParams();
  const navigate = useNavigate();
  const id = brandProjectId(params.projectId ?? "");

  // "Open as page" keeps the object URL and only drops the background state, so the same
  // canonical Overview promotes from Peek to full page without a navigation to a new URL. The
  // URL comes from the shared route builder so it can never drift from the peek/link form.
  function openAsPage() {
    navigate(projectOverviewPath(id), { replace: true });
  }

  const containerTestId = variant === "peek" ? "overview-peek" : "overview-page";

  return (
    <section
      data-testid={containerTestId}
      role={variant === "peek" ? "dialog" : undefined}
      aria-labelledby="overview-heading"
    >
      <div data-testid="overview-content">
        <h2 id="overview-heading">Project overview</h2>
        <p data-testid="overview-project-id">{id}</p>
      </div>
      {variant === "peek" && (
        <button type="button" onClick={openAsPage}>
          Open as page
        </button>
      )}
    </section>
  );
}
