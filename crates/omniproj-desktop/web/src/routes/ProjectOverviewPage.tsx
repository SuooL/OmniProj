// The L2 full-page route. It fetches the Overview and renders the shared ProjectOverview as a
// full page. Loading /
// error / not-found are handled here; the content and its DOM order live in ProjectOverview.

import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router-dom";

import { api, AppError } from "../api";
import { ProjectOverview } from "../components/projects/ProjectOverview";
import { projectId as brandProjectId } from "../domain/project";
import { queryKeys } from "../queryKeys";
import { localizeError, useI18n } from "../i18n/I18nProvider";

export function ProjectOverviewPage() {
  const { locale, t } = useI18n();
  const params = useParams();
  const id = brandProjectId(params.projectId ?? "");
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
    if (data.status === "setup") return;
    const heading = headingRef.current;
    if (!heading) return;
    // Landing focus here is for AT/keyboard orientation, not a keyboard interaction, so the
    // focus ring is suppressed for this one programmatic move; a real Tab clears the flag and
    // restores the normal ring.
    heading.dataset.programmaticFocus = "true";
    heading.focus({ preventScroll: true });
    const clear = () => delete heading.dataset.programmaticFocus;
    heading.addEventListener("blur", clear, { once: true });
    heading.addEventListener("keydown", clear, { once: true });
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
        />
      )}
    </main>
  );
}
