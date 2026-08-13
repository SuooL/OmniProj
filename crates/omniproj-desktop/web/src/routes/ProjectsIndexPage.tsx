// The L1 dense operating index. Task 9 establishes only its routing role: it stays mounted
// beneath a Peek, it reads its filter/sort from the canonical search params, and each row
// opens the project's Overview as a Peek over this still-mounted Index. The dense semantic
// grammar (columns, badges, review order) is Task 10; here rows are intentionally minimal.

import { useQuery } from "@tanstack/react-query";
import { Link, useLocation, useSearchParams } from "react-router-dom";

import { api } from "../api";
import { saveIndexViewState } from "../domain/navigationSession";
import { projectOverviewPath } from "../domain/routes";
import { queryKeys } from "../queryKeys";

/** The canonical Index search params. Filter and sort are URL state, never session-only. */
export function readIndexParams(params: URLSearchParams): { query: string; sort: string } {
  return { query: params.get("q") ?? "", sort: params.get("sort") ?? "review" };
}

export function ProjectsIndexPage() {
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const { query, sort } = readIndexParams(searchParams);

  const { data } = useQuery({
    queryKey: queryKeys.projectIndex,
    queryFn: api.listProjectIndex,
  });
  const projects = data?.projects ?? [];

  return (
    <main data-testid="projects-index" aria-labelledby="projects-index-heading">
      <h1 id="projects-index-heading">Projects</h1>
      <p data-testid="index-active-filter" hidden>
        {query}|{sort}
      </p>
      <ul>
        {projects.map((item) => (
          <li key={item.project_id}>
            <Link
              to={projectOverviewPath(item.project_id)}
              state={{ backgroundLocation: location }}
              data-focus-id={item.project_id}
              onClick={() =>
                saveIndexViewState({
                  scrollY: typeof window !== "undefined" ? window.scrollY : 0,
                  focusId: item.project_id,
                })
              }
            >
              {item.name}
            </Link>
          </li>
        ))}
      </ul>
    </main>
  );
}
