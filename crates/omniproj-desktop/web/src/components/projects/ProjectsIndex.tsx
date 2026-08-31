// The dense four-column operating index. The default order is the backend's deterministic
// review order — this component NEVER re-ranks it; alternative sorts are transparent, opt-in,
// and clearly not a priority/health score. Filter (text + review chips) and sort live in the
// canonical search params. The 7-day review interval is read from the response's review_policy,
// never a frontend constant.

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
  silentDaysThreshold?: number;
}

export function ProjectsIndex({
  projects,
  reviewPolicy,
  now,
  onAddProject,
  silentDaysThreshold = 7,
}: ProjectsIndexProps) {
  const { t } = useI18n();
  const reviewFilters: Array<{ value: ReviewFilter; label: string }> = [
    { value: "all", label: t("index.filterAll") },
    { value: "needs_review", label: t("index.filterNeedsReview") },
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
        <div className="op-index__meta">
          <span className="op-index__order">{t("index.reviewOrderDetail")}</span>
          <span className="op-index__interval">
            {t("index.reviewInterval", { days: reviewPolicy.commitment_review_days })}
          </span>
        </div>

        <div className="op-index__controls">
          <div className="op-filters" role="group" aria-label={t("index.reviewFilters")}>
            {reviewFilters.map((chip) => (
              <FilterChip
                key={chip.value}
                label={chip.label}
                pressed={filter === chip.value}
                onClick={() => setParam("filter", chip.value, chip.value !== "all")}
              />
            ))}
          </div>
          <label className="op-sort">
            <span>{t("index.sort")}</span>
            <select
              aria-label={t("index.reviewOrder")}
              value={sort}
              onChange={(e) => setParam("sort", e.target.value, e.target.value !== "review")}
            >
              <option value="review">{t("index.reviewOrder")}</option>
              <option value="name">{t("index.sortName")}</option>
              <option value="commit">{t("index.sortRecentCommit")}</option>
            </select>
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
            {visible.map((item) => (
              <ProjectRow key={item.project_id} item={item} now={now} silentDaysThreshold={silentDaysThreshold} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
