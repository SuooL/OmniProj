import type { Page } from "@playwright/test";

// The standard 12-project fixture. Kept compact: the browser mock expands each seed into a
// full index item, an Overview and a step list on demand.
export interface Seed {
  id: string;
  name: string;
  status: string;
  /** The project's first step, or null for a project still in setup. */
  step: string | null;
  sourceStatus?: string;
}

export const SEED: Seed[] = [
  { id: "p01", name: "payments-api", status: "active", step: "Reconcile ledger", sourceStatus: "missing" },
  { id: "p02", name: "onboarding-flow", status: "setup", step: null },
  { id: "p03", name: "search-index", status: "active", step: null },
  { id: "p04", name: "billing-worker", status: "active", step: "Idempotent retries" },
  { id: "p05", name: "infra-terraform", status: "active", step: "Split state files" },
  { id: "p06", name: "analytics-dbt", status: "waiting", step: "Await data contract" },
  { id: "p07", name: "design-tokens", status: "parked", step: "Dark mode audit" },
  { id: "p08", name: "cli-tooling", status: "active", step: "Ship v2 flags" },
  { id: "p09", name: "docs-site", status: "active", step: "First draft" },
  { id: "p10", name: "email-service", status: "active", step: "Bounce handling" },
  { id: "p11", name: "mobile-app", status: "active", step: "Offline cache" },
  { id: "p12", name: "ml-pipeline", status: "active", step: "Feature store" },
];

/**
 * Install a deterministic in-browser mock of the Tauri transport, backed by the fixture. The app
 * runs unchanged against `window.__TAURI_INTERNALS__.invoke`. Test hooks on `window.__mock`:
 *   - `pick`      : the next directory the picker returns (string | null)
 *   - `failNext`  : reject the next mutation with this error code (+ optional stateApplied)
 *   - `refreshFail`: project ids whose refresh reports source_failed
 */
export async function installMockTauri(page: Page): Promise<void> {
  await page.addInitScript((seed: Seed[]) => {
    // Interaction assertions run in the optional English locale; localization-specific tests
    // switch this setting and verify that the application persists it across reloads.
    if (window.localStorage.getItem("omniproj.locale") === null) {
      window.localStorage.setItem("omniproj.locale", "en");
    }
    const REVIEW_POLICY = { commitment_review_days: 7, rule_version: "r1-v1" };

    const w = window as unknown as {
      __mock: { pick: string | null; failNext: string | null; failStateApplied: boolean; refreshFail: string[] };
      __TAURI_INTERNALS__: unknown;
    };
    w.__mock = { pick: "/valid/repo", failNext: null, failStateApplied: false, refreshFail: [] };

    interface MockTask {
      id: string; text: string; status: string; parent_id: string | null; depth: number;
      unclear: boolean; due: string | null; note: string | null; tags: string[];
      commits: string[]; adopted_from_proposal_id: string | null;
      was_committed: boolean; is_current_commitment: boolean; updated_at: string;
    }

    let taskCounter = 0;
    function newTask(text: string, parentId: string | null, depth: number): MockTask {
      taskCounter += 1;
      return {
        id: `task-${taskCounter}`, text, status: "open", parent_id: parentId, depth,
        unclear: false, due: null, note: null, tags: [], commits: [],
        adopted_from_proposal_id: null, was_committed: false, is_current_commitment: false,
        updated_at: "2026-08-12T09:00:00Z",
      };
    }

    function observed(seedId: string) {
      return {
        observed_at: "2026-08-12T09:00:00Z",
        head: { kind: "attached", branch: "main" },
        last_commit: { sha: seedId.repeat(8).slice(0, 40).padEnd(40, "0"), short_sha: seedId + "abcd", subject: "latest work", committed_at: "2026-08-11T00:00:00Z" },
        changed_files: 0, staged_files: 0, unstaged_files: 0, untracked_files: 0, status_digest: "abc", commits_since_commitment: 2,
        commit_activity_weeks: [0, 0, 1, 0, 2, 0, 0, 1, 0, 0, 3, 0, 1, 0, 0, 2], silent_days: 1,
      };
    }
    function source(s: Seed) {
      const status = s.sourceStatus ?? "available";
      return { source_id: `${s.id}-src`, kind: "git_repo", location: `/Users/dev/${s.name}`, is_primary: true, status, last_observed_at: "2026-08-12T09:00:00Z", last_successful_refresh_at: "2026-08-12T09:00:00Z", last_error_category: null, revision: 1 };
    }
    function indexItem(s: Seed) {
      return { project_id: s.id, name: s.name, status: s.status, current_commitment: null, observed_actual: observed(s.id), review_reasons: [], source_status: s.sourceStatus ?? "available", open_steps: 0, total_steps: 0, revision: 1, source_revision: 1 };
    }
    function overviewOf(s: Seed) {
      return {
        project_id: s.id, name: s.name, created_at: "2026-08-10T12:00:00Z", status: s.status, status_reason: null, phase: null,
        objective: s.status === "setup" ? null : "Ship it", desired_outcome: null, review_at: null,
        source: source(s), current_commitment: null, observed_actual: observed(s.id),
        review_reasons: [], recent_transitions: [], last_transition: null,
        undoable_transition_id: null, review_policy: REVIEW_POLICY, revision: 1,
      };
    }

    const index = seed.map(indexItem);
    const overviews: Record<string, ReturnType<typeof overviewOf>> = {};
    for (const s of seed) overviews[s.id] = overviewOf(s);
    const tasksByProject: Record<string, MockTask[]> = {};
    const taskRevision: Record<string, number> = {};
    for (const s of seed) {
      tasksByProject[s.id] = s.step ? [newTask(s.step, null, 0)] : [];
      taskRevision[s.id] = 1;
    }

    /** Keep the Index row's step counts in step with the list, like the real backend. */
    function syncCounts(projectId: string) {
      const list = tasksByProject[projectId] ?? [];
      const row = index.find((r) => r.project_id === projectId);
      if (row) {
        row.total_steps = list.length;
        row.open_steps = list.filter((t) => t.status !== "done").length;
      }
    }
    for (const s of seed) syncCounts(s.id);

    function listResult(projectId: string) {
      taskRevision[projectId] = (taskRevision[projectId] ?? 1) + 1;
      syncCounts(projectId);
      return Promise.resolve({ revision: String(taskRevision[projectId]), tasks: tasksByProject[projectId] });
    }

    /** `id` plus everything nested under it, contiguous in document order. */
    function subtree(list: MockTask[], id: string): MockTask[] {
      const start = list.findIndex((t) => t.id === id);
      if (start < 0) return [];
      const out = [list[start]];
      for (let i = start + 1; i < list.length && list[i].depth > list[start].depth; i += 1) out.push(list[i]);
      return out;
    }

    /** Place `moving` (a whole subtree) under `parentId`, after `afterId`. */
    function place(list: MockTask[], moving: MockTask[], parentId: string | null, afterId: string | null) {
      const rest = list.filter((t) => !moving.includes(t));
      const parentDepth = parentId === null ? -1 : (rest.find((t) => t.id === parentId)?.depth ?? -1);
      const shift = parentDepth + 1 - moving[0].depth;
      for (const t of moving) t.depth += shift;
      moving[0].parent_id = parentId;

      let at: number;
      if (afterId !== null) {
        const tail = subtree(rest, afterId);
        at = rest.indexOf(tail[tail.length - 1]) + 1;
      } else if (parentId !== null) {
        at = rest.findIndex((t) => t.id === parentId) + 1;
      } else {
        at = 0;
      }
      rest.splice(at, 0, ...moving);
      return rest;
    }

    let agentSettings = {
      default_model: "anthropic/claude-sonnet-4-6", selected_provider: "anthropic", selected_model: "claude-sonnet-4-6",
      remote_consent: false, ready: false,
      providers: [
        { name: "anthropic", kind: "anthropic", local: false, key_required: true, key_present: false },
        { name: "deepseek", kind: "openai", local: false, key_required: true, key_present: true },
        { name: "ollama", kind: "openai", local: true, key_required: false, key_present: true },
      ],
    };

    function fail(code: string) {
      const stateApplied = w.__mock.failStateApplied;
      return Promise.reject({ code, message: `mock ${code}`, retryable: code === "store_write_failed", state_applied: stateApplied, ...(stateApplied ? { durable_revision: 999 } : {}) });
    }
    function checkFail() {
      const code = w.__mock.failNext;
      if (code) { w.__mock.failNext = null; return code; }
      return null;
    }
    function bump(id: string, mutate: (ov: any) => void) {
      const ov = overviews[id];
      ov.revision += 1;
      mutate(ov);
      const row = index.find((r) => r.project_id === id);
      if (row) {
        row.revision = ov.revision;
        row.status = ov.status;
      }
      return Promise.resolve(ov);
    }

    function invoke(cmd: string, args?: { input?: any; options?: any }): Promise<unknown> {
      const input = args?.input ?? {};
      switch (cmd) {
        case "list_project_index":
          return Promise.resolve({ projects: index, review_policy: REVIEW_POLICY });
        case "get_project_overview":
          return Promise.resolve(overviews[input.project_id]);
        case "get_tasks":
          return Promise.resolve({ revision: String(taskRevision[input.project_id] ?? 1), tasks: tasksByProject[input.project_id] ?? [] });
        case "add_task": {
          const code = checkFail(); if (code) return fail(code);
          const list = tasksByProject[input.project_id] ?? (tasksByProject[input.project_id] = []);
          const parentId = input.parent_id ?? null;
          const parentDepth = parentId === null ? -1 : (list.find((t) => t.id === parentId)?.depth ?? -1);
          const created = newTask(input.text, parentId, parentDepth + 1);
          if (parentId === null && !input.after_id) list.push(created);
          else tasksByProject[input.project_id] = place([...list, created], [created], parentId, input.after_id ?? null);
          return listResult(input.project_id);
        }
        case "move_task": {
          const code = checkFail(); if (code) return fail(code);
          const list = tasksByProject[input.project_id] ?? [];
          const moving = subtree(list, input.id);
          if (moving.length === 0) return Promise.reject({ code: "invalid_input", message: "step not found", retryable: false, state_applied: false });
          tasksByProject[input.project_id] = place(list, moving, input.parent_id ?? null, input.after_id ?? null);
          return listResult(input.project_id);
        }
        case "update_task": {
          const code = checkFail(); if (code) return fail(code);
          const list = tasksByProject[input.project_id] ?? [];
          const task = list.find((item) => item.id === input.id);
          if (!task) return Promise.reject({ code: "invalid_input", message: "task not found", retryable: false, state_applied: false });
          // Mirrors core: tags are normalized (trim, drop empties, case-insensitive dedupe);
          // an omitted `tags` leaves the stored value untouched.
          if (input.tags !== undefined && input.tags !== null) {
            const seen = new Set<string>();
            task.tags = (input.tags as string[])
              .map((tag) => tag.trim())
              .filter((tag) => tag.length > 0 && !seen.has(tag.toLowerCase()) && seen.add(tag.toLowerCase()) !== undefined);
          }
          if (input.text !== undefined && input.text !== null) task.text = input.text;
          task.status = input.status;
          task.due = input.due ?? null;
          task.note = input.note ?? null;
          task.updated_at = "2026-08-12T10:00:00Z";
          return listResult(input.project_id);
        }
        case "remove_task": {
          const code = checkFail(); if (code) return fail(code);
          const list = tasksByProject[input.project_id] ?? [];
          const doomed = new Set(subtree(list, input.id).map((t) => t.id));
          tasksByProject[input.project_id] = list.filter((item) => !doomed.has(item.id));
          return listResult(input.project_id);
        }
        case "advance_task":
          return Promise.resolve({ proposal_id: `${input.id}-proposal`, candidates: ["Inspect the failing path", "Write a regression test", "Implement the smallest fix"] });
        case "adopt_subtasks": {
          const list = tasksByProject[input.project_id] ?? (tasksByProject[input.project_id] = []);
          const parentId = input.parent_id ?? null;
          const parentDepth = parentId === null ? -1 : (list.find((t) => t.id === parentId)?.depth ?? -1);
          let next = list;
          for (const text of input.texts) {
            const created = newTask(text, parentId, parentDepth + 1);
            const lastChild = [...next].reverse().find((t) => t.parent_id === parentId);
            next = place([...next, created], [created], parentId, lastChild?.id ?? null);
          }
          tasksByProject[input.project_id] = next;
          return listResult(input.project_id);
        }
        case "get_commit_timeline":
        case "get_git_graph":
          return Promise.resolve([]);
        case "get_commit_heatmap":
          return Promise.resolve({
            days: Array.from({ length: 371 }, (_, i) => (i % 9 === 0 ? (i % 27 === 0 ? 4 : 1) : 0)),
            last_day: "2026-08-12",
          });
        case "get_reminder_settings":
          return Promise.resolve({ enabled: true, cadence: "daily", silent_days_threshold: 7, revision: "settings-1" });
        case "get_agent_settings":
          return Promise.resolve(agentSettings);
        case "set_agent_settings": {
          const [provider, model] = input.default_model.split(/\/(.+)/);
          agentSettings = { ...agentSettings, default_model: input.default_model, selected_provider: provider, selected_model: model, remote_consent: input.remote_consent, ready: provider === "ollama" || Boolean(input.remote_consent) };
          return Promise.resolve(agentSettings);
        }
        case "test_agent_provider":
          return agentSettings.ready ? Promise.resolve(null) : Promise.reject({ code: "invalid_input", message: "not ready", retryable: false, state_applied: false });
        case "refresh_attention_indicator":
          return Promise.resolve({ count: 0, project_ids: [] });
        case "get_focus_agenda":
          return Promise.resolve({ total_items: 0, projects: [] });
        case "validate_project_source": {
          const loc: string = input.location ?? "";
          if (loc.includes("dup")) return Promise.resolve({ state: "duplicate", location: loc, existing_project_id: "p04", existing_name: "billing-worker" });
          if (loc.includes("plain")) return Promise.resolve({ state: "not_git_repository", location: loc });
          if (loc.includes("bare")) return Promise.resolve({ state: "bare_repository", location: loc });
          return Promise.resolve({ state: "ok", location: loc, head: { kind: "attached", branch: "main" }, last_commit: null });
        }
        case "register_project": {
          const code = checkFail(); if (code) return fail(code);
          const id = "new-proj";
          const s: Seed = { id, name: input.name || "new-proj", status: "setup", step: null };
          index.unshift(indexItem(s));
          overviews[id] = overviewOf(s);
          tasksByProject[id] = [];
          taskRevision[id] = 1;
          return Promise.resolve(overviews[id]);
        }
        case "relink_project_source": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.source.status = "available"; ov.source.location = input.new_location; ov.source.revision += 1; });
        }
        case "refresh_projects": {
          const targets = input.project_ids ?? index.map((row) => row.project_id);
          return Promise.resolve(index
            .filter((row) => targets.includes(row.project_id))
            .map((row) => ({ project_id: row.project_id, outcome: w.__mock.refreshFail.includes(row.project_id) ? "source_failed" : "refreshed", item: row, error_category: w.__mock.refreshFail.includes(row.project_id) ? "source_missing" : undefined })));
        }
        case "complete_project_setup": {
          const code = checkFail(); if (code) return fail(code);
          // Setup writes the first step and nothing else.
          tasksByProject[input.project_id] = [newTask(input.first_commitment, null, 0)];
          taskRevision[input.project_id] = 1;
          syncCounts(input.project_id);
          return bump(input.project_id, (ov) => { ov.status = "active"; ov.objective = input.objective || null; });
        }
        case "save_project_framing": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.objective = input.objective || null; });
        }
        case "set_project_status": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.status = input.status; ov.status_reason = input.reason ?? null; ov.review_at = input.review_at ?? null; });
        }
        case "plugin:dialog|open":
          return Promise.resolve(w.__mock.pick);
        default:
          return Promise.reject({ code: "invalid_input", message: `unhandled ${cmd}`, retryable: false, state_applied: false });
      }
    }

    w.__TAURI_INTERNALS__ = {
      invoke,
      transformCallback: (cb: unknown) => cb,
      convertFileSrc: (p: string) => p,
    };
  }, SEED);
}
