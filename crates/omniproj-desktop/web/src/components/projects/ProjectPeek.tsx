// The Peek inspector: the same ProjectOverview content shown as a non-modal `aside` over the
// still-mounted Index (>=800px only — App decides). It focuses its heading on open, leaves the
// background navigable, and on close restores focus to the originating Index row by its stable
// DOM id. Escape-to-close is handled by the AppShell (it pops history).

import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";

import { api, AppError } from "../../api";
import { loadIndexViewState } from "../../domain/navigationSession";
import { projectId as brandProjectId } from "../../domain/project";
import { projectOverviewPath } from "../../domain/routes";
import { queryKeys } from "../../queryKeys";
import { ProjectOverview } from "./ProjectOverview";

export function ProjectPeek() {
  const params = useParams();
  const id = brandProjectId(params.projectId ?? "");
  const navigate = useNavigate();
  const headingRef = useRef<HTMLHeadingElement>(null);

  const { data, isLoading, isError, error } = useQuery({
    queryKey: queryKeys.projectOverview(id),
    queryFn: () => api.getProjectOverview(id),
  });

  // Focus the heading when the Peek opens.
  useEffect(() => {
    headingRef.current?.focus();
  }, [data]);

  // On close, restore focus to the row that opened the Peek (stable data-focus-id).
  useEffect(() => {
    return () => {
      const focusId = loadIndexViewState()?.focusId;
      if (!focusId) return;
      const row = document.querySelector<HTMLElement>(
        `[data-focus-id="${CSS.escape(focusId)}"]`,
      );
      row?.focus();
    };
  }, []);

  const openAsPage = () => navigate(projectOverviewPath(id), { replace: true });

  return (
    <aside
      className="op-peek"
      aria-label="Project overview"
      data-testid="overview-peek"
    >
      {isLoading && <p role="status">Loading project…</p>}
      {isError && (
        <p role="alert">
          {error instanceof AppError ? error.message : "Couldn't load this project."}
        </p>
      )}
      {data && (
        <ProjectOverview
          overview={data}
          now={new Date()}
          variant="peek"
          onOpenAsPage={openAsPage}
          headingRef={headingRef}
        />
      )}
    </aside>
  );
}
