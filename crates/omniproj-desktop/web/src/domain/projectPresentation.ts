// Pure, immutable presentation helpers for the dense Index. Everything here is a
// deterministic function of already-loaded state. Crucially, NONE of these functions take
// commit counts (or any observed activity) as a health, priority, or ordering input — the
// review order and reasons are computed and sorted in core; the browser only reads them.

import type { ProjectIndexItem, ReviewReason } from "./project";

/** The four Index filters. Deterministic, local, and reversible. */
export type ReviewFilter = "all" | "needs_review" | "waiting" | "parked" | "archived";

/** A transparent label for the default order — it is a review order, not a ranking. */
export const REVIEW_ORDER_LABEL = "Review order (deterministic, not priority or health)";

/** Case-insensitive substring match on the project name. */
export function matchesQuery(item: ProjectIndexItem, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle === "") return true;
  return item.name.toLowerCase().includes(needle);
}

/** Immutable local text filter over project names. Never mutates the input array. */
export function filterByText(
  items: readonly ProjectIndexItem[],
  query: string,
): ProjectIndexItem[] {
  return items.filter((item) => matchesQuery(item, query));
}

/** Apply one of the fixed review filters. Immutable. */
export function applyReviewFilter(
  items: readonly ProjectIndexItem[],
  filter: ReviewFilter,
): ProjectIndexItem[] {
  switch (filter) {
    case "all":
      return excludeArchived(items);
    case "needs_review":
      return items.filter((item) => item.status !== "archived" && item.review_reasons.length > 0);
    case "waiting":
      return items.filter((item) => item.status === "waiting");
    case "parked":
      return items.filter((item) => item.status === "parked");
    case "archived":
      return items.filter((item) => item.status === "archived");
  }
}

/** Exclude archived projects from ordinary operating views. Immutable. */
export function excludeArchived(
  items: readonly ProjectIndexItem[],
): ProjectIndexItem[] {
  return items.filter((item) => item.status !== "archived");
}

/**
 * The primary review reason to display, in the fixed backend priority. Review reasons
 * arrive pre-sorted from core, so the first is the most urgent; this never re-ranks.
 */
export function primaryReason(item: ProjectIndexItem): ReviewReason | null {
  return item.review_reasons[0] ?? null;
}

/** The review reasons beyond the primary one (the `+N`). */
export function hiddenReasons(item: ProjectIndexItem): ReviewReason[] {
  return item.review_reasons.slice(1);
}

/**
 * The accessible name for the `+N` affordance, enumerating the hidden reasons so screen
 * readers announce them. Returns null when there is nothing hidden.
 */
export function hiddenReasonsAccessibleName(item: ProjectIndexItem): string | null {
  const hidden = hiddenReasons(item);
  if (hidden.length === 0) return null;
  const labels = hidden.map((reason) => reason.label).join(", ");
  const noun = hidden.length === 1 ? "review reason" : "review reasons";
  return `${hidden.length} more ${noun}: ${labels}`;
}

/** A relative time plus the exact timestamp for the element's `title`. */
export interface RelativeTime {
  /** Human relative text, e.g. "3 days ago". */
  text: string;
  /** The exact source timestamp, shown as a tooltip title. */
  title: string;
}

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * Format an RFC3339 instant as relative-to-`now` text with an exact `title`. Returns null
 * for an unparseable timestamp (the caller shows nothing rather than a wrong time).
 */
export function formatRelativeTime(iso: string, now: Date): RelativeTime | null {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return null;
  const seconds = Math.max(0, Math.round((now.getTime() - then) / 1000));
  return { text: relativeText(seconds), title: iso };
}

function relativeText(seconds: number): string {
  if (seconds < 45) return "just now";
  if (seconds < 90) return "1 minute ago";
  if (seconds < HOUR) return `${Math.round(seconds / MINUTE)} minutes ago`;
  if (seconds < 90 * MINUTE) return "1 hour ago";
  if (seconds < DAY) return `${Math.round(seconds / HOUR)} hours ago`;
  if (seconds < 2 * DAY) return "yesterday";
  return `${Math.round(seconds / DAY)} days ago`;
}
