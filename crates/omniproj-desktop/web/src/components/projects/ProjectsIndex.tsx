// The re-entry queue. Search and the two routine views stay visible; lifecycle filters and
// alternative sorts are progressively disclosed. Default ordering remains the backend's
// deterministic review order and is never converted into a health or priority score.

import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProjectIndexItem, ReviewPolicy } from "../../domain/project";
import {
  applyReviewFilter,
  filterByText,
  type ReviewFilter,
} from "../../domain/projectPresentation";
import { FilterChip } from "../semantic/FilterChip";
import { ProjectRow } from "./ProjectRow";
import { useI18n } from "../../i18n/I18nProvider";

type SortMode = "review" | "name" | "commit";

function parseFilter(value: string | null): ReviewFilter {
  return value === "needs_review" || value === "waiting" || value === "parked" || value === "archived"
    ? value
    : "all";
}

function parseSort(value: string | null): SortMode {
  return value === "name" || value === "commit" || value === "observed"
    ? value === "observed" ? "commit" : value
    : "review";
}

/** Transparent, opt-in sort. `review` preserves the backend order verbatim (no re-ranking). */
function applySort(items: ProjectIndexItem[], sort: SortMode): ProjectIndexItem[] {
  switch (sort) {
    case "review":
      return items;
    case "name":
      return [...items].sort((a, b) => a.name.localeCompare(b.name));
    case "commit":
      return [...items].sort((a, b) => {
        const at = a.observed_actual?.last_commit?.committed_at ?? "";
        const bt = b.observed_actual?.last_commit?.committed_at ?? "";
        return bt.localeCompare(at);
      });
  }
}

export interface ProjectsIndexProps {
  projects: ProjectIndexItem[];
  reviewPolicy: ReviewPolicy;
  now: Date;
  onAddProject: () => void;
}

export function ProjectsIndex({
  projects,
  reviewPolicy,
  now,
  onAddProject,
}: ProjectsIndexProps) {
  const { t } = useI18n();
  const secondaryFilters: Array<{ value: ReviewFilter; label: string }> = [
    { value: "waiting", label: t("index.filterWaiting") },
    { value: "parked", label: t("index.filterParked") },
    { value: "archived", label: t("index.filterArchived") },
  ];
  const [searchParams, setSearchParams] = useSearchParams();
  const query = searchParams.get("q") ?? "";
  const filter = parseFilter(searchParams.get("filter"));
  const sort = parseSort(searchParams.get("sort"));

  const setParam = (key: string, value: string, keep: boolean) => {
    const next = new URLSearchParams(searchParams);
    if (keep) next.set(key, value);
    else next.delete(key);
    setSearchParams(next, { replace: true, state: null });
  };

  const visible = useMemo(() => {
    const base = applyReviewFilter(projects, filter);
    return applySort(filterByText(base, query), sort);
  }, [projects, filter, query, sort]);

  const isDefaultView = query.trim() === "" && filter === "all" && sort === "review";
  const needsDecision = isDefaultView ? visible.filter((item) => item.review_reasons.length > 0) : [];
  const otherProjects = isDefaultView ? visible.filter((item) => item.review_reasons.length === 0) : [];

  const renderRows = (items: ProjectIndexItem[], label: string) => (
    <ul className="op-index__list" aria-label={label}>
      {items.map((item) => <ProjectRow key={item.project_id} item={item} now={now} />)}
    </ul>
  );

  // A truly empty store offers the primary recovery action. Archived-only stores retain the
  // toolbar so the Archived filter remains an obvious recovery path.
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
            data-project-filter
            type="search"
            aria-label={t("shell.filterProjects")}
            placeholder={t("shell.searchProjects")}
            value={query}
            onChange={(event) => setParam("q", event.target.value, event.target.value !== "")}
          />
          <kbd aria-hidden="true">⌘F</kbd>
        </div>

        <div className="op-index__controls">
          <div className="op-filters" role="group" aria-label={t("index.reviewFilters")}>
            <FilterChip label={t("index.filterAll")} pressed={filter === "all"} onClick={() => setParam("filter", "all", false)} />
            <FilterChip label={t("index.filterNeedsReview")} pressed={filter === "needs_review"} onClick={() => setParam("filter", "needs_review", true)} />
          </div>
          <details className="op-index__more">
            <summary>{t("index.moreFilters")}</summary>
            <div className="op-index__more-panel">
              <div className="op-filters" role="group" aria-label={t("index.lifecycleFilters")}>
                {secondaryFilters.map((chip) => <FilterChip key={chip.value} label={chip.label} pressed={filter === chip.value} onClick={() => setParam("filter", chip.value, true)} />)}
              </div>
              <label className="op-sort">
                <span>{t("index.sort")}</span>
                <select aria-label={t("index.reviewOrder")} value={sort} onChange={(e) => setParam("sort", e.target.value, e.target.value !== "review")}>
                  <option value="review">{t("index.reviewOrder")}</option>
                  <option value="name">{t("index.sortName")}</option>
                  <option value="commit">{t("index.sortRecentCommit")}</option>
                </select>
              </label>
              <small>{t("index.reviewInterval", { days: reviewPolicy.commitment_review_days })}</small>
            </div>
          </details>
        </div>
      </div>

      <div className="op-index__table">
        {visible.length === 0 ? (
          <p className="op-index__nomatch" data-testid="projects-index-nomatch">
            {t("index.noMatch")}
          </p>
        ) : (
          isDefaultView ? (
            <div className="op-index__groups">
              {needsDecision.length > 0 && <section><h2>{t("index.needsDecision")}</h2>{renderRows(needsDecision, t("index.needsDecision"))}</section>}
              {otherProjects.length > 0 && <section><h2>{t("index.otherProjects")}</h2>{renderRows(otherProjects, t("index.otherProjects"))}</section>}
            </div>
          ) : renderRows(visible, t("shell.projects"))
        )}
      </div>
    </section>
  );
}
