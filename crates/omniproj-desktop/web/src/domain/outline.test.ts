import { describe, expect, it } from "vitest";

import {
  indentTarget,
  lastChild,
  moveDownTarget,
  moveUpTarget,
  nextSibling,
  outdentTarget,
  previousSibling,
  subtreeProgress,
  topLevelNumbers,
} from "./outline";
import type { Task } from "./project";

/**
 * Build a list from an indented sketch, so a test reads like the outline it is about:
 *
 *   one
 *     one-a
 *   two
 */
function outline(sketch: string): Task[] {
  return sketch
    .split("\n")
    .map((line) => line.replace(/\s+$/, ""))
    .filter((line) => line.trim() !== "")
    .reduce<{ tasks: Task[]; parents: Array<string | null> }>(
      (state, line) => {
        const depth = (line.length - line.trimStart().length) / 2;
        const id = line.trim();
        state.parents[depth] = id;
        state.tasks.push({
          id,
          text: id,
          status: "open",
          parent_id: depth === 0 ? null : state.parents[depth - 1],
          depth,
          unclear: false,
          due: null,
          note: null,
          tags: [],
          commits: [],
          adopted_from_proposal_id: null,
          was_committed: false,
          is_current_commitment: false,
          updated_at: "2026-08-01T00:00:00Z",
        });
        return state;
      },
      { tasks: [], parents: [] },
    ).tasks;
}

const at = (tasks: Task[], id: string) => tasks.find((task) => task.id === id)!;

const SAMPLE = outline(`
one
  one-a
  one-b
    one-b-i
two
three
`);

describe("sibling navigation", () => {
  it("skips over a sibling's sub-steps rather than stepping into them", () => {
    expect(nextSibling(SAMPLE, at(SAMPLE, "one"))?.id).toBe("two");
    expect(previousSibling(SAMPLE, at(SAMPLE, "three"))?.id).toBe("two");
  });

  it("does not escape the sibling group at its edges", () => {
    expect(previousSibling(SAMPLE, at(SAMPLE, "one"))).toBeNull();
    expect(nextSibling(SAMPLE, at(SAMPLE, "three"))).toBeNull();
    // "one-a" is first among its siblings; the step above it is its own parent.
    expect(previousSibling(SAMPLE, at(SAMPLE, "one-a"))).toBeNull();
    expect(nextSibling(SAMPLE, at(SAMPLE, "one-b"))).toBeNull();
  });

  it("finds the last child to append after", () => {
    expect(lastChild(SAMPLE, "one")?.id).toBe("one-b");
    expect(lastChild(SAMPLE, "two")).toBeNull();
  });
});

describe("indent", () => {
  it("nests a step under the one above it, after that step's existing sub-steps", () => {
    expect(indentTarget(SAMPLE, at(SAMPLE, "two"))).toEqual({
      parent_id: "one",
      after_id: "one-b",
    });
  });

  it("nests under a childless step with no sibling to follow", () => {
    expect(indentTarget(SAMPLE, at(SAMPLE, "three"))).toEqual({
      parent_id: "two",
      after_id: null,
    });
  });

  it("is unavailable for the first step at its level", () => {
    expect(indentTarget(SAMPLE, at(SAMPLE, "one"))).toBeNull();
    expect(indentTarget(SAMPLE, at(SAMPLE, "one-a"))).toBeNull();
  });
});

describe("outdent", () => {
  it("pops out one level and lands right after the old parent", () => {
    expect(outdentTarget(SAMPLE, at(SAMPLE, "one-b"))).toEqual({
      parent_id: null,
      after_id: "one",
    });
    expect(outdentTarget(SAMPLE, at(SAMPLE, "one-b-i"))).toEqual({
      parent_id: "one",
      after_id: "one-b",
    });
  });

  it("is unavailable at top level", () => {
    expect(outdentTarget(SAMPLE, at(SAMPLE, "two"))).toBeNull();
  });
});

describe("reorder", () => {
  it("moves a step above its previous sibling", () => {
    // "three" goes before "two", i.e. after "one".
    expect(moveUpTarget(SAMPLE, at(SAMPLE, "three"))).toEqual({
      parent_id: null,
      after_id: "one",
    });
    // "two" goes to the very top.
    expect(moveUpTarget(SAMPLE, at(SAMPLE, "two"))).toEqual({
      parent_id: null,
      after_id: null,
    });
  });

  it("moves a step below its next sibling, clearing that sibling's sub-steps", () => {
    expect(moveDownTarget(SAMPLE, at(SAMPLE, "one-a"))).toEqual({
      parent_id: "one",
      after_id: "one-b",
    });
  });

  it("is unavailable at the ends of a sibling group", () => {
    expect(moveUpTarget(SAMPLE, at(SAMPLE, "one"))).toBeNull();
    expect(moveDownTarget(SAMPLE, at(SAMPLE, "three"))).toBeNull();
  });
});

describe("presentation", () => {
  it("numbers only the top level, in document order", () => {
    const numbers = topLevelNumbers(SAMPLE);
    expect(numbers.get("one")).toBe(1);
    expect(numbers.get("two")).toBe(2);
    expect(numbers.get("three")).toBe(3);
    expect(numbers.has("one-a")).toBe(false);
  });

  it("reports sub-step progress, and nothing at all for a step with none", () => {
    const tasks = SAMPLE.map((task) =>
      task.id === "one-a" ? { ...task, status: "done" as const } : task,
    );
    expect(subtreeProgress(tasks, at(tasks, "one"))).toEqual({ done: 1, total: 3 });
    expect(subtreeProgress(tasks, at(tasks, "two"))).toBeNull();
  });
});
