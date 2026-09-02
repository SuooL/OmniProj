import type { Page } from "@playwright/test";

// The standard 12-project fixture, in the backend's deterministic review order. Kept compact:
// the browser mock expands each seed into a full index item + Overview on demand.
export interface Seed {
  id: string;
  name: string;
  status: string;
  commitment: string | null;
  reasons: string[];
  sourceStatus?: string;
}

export const SEED: Seed[] = [
  { id: "p01", name: "payments-api", status: "active", commitment: "Reconcile ledger", reasons: ["source_unavailable"], sourceStatus: "missing" },
  { id: "p02", name: "onboarding-flow", status: "setup", commitment: null, reasons: ["complete_setup"] },
  { id: "p03", name: "search-index", status: "active", commitment: null, reasons: ["needs_commitment"] },
  { id: "p04", name: "billing-worker", status: "active", commitment: "Idempotent retries", reasons: ["review_action"] },
  { id: "p05", name: "infra-terraform", status: "active", commitment: "Split state files", reasons: ["scheduled_review"] },
  { id: "p06", name: "analytics-dbt", status: "waiting", commitment: "Await data contract", reasons: [] },
  { id: "p07", name: "design-tokens", status: "parked", commitment: "Dark mode audit", reasons: [] },
  { id: "p08", name: "cli-tooling", status: "active", commitment: "Ship v2 flags", reasons: [] },
  { id: "p09", name: "docs-site", status: "active", commitment: "First draft", reasons: [] },
  { id: "p10", name: "email-service", status: "active", commitment: "Bounce handling", reasons: [] },
  { id: "p11", name: "mobile-app", status: "active", commitment: "Offline cache", reasons: [] },
  { id: "p12", name: "ml-pipeline", status: "active", commitment: "Feature store", reasons: [] },
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
    // Legacy interaction assertions run in the optional English locale; localization-specific
    // tests switch this setting and verify that the application persists it across reloads.
    if (window.localStorage.getItem("omniproj.locale") === null) {
      window.localStorage.setItem("omniproj.locale", "en");
    }
    const REVIEW_POLICY = { commitment_review_days: 7, rule_version: "r1-v1" };
    const REASON_LABEL: Record<string, string> = {
      source_unavailable: "Source unavailable",
      complete_setup: "Complete setup",
      needs_commitment: "Needs commitment",
      overdue_work: "Overdue work",
      review_action: "Review action",
      scheduled_review: "Scheduled review",
    };

    const w = window as unknown as {
      __mock: { pick: string | null; failNext: string | null; failStateApplied: boolean; refreshFail: string[] };
      __TAURI_INTERNALS__: unknown;
    };
    w.__mock = { pick: "/valid/repo", failNext: null, failStateApplied: false, refreshFail: [] };

    function reasons(codes: string[]) {
      return codes.map((code) => ({ code, label: REASON_LABEL[code], evidence: code === "review_action" ? ["Commitment review interval: 7 days", "Last confirmed 2026-08-01T00:00:00Z"] : [], rule_version: "r1-v1" }));
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
    function commitmentOf(s: Seed) {
      return s.commitment === null ? null : { work_item_id: `${s.id}-w1`, text: s.commitment, status: "doing", set_at: "2026-08-10T12:00:00Z", confirmed_at: null };
    }
    function source(s: Seed) {
      const status = s.sourceStatus ?? "available";
      return { source_id: `${s.id}-src`, kind: "git_repo", location: `/Users/dev/${s.name}`, is_primary: true, status, last_observed_at: "2026-08-12T09:00:00Z", last_successful_refresh_at: "2026-08-12T09:00:00Z", last_error_category: null, revision: 1 };
    }
    function indexItem(s: Seed) {
      return { project_id: s.id, name: s.name, status: s.status, current_commitment: commitmentOf(s), observed_actual: s.sourceStatus === "missing" ? observed(s.id) : observed(s.id), review_reasons: reasons(s.reasons), source_status: s.sourceStatus ?? "available", revision: 1, source_revision: 1 };
    }
    function overviewOf(s: Seed) {
      const setTx = { id: `${s.id}-t1`, type: "set", previous_work_item_id: null, next_work_item_id: s.commitment ? `${s.id}-w1` : null, reason: null, occurred_at: "2026-08-10T12:00:00Z", corrects_transition_id: null };
      return {
        project_id: s.id, name: s.name, created_at: "2026-08-10T12:00:00Z", status: s.status, status_reason: null, phase: null,
        objective: s.status === "setup" ? null : "Ship it", desired_outcome: s.status === "setup" ? null : "Dogfood", review_at: null,
        source: source(s), current_commitment: commitmentOf(s), observed_actual: s.sourceStatus === "missing" ? observed(s.id) : observed(s.id),
        review_reasons: reasons(s.reasons), recent_transitions: s.commitment ? [setTx] : [], last_transition: s.commitment ? setTx : null,
        undoable_transition_id: s.commitment ? setTx.id : null, review_policy: REVIEW_POLICY, revision: 1,
      };
    }

    const index = seed.map(indexItem);
    const overviews: Record<string, ReturnType<typeof overviewOf>> = {};
    for (const s of seed) overviews[s.id] = overviewOf(s);
    let txCounter = 100;
    const tasksByProject: Record<string, any[]> = {};
    const taskRevision: Record<string, number> = {};
    for (const s of seed) { tasksByProject[s.id] = []; taskRevision[s.id] = 1; }
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
        row.current_commitment = ov.current_commitment;
        row.status = ov.status;
        row.review_reasons = ov.review_reasons;
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
          const list = tasksByProject[input.project_id] ?? (tasksByProject[input.project_id] = []);
          list.push({ id: `task-${list.length + 1}`, text: input.text, status: "open", unclear: input.unclear, due: null, note: null, tags: [], commits: [], adopted_from_proposal_id: null, linked_work_item_id: null, is_current_commitment: false, updated_at: "2026-08-12T09:00:00Z" });
          taskRevision[input.project_id] = (taskRevision[input.project_id] ?? 1) + 1;
          return Promise.resolve({ revision: String(taskRevision[input.project_id]), tasks: list });
        }
        case "advance_task":
          return Promise.resolve({ proposal_id: `${input.id}-proposal`, candidates: ["Inspect the failing path", "Write a regression test", "Implement the smallest fix"] });
        case "adopt_subtasks": {
          const list = tasksByProject[input.project_id] ?? (tasksByProject[input.project_id] = []);
          for (const text of input.texts) list.push({ id: `task-${list.length + 1}`, text, status: "open", unclear: false, due: null, note: null, tags: [], commits: [], adopted_from_proposal_id: input.proposal_id, linked_work_item_id: null, is_current_commitment: false, updated_at: "2026-08-12T09:00:00Z" });
          taskRevision[input.project_id] = (taskRevision[input.project_id] ?? 1) + 1;
          return Promise.resolve({ revision: String(taskRevision[input.project_id]), tasks: list });
        }
        case "get_commit_timeline":
        case "get_git_graph":
          return Promise.resolve([]);
        case "get_plan":
          return Promise.resolve({ revision: "plan-1", entries: [] });
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
        case "get_dogfood_summary":
          return Promise.resolve({ event_count: 0, project_count: 0, median_duration_seconds: null, meets_event_threshold: false, meets_project_threshold: false });
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
          const s: Seed = { id, name: input.name || "new-proj", status: "setup", commitment: null, reasons: ["complete_setup"] };
          index.unshift(indexItem(s));
          overviews[id] = overviewOf(s);
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
        case "set_commitment": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.__prev = ov.current_commitment; const t = `t${txCounter++}`; ov.current_commitment = { work_item_id: `${input.project_id}-w${txCounter}`, text: input.text, status: "doing", set_at: "2026-08-12T10:00:00Z", confirmed_at: null }; ov.undoable_transition_id = t; });
        }
        case "confirm_commitment": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { if (ov.current_commitment) ov.current_commitment.confirmed_at = "2026-08-12T10:05:00Z"; ov.undoable_transition_id = `t${txCounter++}`; });
        }
        case "complete_commitment": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.__prev = ov.current_commitment; ov.current_commitment = null; ov.undoable_transition_id = `t${txCounter++}`; });
        }
        case "replace_commitment": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.__prev = ov.current_commitment; ov.current_commitment = { work_item_id: `${input.project_id}-w${txCounter++}`, text: input.text, status: "doing", set_at: "2026-08-12T10:00:00Z", confirmed_at: null }; ov.undoable_transition_id = `t${txCounter++}`; });
        }
        case "clear_commitment": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.__prev = ov.current_commitment; ov.current_commitment = null; ov.undoable_transition_id = `t${txCounter++}`; });
        }
        case "undo_commitment_transition": {
          const code = checkFail(); if (code) return fail(code);
          // A real inverse: restore the commitment captured before the last change.
          return bump(input.project_id, (ov) => { ov.current_commitment = ov.__prev ?? null; ov.__prev = undefined; ov.undoable_transition_id = null; });
        }
        case "complete_project_setup": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.status = "active"; ov.objective = input.objective; ov.desired_outcome = input.desired_outcome; ov.current_commitment = { work_item_id: `${input.project_id}-w1`, text: input.first_commitment, status: "doing", set_at: "2026-08-12T10:00:00Z", confirmed_at: null }; ov.review_reasons = []; ov.undoable_transition_id = `t${txCounter++}`; });
        }
        case "save_project_framing": {
          const code = checkFail(); if (code) return fail(code);
          return bump(input.project_id, (ov) => { ov.objective = input.objective; ov.desired_outcome = input.desired_outcome; ov.phase = input.phase ?? null; });
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
