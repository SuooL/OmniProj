// The L1 route. It owns the Index query and its loading/error/empty/content states, then hands
// content to the dense <ProjectsIndex>. The outer container keeps a stable testid across every
// state. Filter and sort are canonical search params, read inside ProjectsIndex.

import { useQuery } from "@tanstack/react-query";

import { api, AppError } from "../api";
import { ProjectsIndex } from "../components/projects/ProjectsIndex";
import { useAppActions } from "../components/AppShell";
import { queryKeys } from "../queryKeys";

export function ProjectsIndexPage() {
  const { openAddProject } = useAppActions();
  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: queryKeys.projectIndex,
    queryFn: api.listProjectIndex,
  });

  const now = new Date();

  return (
    <main
      className="op-index-page"
      data-testid="projects-index"
      aria-labelledby="projects-index-heading"
    >
      <header className="op-page-heading">
        <div>
          <p className="op-page-heading__eyebrow">Workspace</p>
          <h1 id="projects-index-heading">Projects</h1>
          <p className="op-page-heading__summary">
            Re-enter each project through its current commitment and observed work.
          </p>
        </div>
        {data && (
          <p className="op-page-heading__count">
            <strong>{data.projects.length}</strong>
            <span>{data.projects.length === 1 ? "project" : "projects"}</span>
          </p>
        )}
      </header>

      {isLoading && (
        <p className="op-state-panel" data-testid="projects-index-loading" role="status">
          Loading projects…
        </p>
      )}

      {isError && (
        <div
          className="op-state-panel op-state-panel--error"
          data-testid="projects-index-error"
          role="alert"
        >
          <p>{error instanceof AppError ? error.message : "Couldn't load projects."}</p>
          <button
            className="op-button op-button--secondary"
            type="button"
            onClick={() => refetch()}
          >
            Try again
          </button>
        </div>
      )}

      {data && (
        <ProjectsIndex
          projects={data.projects}
          reviewPolicy={data.review_policy}
          now={now}
          onAddProject={openAddProject}
        />
      )}
    </main>
  );
}
