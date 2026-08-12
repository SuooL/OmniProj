// Stable, structured query keys. One place so cache reads/writes and invalidations never
// drift. Keys are readonly tuples so TanStack Query hashes them deterministically.

import type { ProjectId } from "./domain/project";

export const queryKeys = {
  projectIndex: ["project-index"] as const,
  projectOverview: (id: ProjectId) => ["project-overview", id] as const,
  sourceValidation: (location: string) => ["source-validation", location] as const,
  refresh: (ids: readonly ProjectId[] | null) => ["refresh", ids] as const,
} as const;
