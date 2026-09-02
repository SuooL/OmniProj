// The desktop interaction frame: a persistent project rail (master) beside the route surface
// (detail), plus compact native chrome, global actions, the Add Project modal, shortcuts, and
// persistent announcement regions.
//
// The rail is permanent on purpose. The user manipulates a COLLECTION of parallel projects,
// so the collection stays on screen and switching costs one click or one arrow key instead of
// a full page transition.

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
} from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api } from "../api";
import { saveCanonicalLocation } from "../domain/navigationSession";
import { projectId as brandProjectId } from "../domain/project";
import type { ProjectIndexResponse } from "../domain/project";
import {
  ROUTES,
  projectOverviewPath,
  projectsPath,
} from "../domain/routes";
import { useAppShortcuts } from "../hooks/useAppShortcuts";
import { useI18n } from "../i18n/I18nProvider";
import { queryKeys } from "../queryKeys";
import { NotFoundPage } from "../routes/NotFoundPage";
import { ProjectOverviewPage } from "../routes/ProjectOverviewPage";
import { ProjectsIndexPage } from "../routes/ProjectsIndexPage";
import { SettingsPage } from "../routes/SettingsPage";
import { AddProjectDialog } from "./AddProjectDialog";
import { ProjectRail } from "./ProjectRail";
import {
  ChevronLeftIcon,
  GearIcon,
  PlusIcon,
  RefreshIcon,
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
  const { t } = useI18n();
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [addProjectOpen, setAddProjectOpen] = useState(false);
  const [polite, setPolite] = useState("");
  const [assertive, setAssertive] = useState("");
  const [refreshProgress, setRefreshProgress] = useState<{ completed: number; total: number } | null>(null);
  const refreshInFlight = useRef(false);
  const startupRefreshStarted = useRef(false);
  const { data: projectIndex } = useQuery({
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

  const foldRefreshResults = useCallback(
    (results: Awaited<ReturnType<typeof api.refreshProjects>>) => {
      queryClient.setQueryData<ProjectIndexResponse>(queryKeys.projectIndex, (current) => {
        if (!current) return current;
        const byId = new Map(results.filter((r) => r.item).map((r) => [r.project_id, r.item!]));
        return { ...current, projects: current.projects.map((p) => byId.get(p.project_id) ?? p) };
      });
    },
    [queryClient],
  );

  // Refresh each source independently so completed rows appear immediately and one slow or
  // unavailable repository cannot hide progress from the rest of the workspace.
  const onRefresh = useCallback(async (silentEmpty = false) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    const cached = queryClient.getQueryData<ProjectIndexResponse>(queryKeys.projectIndex);
    const ids = Array.isArray(cached?.projects)
      ? cached.projects.map((project) => project.project_id)
      : [];
    if (ids.length === 0) {
      refreshInFlight.current = false;
      if (!silentEmpty) announce("polite", t("shell.upToDate"));
      return;
    }
    setRefreshProgress({ completed: 0, total: ids.length });
    announce("polite", t("shell.refreshStarted"));
    let failed = 0;
    await Promise.all(
      ids.map(async (id) => {
        try {
          const results = await api.refreshProjects([id]);
          foldRefreshResults(results);
          failed += results.filter((result) => result.outcome === "source_failed").length;
        } catch {
          failed += 1;
        } finally {
          setRefreshProgress((current) => current
            ? { ...current, completed: current.completed + 1 }
            : current);
        }
      }),
    );
    try {
      if (failed > 0) {
        announce(
          "assertive",
          t("shell.refreshFailed", { count: failed }),
        );
      } else {
        announce("polite", t("shell.refreshed"));
      }
    } finally {
      refreshInFlight.current = false;
      setRefreshProgress(null);
    }
  }, [announce, foldRefreshResults, queryClient, t]);

  // The persisted cache makes startup instant; this background pass then reconciles it with
  // the current repositories once per application mount.
  useEffect(() => {
    if (!projectIndex || startupRefreshStarted.current) return;
    startupRefreshStarted.current = true;
    void onRefresh(true);
  }, [onRefresh, projectIndex]);

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
  const projectItems = Array.isArray(projectIndex?.projects)
    ? projectIndex.projects
    : [];
  const currentProject = projectItems.find((project) =>
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
    // The permanent rail is the project filter on every screen, so the shortcut has one
    // unambiguous target instead of depending on which route is open.
    onFocusFilter: () => document.querySelector<HTMLInputElement>("[data-project-filter]")?.focus(),
    onOpenAddProject: openAddProject,
    onRefresh,
    onEscape: handleEscape,
  });

  return (
    <AnnouncerContext.Provider value={announce}>
      <AppActionsContext.Provider value={appActions}>
      <div className="app-shell">
        <section className="app-shell__main">
          <header className="app-shell__topbar" data-tauri-drag-region onMouseDown={startWindowDrag}>
            <div className="app-shell__topbar-context">
              {showBack && (
                <button className="op-chrome-button" type="button" aria-label={t("shell.backProjects")} onClick={() => navigate(projectsPath())}>
                  <ChevronLeftIcon />
                </button>
              )}
              <button className="app-shell__brand" type="button" onClick={() => navigate(projectsPath())}>OmniProj</button>
              {showBack && <span className="app-shell__context-name">{location.pathname === ROUTES.settings ? t("shell.settings") : currentProject?.name}</span>}
            </div>
            <div className="app-shell__actions">
              {refreshProgress && (
                <span className="app-shell__refresh-progress" role="status">
                  {refreshProgress.completed}/{refreshProgress.total}
                </span>
              )}
              <button
                type="button"
                onClick={() => void onRefresh()}
                aria-label={refreshProgress ? t("shell.refreshing") : t("shell.refresh")}
                disabled={refreshProgress !== null}
                data-refreshing={refreshProgress !== null}
              ><RefreshIcon /></button>
              <button type="button" onClick={openAddProject} aria-label={t("shell.newProject")}><PlusIcon /></button>
              <button type="button" onClick={() => navigate(ROUTES.settings)} aria-label={t("shell.settings")} aria-current={location.pathname === ROUTES.settings ? "page" : undefined}><GearIcon /></button>
            </div>
          </header>
          <div className="app-shell__body">
            <ProjectRail projects={projectItems} activeId={currentProject?.project_id ?? null} />
            <div className="app-shell__content">
            <Routes>
              <Route path={ROUTES.root} element={<Navigate to={projectsPath()} replace />} />
              <Route path={ROUTES.projects} element={<ProjectsIndexPage />} />
              <Route path={ROUTES.projectById} element={<ProjectIdRedirect />} />
              <Route path={ROUTES.projectOverview} element={<ProjectOverviewPage />} />
              <Route path={ROUTES.settings} element={<SettingsPage />} />
              <Route path={ROUTES.notFound} element={<NotFoundPage />} />
            </Routes>
            </div>
          </div>
        </section>

        {addProjectOpen && <AddProjectDialog onClose={() => setAddProjectOpen(false)} />}

        <LiveStatus polite={polite} assertive={assertive} />
      </div>
      </AppActionsContext.Provider>
    </AnnouncerContext.Provider>
  );
}
