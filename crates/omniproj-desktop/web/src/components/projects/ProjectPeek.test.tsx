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
import type { ProjectOverview } from "../../domain/project";
import { queryKeys } from "../../queryKeys";
import {
  currentCommitment,
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
  window.history.replaceState(null, "", "/");
});
afterEach(() => invokeMock.mockReset());

describe("content order and source", () => {
  it("renders the fixed section order and shows the full source path only here", async () => {
    renderOverview(overview({ source: projectSource({ location: "/Users/dev/omni" }) }));
    await screen.findByTestId("project-overview");

    const order = [
      "overview-identity",
      "review-reasons",
      "current-commitment",
      "observed-actual",
      "commitment-history",
    ].map((id) => screen.getByTestId(id));

    for (let i = 1; i < order.length; i++) {
      expect(
        order[i - 1].compareDocumentPosition(order[i]) & Node.DOCUMENT_POSITION_FOLLOWING,
      ).toBeTruthy();
    }
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
    renderOverview(
      overview({
        source: projectSource({ status: "missing" }),
        observed_actual: observedActual({ observed_at: "2026-08-10T09:00:00Z" }),
      }),
    );
    await screen.findByTestId("observed-actual");
    expect(screen.getByTestId("observed-stale")).toBeInTheDocument();
    expect(screen.getByTestId("source-recovery")).toBeInTheDocument();
    expect(screen.queryByText(/inactiv/i)).not.toBeInTheDocument();
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

describe("commitment mutations", () => {
  it("sets a commitment with the expected revision and announces success", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ current_commitment: null, revision: 3 }), {
      set_commitment: () => overview({ current_commitment: currentCommitment(), revision: 4 }),
    });
    await screen.findByTestId("set-form");

    await user.type(screen.getByLabelText("New commitment"), "Do the thing");
    await user.click(screen.getByRole("button", { name: "Save commitment" }));

    await waitFor(() => expect(callsTo("set_commitment")).toHaveLength(1));
    const [, arg] = callsTo("set_commitment")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ expected_revision: 3, text: "Do the thing" });
    expect(await screen.findByTestId("live-polite")).toHaveTextContent(/commitment set/i);
  });

  it("requires a reason to replace", async () => {
    const user = userEvent.setup();
    renderOverview(overview());
    await screen.findByTestId("current-commitment");
    await user.click(screen.getByRole("button", { name: "Replace" }));

    await user.type(screen.getByLabelText("New commitment"), "New plan");
    expect(screen.getByRole("button", { name: "Save replacement" })).toBeDisabled();
    await user.type(screen.getByLabelText("Replace reason"), "scope changed");
    expect(screen.getByRole("button", { name: "Save replacement" })).toBeEnabled();
  });

  it("completing a commitment never auto-creates a replacement", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      complete_commitment: () => overview({ current_commitment: null, revision: 2 }),
    });
    await screen.findByTestId("current-commitment");
    await user.click(screen.getByRole("button", { name: "Complete" }));

    await waitFor(() => expect(callsTo("complete_commitment")).toHaveLength(1));
    expect(callsTo("set_commitment")).toHaveLength(0);
    expect(callsTo("replace_commitment")).toHaveLength(0);
  });

  it("on revision_conflict refetches, keeps the draft, and shows a comparison note", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ current_commitment: null }), {
      set_commitment: () => {
        throw { code: "revision_conflict", message: "expected 1 found 2", retryable: false, state_applied: false };
      },
    });
    await screen.findByTestId("set-form");
    await user.type(screen.getByLabelText("New commitment"), "kept draft");
    await user.click(screen.getByRole("button", { name: "Save commitment" }));

    expect(await screen.findByTestId("conflict-note")).toBeInTheDocument();
    expect(screen.getByLabelText("New commitment")).toHaveValue("kept draft"); // draft retained
    await waitFor(() => expect(callsTo("get_project_overview").length).toBeGreaterThanOrEqual(2)); // refetched
  });

  it("on store_write_failed keeps the draft and offers Retry and Copy", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ current_commitment: null }), {
      set_commitment: () => {
        throw { code: "store_write_failed", message: "disk full", retryable: true, state_applied: false };
      },
    });
    await screen.findByTestId("set-form");
    await user.type(screen.getByLabelText("New commitment"), "retry me");
    await user.click(screen.getByRole("button", { name: "Save commitment" }));

    const err = await screen.findByTestId("write-error");
    expect(within(err).getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(within(err).getByRole("button", { name: "Copy text" })).toBeInTheDocument();
    expect(screen.getByLabelText("New commitment")).toHaveValue("retry me");
  });

  it("on audit_commit_failed (state_applied) announces, refetches, and never resends", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ current_commitment: null }), {
      set_commitment: () => {
        throw {
          code: "audit_commit_failed",
          message: "saved as 5 but audit failed",
          retryable: false,
          state_applied: true,
          durable_revision: 5,
        };
      },
    });
    await screen.findByTestId("set-form");
    await user.type(screen.getByLabelText("New commitment"), "durable");
    await user.click(screen.getByRole("button", { name: "Save commitment" }));

    expect(await screen.findByTestId("audit-failed-note")).toBeInTheDocument();
    expect(screen.getByTestId("live-assertive")).toHaveTextContent(/state saved; audit commit failed/i);
    expect(callsTo("set_commitment")).toHaveLength(1); // never resent
    await waitFor(() => expect(callsTo("get_project_overview").length).toBeGreaterThanOrEqual(2));
    expect(screen.getByLabelText("New commitment")).toHaveValue(""); // durable -> draft cleared
  });

  it("resubmits a conflicted mutation with the revision rebuilt from the refetch", async () => {
    const user = userEvent.setup();
    let getCount = 0;
    let setCount = 0;
    window.history.replaceState(null, "", "/projects/project-1/overview");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_project_overview") {
        getCount += 1;
        return overview({ current_commitment: null, revision: getCount === 1 ? 3 : 5 });
      }
      if (command === "set_commitment") {
        setCount += 1;
        if (setCount === 1) {
          throw { code: "revision_conflict", message: "stale", retryable: false, state_applied: false };
        }
        return overview({ current_commitment: currentCommitment(), revision: 6 });
      }
      return overview();
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity, refetchOnMount: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByTestId("set-form");
    await user.type(screen.getByLabelText("New commitment"), "do it");
    await user.click(screen.getByRole("button", { name: "Save commitment" }));
    await screen.findByTestId("conflict-note");

    await user.click(screen.getByRole("button", { name: "Save commitment" }));
    await waitFor(() => expect(callsTo("set_commitment")).toHaveLength(2));
    const [, arg] = callsTo("set_commitment")[1] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ expected_revision: 5, text: "do it" });
  });

  it("shows Undo only when a newest undoable transition is returned", async () => {
    renderOverview(overview({ undoable_transition_id: null }));
    await screen.findByTestId("current-commitment");
    expect(screen.queryByTestId("undo-button")).not.toBeInTheDocument();
  });
});

describe("lifecycle and source recovery", () => {
  it("enforces reason + review date before enabling a Waiting save", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ status: "active", revision: 1 }), {
      set_project_status: () => overview({ status: "waiting", revision: 2 }),
    });
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
