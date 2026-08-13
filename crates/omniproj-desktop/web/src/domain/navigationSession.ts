// The persistent navigation session. It remembers *where* the Human was and *how* the Index
// was scrolled/focused — never *what* the projects contain. Two things live here:
//
//   1. the last canonical `pathname + search` (so a restart at "/" lands where they left off);
//   2. the noncanonical Index view state (scroll offset + the id of the row to refocus).
//
// Filters and sort are canonical, so they live in the URL search params, not here. Nothing
// in this module stores project data. Every access is guarded so a disabled/again-full
// sessionStorage degrades to "no saved state" rather than throwing.

const CANONICAL_KEY = "omniproj.nav.canonical";
const INDEX_VIEW_KEY = "omniproj.nav.indexView";

/** The saved Index scroll offset and the row to restore focus to on return. */
export interface IndexViewState {
  scrollY: number;
  focusId: string | null;
}

function storage(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.sessionStorage;
  } catch {
    return null;
  }
}

function read(key: string): string | null {
  try {
    return storage()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    storage()?.setItem(key, value);
  } catch {
    // A full or unavailable store just means we cannot remember this; never throw.
  }
}

/** Save the last canonical location as `pathname + search`. "/" is never saved as a target. */
export function saveCanonicalLocation(pathnameAndSearch: string): void {
  if (pathnameAndSearch === "/" || pathnameAndSearch === "") return;
  write(CANONICAL_KEY, pathnameAndSearch);
}

/** The last canonical `pathname + search`, or null if none was saved. */
export function loadCanonicalLocation(): string | null {
  const value = read(CANONICAL_KEY);
  return value && value !== "/" ? value : null;
}

/** Persist the Index scroll offset and the row id to refocus. */
export function saveIndexViewState(state: IndexViewState): void {
  write(INDEX_VIEW_KEY, JSON.stringify(state));
}

/** The saved Index view state, or null when absent or unparseable. */
export function loadIndexViewState(): IndexViewState | null {
  const raw = read(INDEX_VIEW_KEY);
  if (raw === null) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<IndexViewState>;
    if (typeof parsed.scrollY !== "number") return null;
    const focusId = typeof parsed.focusId === "string" ? parsed.focusId : null;
    return { scrollY: parsed.scrollY, focusId };
  } catch {
    return null;
  }
}

/**
 * Restore the saved canonical route *before* the router mounts, but only when the incoming
 * path is the root. An explicit non-root deep link always wins over saved session state, so
 * we never overwrite it. Idempotent: after the replace, the path is no longer "/".
 */
export function restoreCanonicalRouteOnRoot(): void {
  if (typeof window === "undefined") return;
  if (window.location.pathname !== "/") return;
  const saved = loadCanonicalLocation();
  if (!saved) return;
  window.history.replaceState(window.history.state, "", saved);
}
