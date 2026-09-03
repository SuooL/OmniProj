// Project Overview contract: content order, atomic setup, source-failure recovery,
// commitment mutations with the full error model, Undo gating, focus, and responsive behavior.
// Exercised through <App/> so routing, the announcer live regions, and the query cache all run
// exactly as they ship.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { App } from "../../App";
import { transitionId, type ProjectOverview } from "../../domain/project";
import { queryKeys } from "../../queryKeys";
import {
  commitmentTransition,
  indexItem,
  indexResponse,
  observedActual,
  overview,
  projectSource,
  reviewPolicy,
  reviewReason,
} from "../../test/fixtures";
import { mediaState } from "../../test/setup";

type Handlers = Record<string, (args: { input?: Record<string, unknown> }) => unknown>;

/** Render <App/> on the full-page Overview route for `ov`, with per-command IPC handlers. */
function renderOverview(ov: ProjectOverview, handlers: Handlers = {}) {
  window.history.replaceState(null, "", `/projects/${ov.project_id}/overview`);
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (handlers[command]) return handlers[command](args ?? {});
    if (command === "get_project_overview") return ov;
    if (command === "list_project_index") {
      return indexResponse([indexItem({ project_id: ov.project_id, name: ov.name })]);
    }
    if (command === "refresh_projects") return [];
    return ov; // mutations echo the overview by default
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity, refetchOnMount: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

/** Render <App/> on the Index, seeded, so a row click opens the full project surface. */
function renderIndexThenPeek(ov: ProjectOverview) {
  window.history.replaceState(null, "", "/projects");
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_project_overview") return ov;
    if (command === "refresh_projects") return [];
    return { projects: [], review_policy: reviewPolicy };
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity, refetchOnMount: false } },
  });
  client.setQueryData(
    queryKeys.projectIndex,
    indexResponse([indexItem({ project_id: ov.project_id, name: ov.name })]),
  );
  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

function callsTo(command: string): unknown[][] {
  return invokeMock.mock.calls.filter((c) => c[0] === command);
}

beforeEach(() => {
  window.sessionStorage.clear();
  window.localStorage.clear();
  window.localStorage.setItem("omniproj.locale", "en");
  window.history.replaceState(null, "", "/");
});
afterEach(() => invokeMock.mockReset());

describe("content order and source", () => {
  it("starts with the current next step and reveals repository detail only on demand", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ source: projectSource({ location: "/Users/dev/omni" }) }));
    await screen.findByTestId("project-overview");

    const order = [
      "overview-identity",
      "reentry-context",
    ].map((id) => screen.getByTestId(id));
    // The next step is now the head of the task list rather than a surface of its own.
    expect(screen.getByTestId("now-doing")).toBeInTheDocument();

    for (let i = 1; i < order.length; i++) {
      expect(
        order[i - 1].compareDocumentPosition(order[i]) & Node.DOCUMENT_POSITION_FOLLOWING,
      ).toBeTruthy();
    }
    expect(screen.queryByTestId("source-path")).not.toBeInTheDocument();
    expect(screen.queryByTestId("observed-actual")).not.toBeInTheDocument();

    await user.click(screen.getByText("Planning and tasks"));
    expect(screen.queryByTestId("source-path")).not.toBeInTheDocument();

    await user.click(screen.getByText("View observed change"));
    expect(await screen.findByTestId("observed-actual")).toBeInTheDocument();
    expect(screen.getByTestId("source-path")).toHaveTextContent("/Users/dev/omni");
  });

  it("shows the server-provided review-action evidence (interval + last set) verbatim", async () => {
    renderOverview(
      overview({
        review_reasons: [
          reviewReason("review_action", [
            "Commitment review interval: 7 days",
            "Last confirmed 2026-08-01T00:00:00Z",
          ]),
        ],
      }),
    );
    await screen.findByTestId("review-reasons");
    expect(screen.getByText("Commitment review interval: 7 days")).toBeInTheDocument();
    expect(screen.getByText("Last confirmed 2026-08-01T00:00:00Z")).toBeInTheDocument();
  });

  it("on source failure shows cached facts with a timestamp and recovery, never inactivity wording", async () => {
    const user = userEvent.setup();
    renderOverview(
      overview({
        source: projectSource({ status: "missing" }),
        observed_actual: observedActual({ observed_at: "2026-08-10T09:00:00Z" }),
      }),
    );
    expect(await screen.findByTestId("source-recovery")).toBeInTheDocument();
    expect(screen.queryByTestId("observed-actual")).not.toBeInTheDocument();
    expect(screen.queryByText(/inactiv/i)).not.toBeInTheDocument();

    await user.click(screen.getByText("View observed change"));
    expect(await screen.findByTestId("observed-stale")).toBeInTheDocument();
  });
});

describe("atomic setup", () => {
  it("focuses objective and completes setup in one call with expected revision, no prior framing write", async () => {
    const user = userEvent.setup();
    const ov = overview({
      status: "setup",
      objective: null,
      desired_outcome: null,
      current_commitment: null,
      review_reasons: [reviewReason("complete_setup")],
      revision: 0,
    });
    renderOverview(ov, {
      complete_project_setup: () => overview({ status: "active", revision: 1 }),
    });
    await screen.findByTestId("framing-form");

    expect(screen.getByLabelText("Objective")).toHaveFocus();

    await user.type(screen.getByLabelText("Objective"), "Ship R0");
    await user.type(screen.getByLabelText("Desired outcome"), "Dogfood");
    await user.type(screen.getByLabelText("First commitment"), "Wire the service");
    await user.click(screen.getByRole("button", { name: "Complete setup" }));

    await waitFor(() => expect(callsTo("complete_project_setup")).toHaveLength(1));
    expect(callsTo("save_project_framing")).toHaveLength(0);
    const [, arg] = callsTo("complete_project_setup")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({
      expected_revision: 0,
      objective: "Ship R0",
      desired_outcome: "Dogfood",
      first_commitment: "Wire the service",
    });
  });
});

// The commitment is no longer its own surface: it is the head of the task list. Tests for the
// free-text "set commitment" form and the replace-with-reason form are gone with those forms.
// The error model, refetch behaviour and Undo gating still apply and are exercised here through
// the actions that remain.
describe("the current step, run from the task list", () => {
  const TASKS = { revision: "1", tasks: [] };
  const completed = commitmentTransition({ type: "completed", id: transitionId("transition-9") });

  it("completes the current step and never auto-creates a replacement", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => TASKS,
      complete_commitment: () => overview({ current_commitment: null, revision: 2 }),
    });
    await screen.findByTestId("now-doing");
    await user.click(screen.getByRole("button", { name: "Complete" }));

    await waitFor(() => expect(callsTo("complete_commitment")).toHaveLength(1));
    expect(callsTo("set_commitment")).toHaveLength(0);
    expect(callsTo("replace_commitment")).toHaveLength(0);
  });

  it("switches away by releasing the step, with no replacement demanded up front", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => TASKS,
      clear_commitment: () => overview({ current_commitment: null, revision: 2 }),
    });
    await screen.findByTestId("now-doing");
    await user.click(screen.getByRole("button", { name: "Switch away" }));

    await waitFor(() => expect(callsTo("clear_commitment")).toHaveLength(1));
    expect(callsTo("replace_commitment")).toHaveLength(0);
  });

  it("on revision_conflict refetches and shows a comparison note", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => TASKS,
      complete_commitment: () => {
        throw { code: "revision_conflict", message: "expected 1 found 2", retryable: false, state_applied: false };
      },
    });
    await screen.findByTestId("now-doing");
    await user.click(screen.getByRole("button", { name: "Complete" }));

    expect(await screen.findByTestId("conflict-note")).toBeInTheDocument();
    await waitFor(() => expect(callsTo("get_project_overview").length).toBeGreaterThanOrEqual(2));
  });

  it("on store_write_failed offers Retry", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => TASKS,
      complete_commitment: () => {
        throw { code: "store_write_failed", message: "disk full", retryable: true, state_applied: false };
      },
    });
    await screen.findByTestId("now-doing");
    await user.click(screen.getByRole("button", { name: "Complete" }));

    const err = await screen.findByTestId("write-error");
    expect(within(err).getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("on audit_commit_failed (state_applied) announces, refetches, and never resends", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => TASKS,
      complete_commitment: () => {
        throw {
          code: "audit_commit_failed",
          message: "saved as 5 but audit failed",
          retryable: false,
          state_applied: true,
          durable_revision: 5,
        };
      },
    });
    await screen.findByTestId("now-doing");
    await user.click(screen.getByRole("button", { name: "Complete" }));

    expect(await screen.findByTestId("audit-failed-note")).toBeInTheDocument();
    expect(screen.getByTestId("live-assertive")).toHaveTextContent(/state saved; audit commit failed/i);
    expect(callsTo("complete_commitment")).toHaveLength(1); // never resent
    await waitFor(() => expect(callsTo("get_project_overview").length).toBeGreaterThanOrEqual(2));
  });

  it("offers Undo for a completed step, but never for a set", async () => {
    renderOverview(
      overview({ last_transition: completed, undoable_transition_id: completed.id }),
      { get_tasks: () => TASKS },
    );
    expect(await screen.findByTestId("undo-button")).toBeInTheDocument();
  });

  it("withholds Undo for a set, whose undo would abandon the task", async () => {
    // `overview()` fixture's newest transition is a `set`.
    renderOverview(overview(), { get_tasks: () => TASKS });
    await screen.findByTestId("now-doing");
    expect(screen.queryByTestId("undo-button")).not.toBeInTheDocument();
  });

  it("withholds Undo when no undoable transition is returned", async () => {
    renderOverview(overview({ undoable_transition_id: null }), { get_tasks: () => TASKS });
    await screen.findByTestId("now-doing");
    expect(screen.queryByTestId("undo-button")).not.toBeInTheDocument();
  });
});

describe("lifecycle and source recovery", () => {
  it("enforces reason + review date before enabling a Waiting save", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ status: "active", revision: 1 }), {
      set_project_status: () => overview({ status: "waiting", revision: 2 }),
    });
    await screen.findByTestId("project-overview");
    await user.click(screen.getByText("Project management"));
    const control = within(await screen.findByTestId("lifecycle-control"));
    await user.selectOptions(control.getByLabelText("Set status"), "waiting");
    const save = control.getByRole("button", { name: "Update status" });
    expect(save).toBeDisabled();
    await user.type(control.getByLabelText("Status reason"), "waiting on API");
    expect(save).toBeDisabled();
    await user.type(control.getByLabelText("Review date"), "2026-09-01");
    expect(save).toBeEnabled();

    await user.click(save);
    await waitFor(() => expect(callsTo("set_project_status")).toHaveLength(1));
    const [, arg] = callsTo("set_project_status")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({
      status: "waiting",
      reason: "waiting on API",
      review_at: "2026-09-01T00:00:00Z",
      expected_revision: 1,
    });
  });

  it("requires archive confirmation, and returns to active with no reason or date", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ status: "parked", status_reason: "later" }), {
      set_project_status: () => overview({ status: "active", revision: 2 }),
    });
    await screen.findByTestId("project-overview");
    await user.click(screen.getByText("Project management"));
    const control = within(await screen.findByTestId("lifecycle-control"));

    await user.selectOptions(control.getByLabelText("Set status"), "archived");
    expect(control.getByRole("button", { name: "Update status" })).toBeDisabled();
    await user.click(control.getByLabelText("Confirm archive"));
    expect(control.getByRole("button", { name: "Update status" })).toBeEnabled();

    await user.selectOptions(control.getByLabelText("Set status"), "active");
    await user.click(control.getByRole("button", { name: "Update status" }));
    await waitFor(() => expect(callsTo("set_project_status")).toHaveLength(1));
    const [, arg] = callsTo("set_project_status")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ status: "active", reason: null, review_at: null });
  });

  it("surfaces the source-recovery affordance when the source has moved (relink flow covered in AddProjectDialog.test)", async () => {
    renderOverview(overview({ source: projectSource({ status: "missing", location: "/old", revision: 2 }) }));
    expect(await screen.findByTestId("source-recovery")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /choose new location/i })).toBeInTheDocument();
  });
});

describe("desktop detail focus and responsive", () => {
  it("opens the project in the main content surface and focuses its heading", async () => {
    const user = userEvent.setup();
    renderIndexThenPeek(overview({ project_id: overview().project_id, name: "Alpha" }));

    const row = within(await screen.findByTestId("projects-index")).getByRole("link", {
      name: /^Alpha\b/,
    });
    await user.click(row);

    const page = await screen.findByTestId("overview-page");
    await waitFor(() => expect(within(page).getByTestId("overview-heading")).toHaveFocus());
    expect(screen.queryByTestId("projects-index")).not.toBeInTheDocument();
  });

  it("below 800px renders a full-page detail with no Index or Peek landmark", async () => {
    mediaState.matches = false;
    const user = userEvent.setup();
    renderIndexThenPeek(overview({ project_id: overview().project_id, name: "Alpha" }));

    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", { name: /^Alpha\b/ }),
    );

    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument();
    expect(screen.queryByTestId("projects-index")).not.toBeInTheDocument();
  });

  it("direct access always renders a full page, never a Peek", async () => {
    renderOverview(overview());
    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument();
  });
});
