// The fallback for any unknown route. It never dead-ends: the only action returns to the
// single primary destination, Projects.

import { Link } from "react-router-dom";

import { projectsPath } from "../domain/routes";

export function NotFoundPage() {
  return (
    <main className="op-empty-page" data-testid="not-found">
      <div className="op-empty-page__mark" aria-hidden="true">404</div>
      <p className="op-page-heading__eyebrow">Unknown route</p>
      <h1>Page not found</h1>
      <p>The page may have moved, but your projects and local state are unchanged.</p>
      <Link className="op-button op-button--primary" to={projectsPath()}>Back to Projects</Link>
    </main>
  );
}
