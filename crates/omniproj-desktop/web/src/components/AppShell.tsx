// The desktop interaction frame: a full-height project tree, native window controls, the main
// route surface, the Add Project modal, global shortcuts, and persistent announcement regions.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";

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
import { queryKeys } from "../queryKeys";
import { NotFoundPage } from "../routes/NotFoundPage";
import { ProjectOverviewPage } from "../routes/ProjectOverviewPage";
import { ProjectsIndexPage } from "../routes/ProjectsIndexPage";
import { AddProjectDialog } from "./AddProjectDialog";
import {
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  FolderIcon,
  PlusIcon,
  RefreshIcon,
  SidebarIcon,
} from "./Icons";
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
  const [sidebarOpen, setSidebarOpen] = useState(
    () => typeof window === "undefined" || window.innerWidth >= 800,
  );
  const [polite, setPolite] = useState("");
  const [assertive, setAssertive] = useState("");
  const { data: sidebarProjects } = useQuery({
    queryKey: queryKeys.projectIndex,
    queryFn: api.listProjectIndex,
  });

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
      setSearchParams(next, { replace: true });
    },
    [searchParams, setSearchParams],
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

  const handleEscape = useCallback(() => {
    if (addProjectOpen) {
      setAddProjectOpen(false);
      return;
    }
  }, [addProjectOpen]);

  const showBack = location.pathname !== projectsPath();
  const sidebarProjectItems = Array.isArray(sidebarProjects?.projects)
    ? sidebarProjects.projects
    : [];
  const currentProject = sidebarProjectItems.find((project) =>
    location.pathname.includes(`/projects/${project.project_id}`),
  );

  const startWindowDrag = useCallback((event: React.MouseEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest("button, input, select, textarea, a, [role='button']")) return;
    void getCurrentWindow().startDragging().catch(() => {
      // Browser tests and preview builds do not expose a native window.
    });
  }, []);

  useAppShortcuts({
    onFocusFilter: () => filterRef.current?.focus(),
    onOpenAddProject: openAddProject,
    onRefresh,
    onEscape: handleEscape,
  });

  return (
    <AnnouncerContext.Provider value={announce}>
      <AppActionsContext.Provider value={appActions}>
      <div className="app-shell" data-sidebar-open={sidebarOpen}>
        <aside className="app-shell__sidebar">
          <div
            className="app-shell__sidebar-chrome"
            data-tauri-drag-region
            onMouseDown={startWindowDrag}
          >
            <button
              className="op-chrome-button"
              type="button"
              aria-label="Hide sidebar"
              onClick={() => setSidebarOpen(false)}
            >
              <SidebarIcon />
            </button>
            <button
              className="op-chrome-button"
              type="button"
              aria-label="Back to projects"
              onClick={() => navigate(projectsPath())}
              disabled={!showBack}
            >
              <ChevronLeftIcon />
            </button>
            <button className="op-chrome-button" type="button" aria-label="Forward" disabled>
              <ChevronRightIcon />
            </button>
          </div>
          <div className="app-shell__sidebar-body">
            <div className="app-shell__brand-row">
              <strong>OmniProj</strong>
              <ChevronDownIcon />
            </div>
            <div className="app-shell__search" role="search">
              <input
                ref={filterRef}
                type="search"
                aria-label="Filter projects"
                placeholder="Search projects"
                value={filterValue}
                onChange={onFilterChange}
              />
              <kbd aria-hidden="true">⌘F</kbd>
            </div>
            <nav aria-label="Primary" className="app-shell__nav">
              <div className="app-shell__nav-section-title">
                <span>Projects</span>
                <button type="button" onClick={openAddProject} aria-label="Add Project"><PlusIcon /></button>
              </div>
              <div className="app-shell__project-tree">
                {sidebarProjectItems
                  .filter((project) => project.status !== "archived")
                  .map((project) => {
                    const active = location.pathname.includes(`/projects/${project.project_id}`);
                    return (
                      <div className="app-shell__project-node" key={project.project_id}>
                        <button
                          className="app-shell__project-item"
                          data-active={active}
                          type="button"
                          onClick={() => navigate(projectOverviewPath(project.project_id))}
                        >
                          <FolderIcon />
                          <span>{project.name}</span>
                        </button>
                        {active && (
                          <div className="app-shell__project-children">
                            <span className="is-active">Overview</span>
                            <span>Commitment</span>
                            <span>Activity</span>
                          </div>
                        )}
                      </div>
                    );
                  })}
              </div>
            </nav>
            <div className="app-shell__sidebar-footer">
              <span className="app-shell__avatar" aria-hidden="true">S</span>
              <span>Suool David</span>
            </div>
          </div>
        </aside>

        <section className="app-shell__main">
          <header
            className="app-shell__context-bar"
            data-tauri-drag-region
            onMouseDown={startWindowDrag}
          >
            <div className="app-shell__context-title">
              <button
                className="app-shell__sidebar-toggle"
                type="button"
                aria-label="Show sidebar"
                onClick={() => setSidebarOpen(true)}
              >
                <SidebarIcon />
              </button>
              <FolderIcon />
              <strong>{currentProject?.name ?? "Projects"}</strong>
            </div>
            <div className="app-shell__actions">
              <button type="button" onClick={onRefresh} aria-label="Refresh"><RefreshIcon /></button>
              <button type="button" onClick={openAddProject} aria-label="New project"><PlusIcon /></button>
            </div>
          </header>
          <div className="app-shell__content">
            <Routes>
              <Route path={ROUTES.root} element={<Navigate to={projectsPath()} replace />} />
              <Route path={ROUTES.projects} element={<ProjectsIndexPage />} />
              <Route path={ROUTES.projectById} element={<ProjectIdRedirect />} />
              <Route path={ROUTES.projectOverview} element={<ProjectOverviewPage />} />
              <Route path={ROUTES.notFound} element={<NotFoundPage />} />
            </Routes>
          </div>
        </section>

        {addProjectOpen && <AddProjectDialog onClose={() => setAddProjectOpen(false)} />}

        <LiveStatus polite={polite} assertive={assertive} />
      </div>
      </AppActionsContext.Provider>
    </AnnouncerContext.Provider>
  );
}
