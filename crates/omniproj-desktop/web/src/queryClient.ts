// The pull-only QueryClient. OmniProj never polls or pushes: data is fresh until the user
// asks for a refresh, so queries never go stale on their own and never refetch on focus or
// reconnect. Mutations update the Index row and Overview caches directly from the returned
// DTO — a successful mutation must not trigger a wave of background refetches.

import { QueryClient } from "@tanstack/react-query";

import type { ProjectIndexResponse, ProjectOverview } from "./domain/project";
import { queryKeys } from "./queryKeys";

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: Infinity,
        gcTime: Infinity,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
        refetchOnMount: false,
        retry: 1,
      },
      mutations: {
        retry: 0,
      },
    },
  });
}

/**
 * Fold a freshly-returned Overview into both caches so the Index row and the detail view
 * reflect a mutation without any refetch. The Index row is patched in place from the
 * Overview's shared fields; a project not yet in the Index is left untouched (the next
 * Index load will include it).
 */
export function applyOverviewToCaches(
  client: QueryClient,
  overview: ProjectOverview,
): void {
  client.setQueryData(queryKeys.projectOverview(overview.project_id), overview);
  client.setQueryData<ProjectIndexResponse>(queryKeys.projectIndex, (current) => {
    if (!current) return current;
    let changed = false;
    const projects = current.projects.map((row) => {
      if (row.project_id !== overview.project_id) return row;
      changed = true;
      return {
        ...row,
        name: overview.name,
        status: overview.status,
        current_commitment: overview.current_commitment,
        observed_actual: overview.observed_actual,
        review_reasons: overview.review_reasons,
        source_status: overview.source?.status ?? row.source_status,
        revision: overview.revision,
        source_revision: overview.source?.revision ?? row.source_revision,
      };
    });
    return changed ? { ...current, projects } : current;
  });
}
