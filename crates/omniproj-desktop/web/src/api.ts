// Typed thin client over the OmniProj desktop backend (Tauri IPC). Pull-only:
// nothing here polls or pushes — the UI refetches when the user asks. Mutations are
// only ever fired on explicit user action (charter §5 原则4: notes/ is ground truth).
import { invoke } from "@tauri-apps/api/core";

export interface ProjectCard {
  name: string;
  hash: string;
  path: string;
  last_distilled: string | null;
  branch: string | null;
  /** Uncommitted lines (`git status --porcelain` count) — a fact, not a judgement. */
  dirty: number;
  /** 16-week commit histogram, oldest → newest (the sparkline). */
  commit_weeks: number[];
}

export type TaskStatus = "open" | "doing" | "done";

export interface Task {
  id: string | null;
  text: string;
  status: TaskStatus;
  unclear: boolean;
  /** Expected-completion date `YYYY-MM-DD`, or null. */
  due: string | null;
  /** Attributed commit SHAs (abbreviated) — the actual side of planned-vs-actual. */
  commits: string[];
  /** One-line problem note (问题备注), or null. */
  note: string | null;
}

export interface Commit {
  hash: string;
  short: string;
  date: string;
  author: string;
  subject: string;
}

export interface GraphCommit {
  hash: string;
  short: string;
  parents: string[];
  refs: string[];
  date: string;
  author: string;
  subject: string;
}

export type DecisionStatus = "planned" | "doing" | "done" | "abandoned";

export interface Decision {
  id: string | null;
  date: string;
  title: string;
  status: DecisionStatus;
  commit: string | null;
  body: string;
}

export interface Settings {
  reminders_enabled: boolean;
  silence_days: number;
  interval_hours: number;
}

export const api = {
  projects: () => invoke<ProjectCard[]>("get_projects"),
  tasks: (hash: string) => invoke<Task[]>("get_tasks", { hash }),
  addTask: (hash: string, text: string, unclear: boolean) =>
    invoke<string>("add_task", { hash, text, unclear }),
  setTaskStatus: (hash: string, id: string, status: TaskStatus) =>
    invoke<void>("set_task_status", { hash, id, status }),
  setTaskDue: (hash: string, id: string, date: string | null) =>
    invoke<void>("set_task_due", { hash, id, date }),
  setTaskNote: (hash: string, id: string, note: string | null) =>
    invoke<void>("set_task_note", { hash, id, note }),
  removeTask: (hash: string, id: string) =>
    invoke<void>("remove_task", { hash, id }),
  commits: (hash: string, limit: number) =>
    invoke<Commit[]>("get_commits", { hash, limit }),
  graph: (hash: string, limit: number) =>
    invoke<GraphCommit[]>("get_graph", { hash, limit }),
  attributeCommit: (hash: string, id: string, sha: string) =>
    invoke<void>("attribute_commit", { hash, id, sha }),
  unattributeCommit: (hash: string, id: string, sha: string) =>
    invoke<void>("unattribute_commit", { hash, id, sha }),
  advanceTask: (hash: string, id: string) => invoke<string[]>("advance_task", { hash, id }),
  adoptSubtasks: (hash: string, texts: string[]) =>
    invoke<string[]>("adopt_subtasks", { hash, texts }),
  plan: (hash: string) => invoke<Decision[]>("get_plan", { hash }),
  addDecision: (hash: string, title: string, body: string) =>
    invoke<string>("add_decision", { hash, title, body }),
  setDecisionStatus: (hash: string, id: string, status: DecisionStatus) =>
    invoke<void>("set_decision_status", { hash, id, status }),
  attention: () => invoke<string[]>("get_attention"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  testReminder: () => invoke<void>("test_reminder"),
};
