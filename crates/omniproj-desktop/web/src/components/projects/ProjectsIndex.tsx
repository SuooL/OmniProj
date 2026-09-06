// The project list. Search, sort, open — nothing else.
//
// It used to split the list into "Needs a decision" and "Other projects" and stamp review
// badges on rows. That is the tool deciding what matters. A list of your own projects,
// in an order you chose, is what a notebook's contents page is.

import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProjectIndexItem } from "../../domain/project";
import { applyReviewFilter, filterByText } from "../../domain/projectPresentation";
import { ProjectRow } from "./ProjectRow";
import { useI18n } from "../../i18n/I18nProvider";

type SortMode = "recent" | "name" | "remaining";

function parseSort(value: string | null): SortMode {
  return value === "name" || value === "remaining" ? value : "recent";
}

function applySort(items: ProjectIndexItem[], sort: SortMode): ProjectIndexItem[] {
  switch (sort) {
    case "name":
      return [...items].sort((a, b) => a.name.localeCompare(b.name));
    case "remaining":
      return [...items].sort((a, b) => b.open_steps - a.open_steps);
    case "recent":
      return [...items].sort((a, b) => {
        const at = a.observed_actual?.last_commit?.committed_at ?? "";
        const bt = b.observed_actual?.last_commit?.committed_at ?? "";
        return bt.localeCompare(at);
      });
  }
}

export interface ProjectsIndexProps {
  projects: ProjectIndexItem[];
  now: Date;
  onAddProject: () => void;
}

export function ProjectsIndex({ projects, now, onAddProject }: ProjectsIndexProps) {
  const { t } = useI18n();
  const [searchParams, setSearchParams] = useSearchParams();
  const query = searchParams.get("q") ?? "";
  const sort = parseSort(searchParams.get("sort"));
  const showArchived = searchParams.get("archived") === "1";

  const setParam = (key: string, value: string, keep: boolean) => {
    const next = new URLSearchParams(searchParams);
    if (keep) next.set(key, value);
    else next.delete(key);
    setSearchParams(next, { replace: true, state: null });
  };

  const visible = useMemo(() => {
    // Archived projects are finished business; they stay out of the way until asked for.
    const base = applyReviewFilter(projects, showArchived ? "archived" : "all");
    return applySort(filterByText(base, query), sort);
  }, [projects, query, showArchived, sort]);

  if (projects.length === 0) {
    return (
      <section data-testid="projects-index-empty" aria-labelledby="projects-empty-heading">
        <h2 id="projects-empty-heading">{t("index.emptyTitle")}</h2>
        <p>{t("index.emptyBody")}</p>
        <button type="button" className="op-primary" onClick={onAddProject}>
          {t("index.addProject")}
        </button>
      </section>
    );
  }

  return (
    <section className="op-index" aria-labelledby="projects-index-heading">
      <div className="op-index__toolbar">
        <div className="op-index__search" role="search">
          <input
            type="search"
            aria-label={t("shell.filterProjects")}
            placeholder={t("shell.searchProjects")}
            value={query}
            onChange={(event) => setParam("q", event.target.value, event.target.value !== "")}
          />
          <kbd aria-hidden="true">⌘F</kbd>
        </div>

        <div className="op-index__controls">
          <label className="op-sort">
            <span>{t("index.sort")}</span>
            <select
              aria-label={t("index.sort")}
              value={sort}
              onChange={(event) => setParam("sort", event.target.value, event.target.value !== "recent")}
            >
              <option value="recent">{t("index.sortRecentCommit")}</option>
              <option value="name">{t("index.sortName")}</option>
              <option value="remaining">{t("index.sortRemaining")}</option>
            </select>
          </label>
          <label className="op-index__archived">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(event) => setParam("archived", "1", event.target.checked)}
            />
            <span>{t("index.filterArchived")}</span>
          </label>
        </div>
      </div>

      <div className="op-index__table">
        {visible.length === 0 ? (
          <p className="op-index__nomatch" data-testid="projects-index-nomatch">
            {t("index.noMatch")}
          </p>
        ) : (
          <ul className="op-index__list" aria-label={t("shell.projects")}>
            {visible.map((item) => <ProjectRow key={item.project_id} item={item} now={now} />)}
          </ul>
        )}
      </div>
    </section>
  );
}
