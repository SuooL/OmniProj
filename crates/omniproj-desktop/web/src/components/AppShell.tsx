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

import { saveCanonicalLocation } from "../domain/navigationSession";
import { projectId as brandProjectId } from "../domain/project";
import {
  ROUTES,
  projectOverviewPath,
  projectsPath,
} from "../domain/routes";
import { useAppShortcuts } from "../hooks/useAppShortcuts";
import { queryKeys } from "../queryKeys";
import { NotFoundPage } from "../routes/NotFoundPage";
import { ProjectOverviewPage } from "../routes/ProjectOverviewPage";
import { ProjectsIndexPage } from "../routes/ProjectsIndexPage";
import { LiveStatus } from "./LiveStatus";

// --- Announcer -------------------------------------------------------------
// Any screen can announce without owning a live region: the two persistent regions live in
// the shell, and this context routes a message to the polite or assertive one.
export type AnnounceLevel = "polite" | "assertive";
export type Announce = (level: AnnounceLevel, message: string) => void;

const AnnouncerContext = createContext<Announce | null>(null);

/** Announce a polite (progress) or assertive (error) message through the shell's regions. */
export function useAnnouncer(): Announce {
  const announce = useContext(AnnouncerContext);
  if (announce === null) {
    throw new Error("useAnnouncer must be used within the AppShell");
  }
  return announce;
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

  const backgroundLocation =
    (location.state as BackgroundState | null)?.backgroundLocation ?? null;

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

  const onRefresh = useCallback(() => {
    announce("polite", "Refreshing projects…");
    void queryClient.refetchQueries({ queryKey: queryKeys.projectIndex });
  }, [announce, queryClient]);

  const openAddProject = useCallback(() => setAddProjectOpen(true), []);

  // Escape closes only the topmost surface: the Add Project modal before the Peek. Closing a
  // Peek pops history (rather than pushing a fresh Index entry) so the original Index entry —
  // which carries scroll offset and return-focus id in history state — is restored, and Back
  // does not re-open the just-dismissed Peek.
  const handleEscape = useCallback(() => {
    if (addProjectOpen) {
      setAddProjectOpen(false);
      return;
    }
    if (backgroundLocation) {
      navigate(-1);
    }
  }, [addProjectOpen, backgroundLocation, navigate]);

  useAppShortcuts({
    onFocusFilter: () => filterRef.current?.focus(),
    onOpenAddProject: openAddProject,
    onRefresh,
    onEscape: handleEscape,
  });

  return (
    <AnnouncerContext.Provider value={announce}>
      <div className="app-shell">
        <header className="app-shell__bar">
          <nav aria-label="Primary">
            <Link to={projectsPath()}>Projects</Link>
          </nav>
          <div role="search">
            <input
              ref={filterRef}
              type="search"
              aria-label="Filter projects"
              value={filterValue}
              onChange={onFilterChange}
            />
          </div>
          <button type="button" onClick={openAddProject}>
            Add Project
          </button>
          <button type="button" onClick={onRefresh}>
            Refresh
          </button>
        </header>

        {/* The main outlet renders the background (Index) while a Peek is open, so the Index
            stays mounted underneath it. */}
        <Routes location={backgroundLocation ?? location}>
          <Route path={ROUTES.root} element={<Navigate to={projectsPath()} replace />} />
          <Route path={ROUTES.projects} element={<ProjectsIndexPage />} />
          <Route path={ROUTES.projectById} element={<ProjectIdRedirect />} />
          <Route
            path={ROUTES.projectOverview}
            element={<ProjectOverviewPage variant="page" />}
          />
          <Route path={ROUTES.notFound} element={<NotFoundPage />} />
        </Routes>

        {/* The Peek overlay: the same canonical Overview URL, rendered over the Index. */}
        {backgroundLocation && (
          <Routes>
            <Route
              path={ROUTES.projectOverview}
              element={<ProjectOverviewPage variant="peek" />}
            />
          </Routes>
        )}

        {addProjectOpen && (
          <div role="dialog" aria-modal="true" aria-label="Add Project">
            <h2>Add Project</h2>
            <button type="button" onClick={() => setAddProjectOpen(false)}>
              Close
            </button>
          </div>
        )}

        <LiveStatus polite={polite} assertive={assertive} />
      </div>
    </AnnouncerContext.Provider>
  );
}
