// Typed thin client over the OmniProj desktop backend (Tauri IPC). Pull-only:
// nothing here polls or pushes — the UI refetches when the user asks.
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

export const api = {
  projects: () => invoke<ProjectCard[]>("get_projects"),
};
