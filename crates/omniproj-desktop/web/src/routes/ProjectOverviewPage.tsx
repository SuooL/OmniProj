// The L2 full-page route. It fetches the Overview and renders the shared ProjectOverview as a
// full page (direct access or an Index-origin Peek promoted via "Open as page"). Loading /
// error / not-found are handled here; the content and its DOM order live in ProjectOverview.

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
      {data && <ProjectOverview overview={data} now={new Date()} variant="page" />}
    </main>
  );
}
