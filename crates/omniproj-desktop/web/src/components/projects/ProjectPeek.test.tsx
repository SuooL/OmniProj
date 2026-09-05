// Project page contract: content order, the outline's keyboard model and write path,
// atomic setup, source-failure recovery, lifecycle, focus, and responsive behavior.
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
  indexItem,
  indexResponse,
  observedActual,
  overview,
  projectSource,
  reviewPolicy,
  reviewReason,
  task,
  taskList,
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
  it("puts the steps on the page itself and hides repository detail behind a disclosure", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ source: projectSource({ location: "/Users/dev/omni" }) }), {
      get_tasks: () => taskList([task()]),
    });
    await screen.findByTestId("project-overview");

    // The list is the page. It is not behind a tab, an accordion, or a second click.
    expect(await screen.findByTestId("task-outline")).toBeInTheDocument();
    const identity = screen.getByTestId("overview-identity");
    const outline = screen.getByTestId("task-outline");
    expect(
      identity.compareDocumentPosition(outline) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    // Reference material stays collapsed until asked for.
    const commits = screen.getByText("Commits and branch graph").closest("details")!;
    expect(commits).not.toHaveAttribute("open");
    await user.click(screen.getByText("Commits and branch graph"));
    expect(commits).toHaveAttribute("open");
    expect(await screen.findByText("/Users/dev/omni")).toBeInTheDocument();
  });

  it("carries none of the vocabulary the redesign removed", async () => {
    renderOverview(overview(), { get_tasks: () => taskList([task()]) });
    await screen.findByTestId("project-overview");

    for (const gone of [/commitment/i, /re-enter/i, /observed actual/i, /needs review/i]) {
      expect(screen.queryByText(gone)).not.toBeInTheDocument();
    }
    expect(screen.queryByTestId("now-doing")).not.toBeInTheDocument();
    expect(screen.queryByTestId("reentry-context")).not.toBeInTheDocument();
  });

  it("on source failure offers recovery instead of stale-fact wording", async () => {
    renderOverview(
      overview({
        source: projectSource({ status: "missing" }),
        observed_actual: observedActual({ observed_at: "2026-08-10T09:00:00Z" }),
      }),
      { get_tasks: () => taskList([]) },
    );
    expect(await screen.findByTestId("source-recovery")).toBeInTheDocument();
    expect(screen.queryByText(/inactiv/i)).not.toBeInTheDocument();
  });
});

describe("atomic setup", () => {
  it("asks only for the first step, and completes setup in one call", async () => {
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

    // The one required field takes focus; nothing else stands between the user and a list.
    expect(screen.getByLabelText("First step")).toHaveFocus();
    expect(screen.queryByLabelText("Objective")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Desired outcome")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("First step"), "Wire the service");
    await user.click(screen.getByRole("button", { name: "Start this project" }));

    await waitFor(() => expect(callsTo("complete_project_setup")).toHaveLength(1));
    expect(callsTo("save_project_framing")).toHaveLength(0);
    const [, arg] = callsTo("complete_project_setup")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({
      expected_revision: 0,
      first_commitment: "Wire the service",
    });
  });
});

// The outline is the only place work is edited, so its keyboard model and its write path
// are contract, not detail. The error model these tests exercise (conflict, refetch) used to
// live on the commitment actions; it applies to every step write now.
describe("the outline", () => {
  const PARENT = task({ id: "step-1", text: "Extract reports" });
  const CHILD = task({ id: "step-2", text: "OCR", parent_id: "step-1", depth: 1 });
  const SECOND = task({ id: "step-3", text: "Build the framework" });

  it("numbers top-level steps and nests sub-steps under them", async () => {
    renderOverview(overview(), { get_tasks: () => taskList([PARENT, CHILD, SECOND]) });
    const rows = await screen.findAllByTestId("outline-row");

    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("1.");
    expect(rows[1]).toHaveAttribute("data-depth", "1");
    // The sub-step is not numbered; numbering continues past it at the top level.
    expect(rows[2]).toHaveTextContent("2.");
  });

  it("ticks a step off in one click", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => taskList([PARENT]),
      update_task: () => taskList([{ ...PARENT, status: "done" }], "2"),
    });
    await screen.findByTestId("task-outline");

    await user.click(screen.getByRole("checkbox", { name: /Extract reports/ }));

    await waitFor(() => expect(callsTo("update_task")).toHaveLength(1));
    const [, arg] = callsTo("update_task")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ id: "step-1", status: "done", expected_revision: "1" });
  });

  it("Tab nests a step under the one above it", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => taskList([PARENT, SECOND]),
      move_task: () => taskList([PARENT, { ...SECOND, parent_id: "step-1", depth: 1 }], "2"),
    });
    await screen.findByTestId("task-outline");

    await user.click(screen.getByRole("button", { name: "Build the framework" }));
    await user.keyboard("{Tab}");

    await waitFor(() => expect(callsTo("move_task")).toHaveLength(1));
    const [, arg] = callsTo("move_task")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ id: "step-3", parent_id: "step-1", after_id: null });
  });

  it("Enter adds the next step as a sibling, right after this one", async () => {
    const user = userEvent.setup();
    renderOverview(overview(), {
      get_tasks: () => taskList([PARENT, CHILD]),
      add_task: () => taskList([PARENT, CHILD, task({ id: "step-9", text: "", parent_id: "step-1", depth: 1 })], "2"),
    });
    await screen.findByTestId("task-outline");

    await user.click(screen.getByRole("button", { name: "OCR" }));
    await user.keyboard("{Enter}");

    await waitFor(() => expect(callsTo("add_task")).toHaveLength(1));
    const [, arg] = callsTo("add_task")[0] as [string, { input: Record<string, unknown> }];
    expect(arg.input).toMatchObject({ parent_id: "step-1", after_id: "step-2" });
  });

  it("on a write conflict says so and reloads the list rather than retrying blind", async () => {
    const user = userEvent.setup();
    let reads = 0;
    renderOverview(overview(), {
      get_tasks: () => {
        reads += 1;
        return taskList([PARENT]);
      },
      update_task: () => {
        throw { code: "revision_conflict", message: "expected 1 found 2", retryable: false, state_applied: false };
      },
    });
    await screen.findByTestId("task-outline");
    const before = reads;

    await user.click(screen.getByRole("checkbox", { name: /Extract reports/ }));

    expect(await screen.findByTestId("outline-error")).toBeInTheDocument();
    await waitFor(() => expect(reads).toBeGreaterThan(before));
    expect(callsTo("update_task")).toHaveLength(1); // never resent
  });
});

describe("lifecycle and source recovery", () => {
  it("enforces reason + review date before enabling a Waiting save", async () => {
    const user = userEvent.setup();
    renderOverview(overview({ status: "active", revision: 1 }), {
      set_project_status: () => overview({ status: "waiting", revision: 2 }),
    });
    await screen.findByTestId("project-overview");
    await user.click(screen.getByText("Project settings"));
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
    await user.click(screen.getByText("Project settings"));
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
