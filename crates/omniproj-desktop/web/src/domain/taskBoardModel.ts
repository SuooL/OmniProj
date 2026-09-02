// Pure derivations for the planning board view (R1c, FR-R6). No I/O, no clocks:
// `today` is the user's LOCAL calendar date (YYYY-MM-DD), passed in by the caller,
// mirroring the core rule that due dates carry day semantics (due == today is not overdue).

import type { Task } from "./project";

export type TaskViewMode = "list" | "board";
export const TASK_VIEW_STORAGE_KEY = "omniproj.task-view";

/** The user's local calendar date as YYYY-MM-DD (en-CA formats ISO-style). */
export function localToday(): string {
  return new Date().toLocaleDateString("en-CA");
}

/** Whole days from `from` to `to` (both YYYY-MM-DD); positive when `to` is later. */
export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000);
}

export type DueSignal =
  | { kind: "overdue"; days: number }
  | { kind: "soon"; days: number }
  | { kind: "scheduled" }
  | { kind: "none" };

/** Overdue = the due day has fully passed; "soon" = within the next 7 days (visual only). */
export function dueSignal(due: string | null, today: string): DueSignal {
  if (!due) return { kind: "none" };
  const days = daysBetween(due, today);
  if (days > 0) return { kind: "overdue", days };
  if (days >= -7) return { kind: "soon", days: Math.max(0, -days) };
  return { kind: "scheduled" };
}

/** Deterministic in-column order: overdue first (oldest due first), then dated ascending,
 * then undated by most recent mutation. Ties break on id for stability. */
export function columnOrder(tasks: Task[], today: string): Task[] {
  const rank = (task: Task): number => {
    if (!task.due) return 2;
    return dueSignal(task.due, today).kind === "overdue" ? 0 : 1;
  };
  return [...tasks].sort((a, b) => {
    const byRank = rank(a) - rank(b);
    if (byRank !== 0) return byRank;
    if (a.due && b.due && a.due !== b.due) return a.due < b.due ? -1 : 1;
    if (!a.due && !b.due && a.updated_at !== b.updated_at) return a.updated_at > b.updated_at ? -1 : 1;
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}

export interface BoardColumns {
  open: Task[];
  doing: Task[];
  done: Task[];
}

/** Split into status columns with deterministic ordering; done is most-recent-first. */
export function boardColumns(tasks: Task[], today: string): BoardColumns {
  const by = (status: Task["status"]) => tasks.filter((task) => task.status === status);
  return {
    open: columnOrder(by("open"), today),
    doing: columnOrder(by("doing"), today),
    done: [...by("done")].sort((a, b) =>
      a.updated_at === b.updated_at ? (a.id < b.id ? -1 : 1) : a.updated_at > b.updated_at ? -1 : 1,
    ),
  };
}

/** How many done cards show before the column folds into a count. */
export const DONE_PREVIEW_COUNT = 5;
