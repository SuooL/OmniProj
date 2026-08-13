// The fallback for any unknown route. It never dead-ends: the only action returns to the
// single primary destination, Projects.

import { Link } from "react-router-dom";

import { projectsPath } from "../domain/routes";

export function NotFoundPage() {
  return (
    <main data-testid="not-found">
      <h1>Page not found</h1>
      <Link to={projectsPath()}>Back to Projects</Link>
    </main>
  );
}
