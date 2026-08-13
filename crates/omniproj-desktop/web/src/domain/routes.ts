// The single source of canonical route strings. Components never concatenate route paths;
// they call these builders so the shape of a URL lives in exactly one place. R0 ships only
// Projects (L1) and a project Overview (L2). Deeper objects are future routes, not here.

import type { ProjectId } from "./project";

/** Static canonical paths. */
export const ROUTES = {
  root: "/",
  projects: "/projects",
  notFound: "*",
  // Parameterized patterns, for <Route path> declarations only (not for navigation).
  projectById: "/projects/:projectId",
  projectOverview: "/projects/:projectId/overview",
} as const;

/** The dense operating index. */
export function projectsPath(): string {
  return ROUTES.projects;
}

/** The bare project path, which always redirects to the canonical Overview. */
export function projectByIdPath(id: ProjectId): string {
  return `/projects/${encodeURIComponent(id)}`;
}

/** The canonical Overview URL, shared verbatim by the Peek and the full page. */
export function projectOverviewPath(id: ProjectId): string {
  return `/projects/${encodeURIComponent(id)}/overview`;
}

/** True when a pathname is the canonical Overview for some project. */
export function isOverviewPath(pathname: string): boolean {
  return /^\/projects\/[^/]+\/overview\/?$/.test(pathname);
}
