import { describe, expect, it } from "vitest";

import type { Task } from "./project";
import { boardColumns, columnOrder, daysBetween, dueSignal, isoWeekStart, timeGroups, DONE_PREVIEW_COUNT } from "./taskBoardModel";

const TODAY = "2026-09-02";

function task(id: string, over: Partial<Task> = {}): Task {
  return {
    id,
    text: `task ${id}`,
    status: "open",
    unclear: false,
    due: null,
    note: null,
    tags: [],
    commits: [],
    adopted_from_proposal_id: null,
    linked_work_item_id: null,
    is_current_commitment: false,
    updated_at: "2026-08-01T00:00:00Z",
    ...over,
  };
}

describe("dueSignal", () => {
  it("treats the due day itself as not overdue, mirroring the core rule", () => {
    expect(dueSignal("2026-09-02", TODAY)).toEqual({ kind: "soon", days: 0 });
    expect(dueSignal("2026-09-01", TODAY)).toEqual({ kind: "overdue", days: 1 });
    expect(dueSignal("2026-09-09", TODAY)).toEqual({ kind: "soon", days: 7 });
    expect(dueSignal("2026-09-10", TODAY)).toEqual({ kind: "scheduled" });
    expect(dueSignal(null, TODAY)).toEqual({ kind: "none" });
  });

  it("daysBetween is calendar-exact across a month boundary", () => {
    expect(daysBetween("2026-08-30", "2026-09-02")).toBe(3);
  });
});

describe("columnOrder", () => {
  it("puts oldest overdue first, then dated ascending, then undated by recency", () => {
    const ordered = columnOrder(
      [
        task("undated-old", { updated_at: "2026-08-01T00:00:00Z" }),
        task("due-far", { due: "2026-09-20" }),
        task("overdue-new", { due: "2026-09-01" }),
        task("undated-new", { updated_at: "2026-08-20T00:00:00Z" }),
        task("overdue-old", { due: "2026-08-15" }),
        task("due-near", { due: "2026-09-05" }),
      ],
      TODAY,
    ).map((item) => item.id);
    expect(ordered).toEqual([
      "overdue-old",
      "overdue-new",
      "due-near",
      "due-far",
      "undated-new",
      "undated-old",
    ]);
  });
});

describe("boardColumns", () => {
  it("splits by status and keeps done most-recent-first", () => {
    const columns = boardColumns(
      [
        task("a", { status: "done", updated_at: "2026-08-10T00:00:00Z" }),
        task("b", { status: "doing" }),
        task("c", { status: "done", updated_at: "2026-08-12T00:00:00Z" }),
        task("d", { status: "open" }),
      ],
      TODAY,
    );
    expect(columns.open.map((item) => item.id)).toEqual(["d"]);
    expect(columns.doing.map((item) => item.id)).toEqual(["b"]);
    expect(columns.done.map((item) => item.id)).toEqual(["c", "a"]);
    expect(DONE_PREVIEW_COUNT).toBe(5);
  });
});

describe("timeGroups", () => {
  // 2026-09-02 is a Wednesday: ISO week runs Mon 2026-08-31 .. Sun 2026-09-06.
  it("computes the ISO week start (Monday) including the Sunday edge", () => {
    expect(isoWeekStart("2026-09-02")).toBe("2026-08-31");
    expect(isoWeekStart("2026-08-31")).toBe("2026-08-31"); // Monday maps to itself
    expect(isoWeekStart("2026-09-06")).toBe("2026-08-31"); // Sunday belongs to the same week
  });

  it("groups by due against the local calendar and omits empty groups and done tasks", () => {
    const groups = timeGroups(
      [
        task("overdue", { due: "2026-09-01" }),
        task("today", { due: "2026-09-02" }),
        task("week", { due: "2026-09-06" }),
        task("next", { due: "2026-09-07" }),
        task("later", { due: "2026-09-14" }),
        task("open-undated"),
        task("finished", { status: "done", due: "2026-09-01" }),
      ],
      TODAY,
    );
    expect(groups.map((group) => group.key)).toEqual([
      "overdue",
      "today",
      "thisWeek",
      "nextWeek",
      "later",
      "unscheduled",
    ]);
    expect(groups.find((group) => group.key === "later")!.tasks.map((item) => item.id)).toEqual(["later"]);
    expect(groups.flatMap((group) => group.tasks.map((item) => item.id))).not.toContain("finished");
  });

  it("next week ends seven days after this ISO week", () => {
    const groups = timeGroups(
      [task("boundary", { due: "2026-09-13" }), task("beyond", { due: "2026-09-14" })],
      TODAY,
    );
    expect(groups.find((group) => group.key === "nextWeek")!.tasks.map((item) => item.id)).toEqual(["boundary"]);
    expect(groups.find((group) => group.key === "later")!.tasks.map((item) => item.id)).toEqual(["beyond"]);
  });
});

describe("dueSignal for finished work", () => {
  it("never marks a done task overdue or due-soon, matching core's OverdueWork rule", () => {
    expect(dueSignal("2026-08-01", TODAY, "done")).toEqual({ kind: "scheduled" });
    expect(dueSignal("2026-09-02", TODAY, "done")).toEqual({ kind: "scheduled" });
    // Unfinished work is unaffected.
    expect(dueSignal("2026-08-01", TODAY, "open")).toEqual({ kind: "overdue", days: 32 });
    expect(dueSignal("2026-08-01", TODAY, "doing")).toEqual({ kind: "overdue", days: 32 });
  });
});
