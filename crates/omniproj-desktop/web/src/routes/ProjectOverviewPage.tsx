// The L2 full-page route. It fetches the Overview and renders the shared ProjectOverview as a
// full page. Loading /
// error / not-found are handled here; the content and its DOM order live in ProjectOverview.

import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useParams, useSearchParams } from "react-router-dom";

import { api, AppError } from "../api";
import { ProjectOverview, type ProjectWorkspaceView } from "../components/projects/ProjectOverview";
import { projectId as brandProjectId } from "../domain/project";
import { queryKeys } from "../queryKeys";
import { localizeError, useI18n } from "../i18n/I18nProvider";

export function ProjectOverviewPage() {
  const { locale, t } = useI18n();
  const params = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const id = brandProjectId(params.projectId ?? "");
  const requestedView = searchParams.get("view");
  const view: ProjectWorkspaceView = requestedView === "plan" || requestedView === "activity" || requestedView === "project"
    ? requestedView
    : "reentry";
  const { data, isLoading, isError, error } = useQuery({
    queryKey: queryKeys.projectOverview(id),
    queryFn: () => api.getProjectOverview(id),
  });

  // Land focus in the content once when it first loads, so keyboard/AT users are not stranded
  // on the shell. Setup lets Objective win.
  const headingRef = useRef<HTMLHeadingElement>(null);
  const didFocus = useRef(false);
  useEffect(() => {
    if (!data || didFocus.current) return;
    didFocus.current = true;
    if (data.status !== "setup") headingRef.current?.focus();
  }, [data]);

  return (
    <main className="op-overview-page" data-testid="overview-page" aria-labelledby="overview-heading">
      {isLoading && (
        <p className="op-state-panel" role="status" data-testid="overview-loading">
          {t("overview.loading")}
        </p>
      )}
      {isError && (
        <div className="op-state-panel op-state-panel--error" role="alert" data-testid="overview-error">
          {error instanceof AppError ? localizeError(error, locale) : t("overview.loadFailed")}
        </div>
      )}
      {data && (
        <ProjectOverview
          overview={data}
          now={new Date()}
          headingRef={headingRef}
          view={view}
          onViewChange={(nextView) => {
            const next = new URLSearchParams(searchParams);
            if (nextView === "reentry") next.delete("view");
            else next.set("view", nextView);
            setSearchParams(next, { replace: true });
          }}
        />
      )}
    </main>
  );
}
