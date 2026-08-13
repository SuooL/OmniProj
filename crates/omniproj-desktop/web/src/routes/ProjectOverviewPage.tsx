// The L2 full-page route. It fetches the Overview and renders the shared ProjectOverview as a
// full page (direct access or an Index-origin Peek promoted via "Open as page"). Loading /
// error / not-found are handled here; the content and its DOM order live in ProjectOverview.

import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router-dom";

import { api, AppError } from "../api";
import { ProjectOverview } from "../components/projects/ProjectOverview";
import { projectId as brandProjectId } from "../domain/project";
import { queryKeys } from "../queryKeys";

export function ProjectOverviewPage() {
  const params = useParams();
  const id = brandProjectId(params.projectId ?? "");
  const { data, isLoading, isError, error } = useQuery({
    queryKey: queryKeys.projectOverview(id),
    queryFn: () => api.getProjectOverview(id),
  });

  // Land focus in the content once when it first loads (direct access or a Peek promoted to a
  // full page), so keyboard/AT users are not stranded on the shell. Setup lets Objective win.
  const headingRef = useRef<HTMLHeadingElement>(null);
  const didFocus = useRef(false);
  useEffect(() => {
    if (!data || didFocus.current) return;
    didFocus.current = true;
    if (data.status !== "setup") headingRef.current?.focus();
  }, [data]);

  return (
    <main data-testid="overview-page" aria-labelledby="overview-heading">
      {isLoading && (
        <p role="status" data-testid="overview-loading">
          Loading project…
        </p>
      )}
      {isError && (
        <div role="alert" data-testid="overview-error">
          {error instanceof AppError ? error.message : "Couldn't load this project."}
        </div>
      )}
      {data && (
        <ProjectOverview overview={data} now={new Date()} variant="page" headingRef={headingRef} />
      )}
    </main>
  );
}
