// The interaction frame that wraps every screen: the single primary destination (Projects),
// the local filter, Add Project, pull-refresh, the background-location Peek overlay, the
// stacked Add Project modal, the global shortcuts, and the two persistent announcement
// regions. Routing decisions live here; App.tsx only owns the BrowserRouter and the
// pre-mount route restore.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Location } from "react-router-dom";
import {
  Link,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";

import { api, AppError } from "../api";
import { saveCanonicalLocation } from "../domain/navigationSession";
import { projectId as brandProjectId } from "../domain/project";
import type { ProjectIndexResponse } from "../domain/project";
import {
  ROUTES,
  projectOverviewPath,
  projectsPath,
} from "../domain/routes";
import { useAppShortcuts } from "../hooks/useAppShortcuts";
import { useIsPeekViewport } from "../hooks/useMediaQuery";
import { queryKeys } from "../queryKeys";
import { NotFoundPage } from "../routes/NotFoundPage";
import { ProjectOverviewPage } from "../routes/ProjectOverviewPage";
import { ProjectsIndexPage } from "../routes/ProjectsIndexPage";
import { ProjectPeek } from "./projects/ProjectPeek";
import { AddProjectDialog } from "./AddProjectDialog";
import { LiveStatus } from "./LiveStatus";

// --- Announcer -------------------------------------------------------------
// Any screen can announce without owning a live region: the two persistent regions live in
// the shell, and this context routes a message to the polite or assertive one.
export type AnnounceLevel = "polite" | "assertive";
export type Announce = (level: AnnounceLevel, message: string) => void;

// A safe no-op default so content components can render/announce in isolation (unit tests)
// while the real shell supplies the live regions in production.
const AnnouncerContext = createContext<Announce>(() => {});

/** Announce a polite (progress) or assertive (error) message through the shell's regions. */
export function useAnnouncer(): Announce {
  return useContext(AnnouncerContext);
}

// Shell-owned actions any screen can invoke without owning the modal/refresh state (e.g. the
// Index empty state's "Add project", or a future toolbar).
export interface AppActions {
  openAddProject: () => void;
  refresh: () => void;
}

const AppActionsContext = createContext<AppActions | null>(null);

export function useAppActions(): AppActions {
  const actions = useContext(AppActionsContext);
  if (actions === null) {
    throw new Error("useAppActions must be used within the AppShell");
  }
  return actions;
}

interface BackgroundState {
  backgroundLocation?: Location;
}

/** The bare `/projects/:projectId` always resolves to the canonical Overview. */
function ProjectIdRedirect() {
  const params = useParams();
  const id = brandProjectId(params.projectId ?? "");
  return <Navigate to={projectOverviewPath(id)} replace />;
}

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const filterRef = useRef<HTMLInputElement>(null);
  const [addProjectOpen, setAddProjectOpen] = useState(false);
  const [polite, setPolite] = useState("");
  const [assertive, setAssertive] = useState("");

  // A Peek only exists on a wide viewport; below 800px an Index-origin navigation is a real
  // full-page detail (no Peek, no lingering Index in the DOM/a11y tree).
  const isPeekViewport = useIsPeekViewport();
  const backgroundLocation =
    (location.state as BackgroundState | null)?.backgroundLocation ?? null;
  const effectiveBackgroundLocation = isPeekViewport ? backgroundLocation : null;

  const announce = useCallback<Announce>((level, message) => {
    if (level === "polite") setPolite(message);
    else setAssertive(message);
  }, []);

  // Remember the last canonical location so a restart at "/" lands back here. "/" is never
  // saved as a target (see navigationSession), which also avoids a restore loop.
  useEffect(() => {
    saveCanonicalLocation(location.pathname + location.search);
  }, [location.pathname, location.search]);

  const filterValue = searchParams.get("q") ?? "";
  const onFilterChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const next = new URLSearchParams(searchParams);
      if (event.target.value) next.set("q", event.target.value);
      else next.delete("q");
      // Preserve the current history state so filtering while a Peek is open does not drop
      // `backgroundLocation` and silently reinterpret the Overview URL as a full page.
      setSearchParams(next, { replace: true, state: location.state });
    },
    [location.state, searchParams, setSearchParams],
  );

  // Pull-refresh re-observes every source (refresh_projects), folds each returned row into the
  // Index cache without a refetch wave, and reports partial failures assertively so a source
  // that could not be read is never silently dropped.
  const onRefresh = useCallback(async () => {
    announce("polite", "Refreshing projects…");
    try {
      const results = await api.refreshProjects(null);
      queryClient.setQueryData<ProjectIndexResponse>(queryKeys.projectIndex, (current) => {
        if (!current) return current;
        const byId = new Map(results.filter((r) => r.item).map((r) => [r.project_id, r.item!]));
        return { ...current, projects: current.projects.map((p) => byId.get(p.project_id) ?? p) };
      });
      const failed = results.filter((r) => r.outcome === "source_failed");
      if (failed.length > 0) {
        announce(
          "assertive",
          `${failed.length} project${failed.length === 1 ? "" : "s"} could not be refreshed.`,
        );
      } else {
        announce("polite", "Projects refreshed.");
      }
    } catch (raw) {
      const err = raw instanceof AppError ? raw : new AppError({ code: "unknown", message: "Refresh failed.", retryable: false, stateApplied: false });
      announce("assertive", err.message);
    }
  }, [announce, queryClient]);

  const openAddProject = useCallback(() => setAddProjectOpen(true), []);
  const appActions = useMemo(
    () => ({ openAddProject, refresh: onRefresh }),
    [openAddProject, onRefresh],
  );

  // Escape closes only the topmost surface: the Add Project modal before the Peek. Closing a
  // Peek pops history (rather than pushing a fresh Index entry) so the original Index entry —
  // which carries scroll offset and return-focus id in history state — is restored, and Back
  // does not re-open the just-dismissed Peek.
  const handleEscape = useCallback(() => {
    if (addProjectOpen) {
      setAddProjectOpen(false);
      return;
    }
    if (effectiveBackgroundLocation) {
      navigate(-1);
    }
  }, [addProjectOpen, effectiveBackgroundLocation, navigate]);

  useAppShortcuts({
    onFocusFilter: () => filterRef.current?.focus(),
    onOpenAddProject: openAddProject,
    onRefresh,
    onEscape: handleEscape,
  });

  return (
    <AnnouncerContext.Provider value={announce}>
      <AppActionsContext.Provider value={appActions}>
      <div className="app-shell">
        <header className="app-shell__bar">
          <nav aria-label="Primary">
            <Link className="app-shell__brand" to={projectsPath()} aria-label="Projects">
              <span className="app-shell__brand-mark" aria-hidden="true">
                O
              </span>
              <span className="app-shell__brand-copy" aria-hidden="true">
                <strong>OmniProj</strong>
                <small>Research workspace</small>
              </span>
            </Link>
          </nav>
          <div className="app-shell__search" role="search">
            <input
              ref={filterRef}
              type="search"
              aria-label="Filter projects"
              placeholder="Filter projects…"
              value={filterValue}
              onChange={onFilterChange}
            />
            <kbd aria-hidden="true">⌘F</kbd>
          </div>
          <div className="app-shell__actions">
            <button
              className="op-button op-button--primary"
              type="button"
              onClick={openAddProject}
            >
              Add Project
            </button>
            <button
              className="op-button op-button--secondary"
              type="button"
              onClick={onRefresh}
            >
              Refresh
            </button>
          </div>
        </header>

        {/* The main outlet renders the background (Index) while a Peek is open, so the Index
            stays mounted underneath it. */}
        <Routes location={effectiveBackgroundLocation ?? location}>
          <Route path={ROUTES.root} element={<Navigate to={projectsPath()} replace />} />
          <Route path={ROUTES.projects} element={<ProjectsIndexPage />} />
          <Route path={ROUTES.projectById} element={<ProjectIdRedirect />} />
          <Route path={ROUTES.projectOverview} element={<ProjectOverviewPage />} />
          <Route path={ROUTES.notFound} element={<NotFoundPage />} />
        </Routes>

        {/* The Peek overlay: the same canonical Overview URL, rendered over the still-mounted
            Index — but only on a wide viewport. */}
        {effectiveBackgroundLocation && (
          <Routes>
            <Route path={ROUTES.projectOverview} element={<ProjectPeek />} />
          </Routes>
        )}

        {addProjectOpen && <AddProjectDialog onClose={() => setAddProjectOpen(false)} />}

        <LiveStatus polite={polite} assertive={assertive} />
      </div>
      </AppActionsContext.Provider>
    </AnnouncerContext.Provider>
  );
}
