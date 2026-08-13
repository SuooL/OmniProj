// The dense four-column operating index. The default order is the backend's deterministic
// review order — this component NEVER re-ranks it; alternative sorts are transparent, opt-in,
// and clearly not a priority/health score. Filter (text + review chips) and sort live in the
// canonical search params. The 7-day review interval is read from the response's review_policy,
// never a frontend constant.

import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProjectIndexItem, ReviewPolicy } from "../../domain/project";
import {
  REVIEW_ORDER_LABEL,
  applyReviewFilter,
  excludeArchived,
  filterByText,
  type ReviewFilter,
} from "../../domain/projectPresentation";
import { FilterChip } from "../semantic/FilterChip";
import { ProjectRow } from "./ProjectRow";

type SortMode = "review" | "name" | "observed";

const REVIEW_FILTERS: Array<{ value: ReviewFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "needs_review", label: "Needs review" },
  { value: "waiting", label: "Waiting" },
  { value: "parked", label: "Parked" },
];

const COLUMNS = ["Project", "Current commitment", "Observed actual", "Review"];

function parseFilter(value: string | null): ReviewFilter {
  return value === "needs_review" || value === "waiting" || value === "parked"
    ? value
    : "all";
}

function parseSort(value: string | null): SortMode {
  return value === "name" || value === "observed" ? value : "review";
}

/** Transparent, opt-in sort. `review` preserves the backend order verbatim (no re-ranking). */
function applySort(items: ProjectIndexItem[], sort: SortMode): ProjectIndexItem[] {
  switch (sort) {
    case "review":
      return items;
    case "name":
      return [...items].sort((a, b) => a.name.localeCompare(b.name));
    case "observed":
      return [...items].sort((a, b) => {
        const at = a.observed_actual?.observed_at ?? "";
        const bt = b.observed_actual?.observed_at ?? "";
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

  const nonArchived = useMemo(() => excludeArchived(projects), [projects]);
  const visible = useMemo(() => {
    const base = applyReviewFilter(nonArchived, filter);
    return applySort(filterByText(base, query), sort);
  }, [nonArchived, filter, query, sort]);

  // A store with no non-archived projects offers the primary recovery action (an all-archived
  // store is empty for R0 purposes, not a dead "no matches" screen).
  if (nonArchived.length === 0) {
    return (
      <section data-testid="projects-index-empty" aria-labelledby="projects-empty-heading">
        <h2 id="projects-empty-heading">No projects yet</h2>
        <p>Add a project to begin re-entering and advancing your work.</p>
        <button type="button" className="op-primary" onClick={onAddProject}>
          Add project
        </button>
      </section>
    );
  }

  return (
    <section className="op-index" aria-labelledby="projects-index-heading">
      <div className="op-index__toolbar">
        <div className="op-index__meta">
          <span className="op-index__order">{REVIEW_ORDER_LABEL}</span>
          <span className="op-index__interval">
            Commitment review interval: {reviewPolicy.commitment_review_days} days
          </span>
        </div>

        <div className="op-index__controls">
          <div className="op-filters" role="group" aria-label="Review filters">
            {REVIEW_FILTERS.map((chip) => (
              <FilterChip
                key={chip.value}
                label={chip.label}
                pressed={filter === chip.value}
                onClick={() => setParam("filter", chip.value, chip.value !== "all")}
              />
            ))}
          </div>
          <label className="op-sort">
            <span>Sort</span>
            <select
              aria-label="Review order"
              value={sort}
              onChange={(e) => setParam("sort", e.target.value, e.target.value !== "review")}
            >
              <option value="review">Review order</option>
              <option value="name">Name</option>
              <option value="observed">Recently observed</option>
            </select>
          </label>
        </div>
      </div>

      <div className="op-index__table">
        <div className="op-index__head" aria-hidden="true">
          {COLUMNS.map((c) => (
            <span key={c} className="op-index__col">
              {c}
            </span>
          ))}
        </div>

        {visible.length === 0 ? (
          <p className="op-index__nomatch" data-testid="projects-index-nomatch">
            No projects match this filter.
          </p>
        ) : (
          <ul className="op-index__list" aria-label="Projects">
            {visible.map((item) => (
              <ProjectRow key={item.project_id} item={item} now={now} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
