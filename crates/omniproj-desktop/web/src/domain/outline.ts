// Where a keystroke moves a step. Tab, Shift-Tab and ⌥↑/↓ each resolve to one
// `move_task` call — "put this step under that parent, after that sibling" — and every
// one of those targets is computed here, off the DOM, so the rules are testable.
//
// The list is always a pre-order flattening: a step's sub-steps sit immediately after it,
// and `depth` agrees with `parent_id`. Both invariants are the backend's to hold; these
// helpers read them and never re-derive one from the other.

import type { Task } from "./project";

/** Where a step should end up. `null` from a resolver means "this move isn't available". */
export interface MoveTarget {
  parent_id: string | null;
  after_id: string | null;
}

function indexOf(tasks: Task[], id: string): number {
  return tasks.findIndex((task) => task.id === id);
}

/** The step immediately before `task` at the same level, or null if it is the first. */
export function previousSibling(tasks: Task[], task: Task): Task | null {
  const start = indexOf(tasks, task.id);
  if (start < 0) return null;
  for (let index = start - 1; index >= 0; index -= 1) {
    const candidate = tasks[index];
    if (candidate.parent_id === task.parent_id) return candidate;
    // Passing something shallower means we left the sibling group entirely.
    if (candidate.depth < task.depth) return null;
  }
  return null;
}

/** The step immediately after `task` at the same level, skipping over its sub-steps. */
export function nextSibling(tasks: Task[], task: Task): Task | null {
  const start = indexOf(tasks, task.id);
  if (start < 0) return null;
  for (let index = start + 1; index < tasks.length; index += 1) {
    const candidate = tasks[index];
    if (candidate.parent_id === task.parent_id) return candidate;
    if (candidate.depth <= task.depth) return null;
  }
  return null;
}

/** The last step nested directly under `parentId`, or null when it has none yet. */
export function lastChild(tasks: Task[], parentId: string): Task | null {
  const children = tasks.filter((task) => task.parent_id === parentId);
  return children.length > 0 ? children[children.length - 1] : null;
}

/**
 * Tab: nest under the step above it at the same level. A step with nothing above it at
 * its own level has no parent to move under, so Tab does nothing — the same rule every
 * outliner uses.
 */
export function indentTarget(tasks: Task[], task: Task): MoveTarget | null {
  const parent = previousSibling(tasks, task);
  if (!parent) return null;
  const sibling = lastChild(tasks, parent.id);
  return { parent_id: parent.id, after_id: sibling?.id ?? null };
}

/**
 * Shift-Tab: pop out one level and land directly after the step it used to belong to,
 * so it stays where the eye last saw it rather than jumping to the end of the list.
 */
export function outdentTarget(tasks: Task[], task: Task): MoveTarget | null {
  if (task.parent_id === null) return null;
  const parent = tasks.find((candidate) => candidate.id === task.parent_id);
  if (!parent) return null;
  return { parent_id: parent.parent_id, after_id: parent.id };
}

/** ⌥↑: swap with the sibling above, landing before it. */
export function moveUpTarget(tasks: Task[], task: Task): MoveTarget | null {
  const above = previousSibling(tasks, task);
  if (!above) return null;
  const twoAbove = previousSibling(tasks, above);
  return { parent_id: task.parent_id, after_id: twoAbove?.id ?? null };
}

/** ⌥↓: swap with the sibling below, landing after it and after its sub-steps. */
export function moveDownTarget(tasks: Task[], task: Task): MoveTarget | null {
  const below = nextSibling(tasks, task);
  if (!below) return null;
  return { parent_id: task.parent_id, after_id: below.id };
}

/**
 * Top-level steps are numbered 1, 2, 3 — the numbering the notebook page has. Sub-steps
 * get no number: they are a decomposition of the step above them, and a second numbering
 * scheme underneath the first only adds noise to read past.
 */
export function topLevelNumbers(tasks: Task[]): Map<string, number> {
  const numbers = new Map<string, number>();
  let next = 1;
  for (const task of tasks) {
    if (task.parent_id === null) {
      numbers.set(task.id, next);
      next += 1;
    }
  }
  return numbers;
}

/** Every step nested under `task`, at any depth, in document order. */
export function subtree(tasks: Task[], task: Task): Task[] {
  const start = indexOf(tasks, task.id);
  if (start < 0) return [];
  const nested: Task[] = [];
  for (let index = start + 1; index < tasks.length; index += 1) {
    if (tasks[index].depth <= task.depth) break;
    nested.push(tasks[index]);
  }
  return nested;
}

/**
 * Progress of one step: how many of its sub-steps are done. A step with no sub-steps
 * returns null — "0/0" reads as "nothing done" when the truth is "nothing to break down".
 */
export function subtreeProgress(tasks: Task[], task: Task): { done: number; total: number } | null {
  const nested = subtree(tasks, task);
  if (nested.length === 0) return null;
  return {
    done: nested.filter((child) => child.status === "done").length,
    total: nested.length,
  };
}
