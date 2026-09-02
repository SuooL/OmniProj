// The persistent project rail: a desktop master pane. Switching projects is a single click
// or an arrow key away, and the list never disappears when a project is open — the thing the
// user manipulates is a COLLECTION, so the collection stays on screen.
//
// The rail is navigation only. Portfolio reasoning (review reasons, grouping, the focus
// strip) still belongs to the Projects surface in the detail pane.

import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import type { ProjectIndexItem } from "../domain/project";
import { projectOverviewPath, projectsPath } from "../domain/routes";
import { useI18n } from "../i18n/I18nProvider";

export const RAIL_WIDTH_STORAGE_KEY = "omniproj.rail-width";
export const RAIL_COLLAPSED_STORAGE_KEY = "omniproj.rail-collapsed";
const RAIL_MIN = 180;
const RAIL_MAX = 420;
const RAIL_DEFAULT = 248;

function storedNumber(key: string, fallback: number): number {
  if (typeof window === "undefined") return fallback;
  try {
    const raw = Number(window.localStorage.getItem(key));
    return Number.isFinite(raw) && raw >= RAIL_MIN && raw <= RAIL_MAX ? raw : fallback;
  } catch {
    return fallback;
  }
}

function storedFlag(key: string): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

export interface ProjectRailProps {
  projects: ProjectIndexItem[];
  /** The project currently shown in the detail pane, if any. */
  activeId: string | null;
}

export function ProjectRail({ projects, activeId }: ProjectRailProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [filter, setFilter] = useState("");
  const [width, setWidth] = useState(() => storedNumber(RAIL_WIDTH_STORAGE_KEY, RAIL_DEFAULT));
  const [collapsed, setCollapsed] = useState(() => storedFlag(RAIL_COLLAPSED_STORAGE_KEY));
  const listRef = useRef<HTMLUListElement>(null);

  const visible = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return projects;
    return projects.filter((project) => project.name.toLowerCase().includes(needle));
  }, [filter, projects]);

  function persistWidth(next: number) {
    setWidth(next);
    try { window.localStorage.setItem(RAIL_WIDTH_STORAGE_KEY, String(next)); } catch { /* best effort */ }
  }

  function toggleCollapsed() {
    setCollapsed((current) => {
      const next = !current;
      try { window.localStorage.setItem(RAIL_COLLAPSED_STORAGE_KEY, String(next)); } catch { /* best effort */ }
      return next;
    });
  }

  // Drag the divider; the pointer is captured so the drag survives leaving the handle.
  function onHandlePointerDown(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    const handle = event.currentTarget;
    handle.setPointerCapture(event.pointerId);
    const onMove = (move: PointerEvent) => {
      const next = Math.min(RAIL_MAX, Math.max(RAIL_MIN, startWidth + (move.clientX - startX)));
      setWidth(next);
    };
    const onUp = (up: PointerEvent) => {
      handle.releasePointerCapture(up.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      persistWidth(Math.min(RAIL_MAX, Math.max(RAIL_MIN, startWidth + (up.clientX - startX))));
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  }

  /** Arrow keys move between projects; Enter/Space opens. Home/End jump to the ends. */
  function onListKeyDown(event: React.KeyboardEvent<HTMLUListElement>) {
    const items = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-rail-item]") ?? []);
    if (items.length === 0) return;
    const index = items.findIndex((item) => item === document.activeElement);
    const go = (next: number) => {
      event.preventDefault();
      items[Math.min(items.length - 1, Math.max(0, next))]?.focus();
    };
    if (event.key === "ArrowDown") go(index + 1);
    else if (event.key === "ArrowUp") go(index - 1);
    else if (event.key === "Home") go(0);
    else if (event.key === "End") go(items.length - 1);
  }

  // Keep the open project scrolled into view when it changes from elsewhere (menu, shortcut).
  useEffect(() => {
    if (!activeId) return;
    const item = listRef.current?.querySelector<HTMLElement>(
      `[data-rail-item="${CSS.escape(activeId)}"]`,
    );
    // jsdom has no scroll implementation; the guard keeps unit tests rendering the rail.
    if (typeof item?.scrollIntoView === "function") item.scrollIntoView({ block: "nearest" });
  }, [activeId]);

  if (collapsed) {
    return (
      <nav className="app-rail app-rail--collapsed" aria-label={t("rail.label")} data-testid="project-rail">
        <button
          type="button"
          className="op-chrome-button app-rail__toggle"
          aria-expanded={false}
          aria-label={t("rail.expand")}
          onClick={toggleCollapsed}
        >
          ›
        </button>
      </nav>
    );
  }

  return (
    <nav
      className="app-rail"
      aria-label={t("rail.label")}
      data-testid="project-rail"
      style={{ width }}
    >
      <div className="app-rail__head">
        <input
          type="search"
          className="op-control"
          data-project-filter
          aria-label={t("rail.search")}
          placeholder={t("rail.search")}
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
        />
        <button
          type="button"
          className="op-chrome-button app-rail__toggle"
          aria-expanded
          aria-label={t("rail.collapse")}
          onClick={toggleCollapsed}
        >
          ‹
        </button>
      </div>

      <button
        type="button"
        className={`app-rail__all${activeId === null ? " is-active" : ""}`}
        aria-current={activeId === null ? "page" : undefined}
        onClick={() => navigate(projectsPath())}
      >
        {t("rail.allProjects")}
        <span className="op-section__count">{projects.length}</span>
      </button>

      {visible.length === 0 ? (
        <p className="app-rail__empty op-muted">{t("rail.noMatch")}</p>
      ) : (
        <ul className="app-rail__list" ref={listRef} onKeyDown={onListKeyDown}>
          {visible.map((project) => {
            const active = project.project_id === activeId;
            const needsDecision = project.review_reasons.length > 0;
            return (
              <li key={project.project_id}>
                <button
                  type="button"
                  data-rail-item={project.project_id}
                  className={`app-rail__item${active ? " is-active" : ""}`}
                  aria-current={active ? "page" : undefined}
                  onClick={() => navigate(projectOverviewPath(project.project_id))}
                >
                  {/* Non-color redundancy: the dot is accompanied by an accessible label. */}
                  <span
                    className={`app-rail__dot${needsDecision ? " is-flagged" : ""}`}
                    aria-hidden="true"
                  />
                  <span className="app-rail__name">{project.name}</span>
                  {needsDecision && <span className="op-visually-hidden">{t("rail.needsDecision")}</span>}
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <div
        className="app-rail__handle"
        role="separator"
        aria-orientation="vertical"
        aria-label={t("rail.resize")}
        aria-valuenow={width}
        aria-valuemin={RAIL_MIN}
        aria-valuemax={RAIL_MAX}
        onPointerDown={onHandlePointerDown}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft") persistWidth(Math.max(RAIL_MIN, width - 16));
          if (event.key === "ArrowRight") persistWidth(Math.min(RAIL_MAX, width + 16));
        }}
        tabIndex={0}
      />
    </nav>
  );
}
