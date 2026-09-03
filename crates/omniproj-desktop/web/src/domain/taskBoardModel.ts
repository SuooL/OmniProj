// Pure derivations for the planning board view (R1c, FR-R6). No I/O, no clocks:
// `today` is the user's LOCAL calendar date (YYYY-MM-DD), passed in by the caller,
// mirroring the core rule that due dates carry day semantics (due == today is not overdue).

import type { Task } from "./project";

export type TaskViewMode = "list" | "time";
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

/** Overdue = the due day has fully passed; "soon" = within the next 7 days (visual only).
 * A finished task is never overdue or due-soon — it only carries its date, matching core,
 * where only Planned/Doing/Blocked items produce the OverdueWork review reason. */
export function dueSignal(due: string | null, today: string, status?: Task["status"]): DueSignal {
  if (!due) return { kind: "none" };
  if (status === "done") return { kind: "scheduled" };
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
    return dueSignal(task.due, today, task.status).kind === "overdue" ? 0 : 1;
  };
  return [...tasks].sort((a, b) => {
    const byRank = rank(a) - rank(b);
    if (byRank !== 0) return byRank;
    if (a.due && b.due && a.due !== b.due) return a.due < b.due ? -1 : 1;
    if (!a.due && !b.due && a.updated_at !== b.updated_at) return a.updated_at > b.updated_at ? -1 : 1;
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}

// --- Time-grouped view (R1d) ----------------------------------------------

export type TimeGroupKey = "overdue" | "today" | "thisWeek" | "nextWeek" | "later" | "unscheduled";

/** Fixed rendering order of the time groups. */
export const TIME_GROUP_ORDER: readonly TimeGroupKey[] = [
  "overdue",
  "today",
  "thisWeek",
  "nextWeek",
  "later",
  "unscheduled",
];

function addDays(date: string, days: number): string {
  const shifted = new Date(Date.parse(`${date}T00:00:00Z`) + days * 86_400_000);
  return shifted.toISOString().slice(0, 10);
}

/** The Monday of `today`'s ISO week (weeks run Monday–Sunday). */
export function isoWeekStart(today: string): string {
  const day = new Date(`${today}T00:00:00Z`).getUTCDay(); // 0 = Sunday
  return addDays(today, -((day + 6) % 7));
}

/** Group by due against the local calendar: overdue / today / rest of this ISO week /
 * next ISO week / later / unscheduled. Done tasks are excluded — this view answers
 * "what comes due next", not a retrospective. Empty groups are omitted. */
export function timeGroups(tasks: Task[], today: string): Array<{ key: TimeGroupKey; tasks: Task[] }> {
  const weekEnd = addDays(isoWeekStart(today), 6);
  const nextWeekEnd = addDays(weekEnd, 7);
  const keyed = new Map<TimeGroupKey, Task[]>(TIME_GROUP_ORDER.map((key) => [key, []]));
  for (const task of tasks) {
    if (task.status === "done") continue;
    const key: TimeGroupKey = !task.due
      ? "unscheduled"
      : task.due < today
        ? "overdue"
        : task.due === today
          ? "today"
          : task.due <= weekEnd
            ? "thisWeek"
            : task.due <= nextWeekEnd
              ? "nextWeek"
              : "later";
    keyed.get(key)!.push(task);
  }
  return TIME_GROUP_ORDER.map((key) => ({ key, tasks: columnOrder(keyed.get(key)!, today) })).filter(
    (group) => group.tasks.length > 0,
  );
}

/** Split a user-entered tag string on comma variants; trimming and dedupe happen in core. */
export function parseTagsInput(value: string): string[] {
  return value.split(/[,，、]/).map((tag) => tag.trim()).filter((tag) => tag.length > 0);
}

/** The editable shape of one task row while its panel is open. */
export interface TaskDraft {
  status: string;
  due: string;
  note: string;
  tags: string;
}
