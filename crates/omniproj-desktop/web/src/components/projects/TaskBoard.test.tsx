import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { projectId } from "../../domain/project";
import { I18nProvider } from "../../i18n/I18nProvider";
import { TaskBoard } from "./TaskBoard";

const PROJECT_ID = projectId("project-task-board");
const TASKS = {
  revision: "7",
  tasks: [{
    id: "a1b2",
    text: "Validate cohort labels",
    status: "open",
    unclear: true,
    due: null,
    note: null,
    tags: ["论文", "infra"],
    commits: [],
    adopted_from_proposal_id: null,
    linked_work_item_id: null,
    is_current_commitment: false,
    updated_at: "2026-08-12T09:00:00Z",
  }, {
    id: "c3d4",
    text: "Refactor the pipeline",
    status: "doing",
    unclear: false,
    due: null,
    note: null,
    tags: ["infra"],
    commits: [],
    adopted_from_proposal_id: null,
    linked_work_item_id: null,
    is_current_commitment: false,
    updated_at: "2026-08-11T09:00:00Z",
  }],
};

function renderBoard() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><I18nProvider initialLocale="en"><TaskBoard projectId={PROJECT_ID} hasCurrentCommitment={false} /></I18nProvider></QueryClientProvider>);
}

afterEach(() => invokeMock.mockReset());

it("selectively adopts Advance candidates and retains the proposal id", async () => {
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (command === "get_tasks") return TASKS;
    if (command === "advance_task") return { proposal_id: "proposal-42", candidates: ["Define cohort", "Run evaluation"] };
    if (command === "adopt_subtasks") {
      expect(args?.input).toMatchObject({ expected_revision: "7", proposal_id: "proposal-42", texts: ["Define cohort"] });
      return { revision: "8", tasks: TASKS.tasks };
    }
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();
  // Advance lives in the row's edit panel now, so the row is opened first.
  await user.click(await screen.findByRole("button", { name: /Validate cohort labels/ }));
  await user.click(await screen.findByRole("button", { name: /ask agent/i }));
  const choices = await screen.findAllByRole("checkbox");
  await user.click(choices.at(-2)!);
  await user.click(screen.getByRole("button", { name: /adopt selected/i }));
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "adopt_subtasks")).toBe(true));
});

it("promotes a planning task to the current commitment with both revisions", async () => {
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (command === "get_tasks") return TASKS;
    if (command === "promote_task_to_commitment") {
      expect(args?.input).toEqual({ project_id: PROJECT_ID, task_id: "a1b2", expected_task_revision: "7", expected_project_revision: 7 });
      return {};
    }
    if (command === "list_project_index" || command === "get_project_overview") return {};
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();
  await user.click(await screen.findByRole("button", { name: /Validate cohort labels/ }));
  await user.click((await screen.findAllByRole("button", { name: /make current commitment/i }))[0]);
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "promote_task_to_commitment")).toBe(true));
});

it("shows an unambiguous empty YYYY-MM-DD due-date field", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return TASKS;
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();
  await user.click(await screen.findByRole("button", { name: /Validate cohort labels/ }));
  expect(await screen.findByLabelText(/expected completion date: Validate cohort labels/i)).toHaveAttribute(
    "placeholder",
    "YYYY-MM-DD",
  );
});

it("renders tag chips, filters with AND semantics, and clears the filter", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return TASKS;
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();

  // Both tasks visible, chips rendered with the user's casing.
  expect(await screen.findByText("Validate cohort labels", { exact: false })).toBeInTheDocument();
  expect(screen.getByText("Refactor the pipeline", { exact: false })).toBeInTheDocument();

  // AND filter: 论文 + infra keeps only the first task.
  const group = screen.getByRole("group", { name: /filter by tag/i });
  await user.click(within(group).getByRole("button", { name: "论文" }));
  await user.click(within(group).getByRole("button", { name: "infra" }));
  expect(screen.getByText("Validate cohort labels", { exact: false })).toBeInTheDocument();
  expect(screen.queryByText("Refactor the pipeline", { exact: false })).not.toBeInTheDocument();

  // Clear restores everything.
  await user.click(screen.getByRole("button", { name: /clear tag filter/i }));
  expect(screen.getByText("Refactor the pipeline", { exact: false })).toBeInTheDocument();
});

it("keeps rows read-only until opened, then autosaves the edit on blur", async () => {
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (command === "get_tasks") return TASKS;
    if (command === "update_task") {
      expect(args?.input).toMatchObject({ id: "a1b2", tags: ["论文", "eval"] });
      return TASKS;
    }
    if (command === "list_project_index" || command === "get_project_overview") return {};
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();

  // Collapsed: the row is a single expandable control, with no editing fields on screen.
  const row = await screen.findByRole("button", { name: /Validate cohort labels/ });
  expect(row).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByLabelText(/tags: Validate cohort labels/i)).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /save task/i })).not.toBeInTheDocument();

  await user.click(row);
  expect(row).toHaveAttribute("aria-expanded", "true");
  const input = await screen.findByLabelText(/tags: Validate cohort labels/i);
  await user.clear(input);
  await user.type(input, "论文, eval");
  // Leaving the panel is what persists: no explicit save control exists.
  await user.tab();
  await user.click(screen.getByRole("button", { name: /Refactor the pipeline/ }));
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "update_task")).toBe(true));
});

it("does not send an update when nothing changed", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return TASKS;
    if (command === "list_project_index" || command === "get_project_overview") return {};
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();
  const row = await screen.findByRole("button", { name: /Validate cohort labels/ });
  await user.click(row);
  await user.click(screen.getByRole("button", { name: /Refactor the pipeline/ }));
  expect(invokeMock.mock.calls.some((call) => call[0] === "update_task")).toBe(false);
});

it("parseTagsInput splits comma variants and trims", async () => {
  const { parseTagsInput } = await import("./TaskBoard");
  expect(parseTagsInput("论文, infra、eval ，  x ")).toEqual(["论文", "infra", "eval", "x"]);
  expect(parseTagsInput("   ")).toEqual([]);
});

it("board view moves a card via the keyboard-accessible control and locks commitment cards", async () => {
  window.localStorage.setItem("omniproj.task-view", "board");
  const boardTasks = {
    revision: "9",
    tasks: [
      { ...TASKS.tasks[0], id: "m1", text: "Movable card", unclear: false, tags: [] },
      { ...TASKS.tasks[1], id: "l1", text: "Locked card", status: "doing", linked_work_item_id: "l1", tags: [] },
    ],
  };
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (command === "get_tasks") return boardTasks;
    if (command === "update_task") {
      expect(args?.input).toMatchObject({ id: "m1", status: "doing", tags: [] });
      return boardTasks;
    }
    if (command === "list_project_index" || command === "get_project_overview") return {};
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();

  const columns = await screen.findByTestId("task-board-columns");
  // Locked card exposes guidance instead of a move control.
  expect(within(columns).getByText(/managed by commitment actions/i)).toBeInTheDocument();
  expect(within(columns).queryByLabelText(/move to: Locked card/i)).not.toBeInTheDocument();
  // Keyboard-accessible move on the unlocked card.
  await user.selectOptions(within(columns).getByLabelText(/move to: Movable card/i), "doing");
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "update_task")).toBe(true));
  window.localStorage.removeItem("omniproj.task-view");
});

it("board view folds the done column beyond five and expands on demand", async () => {
  window.localStorage.setItem("omniproj.task-view", "board");
  const done = Array.from({ length: 7 }, (_, index) => ({
    ...TASKS.tasks[0],
    id: `done-${index}`,
    text: `Done item ${index}`,
    status: "done",
    tags: [],
    updated_at: `2026-08-${String(10 + index).padStart(2, "0")}T00:00:00Z`,
  }));
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return { revision: "9", tasks: done };
    throw new Error(`unexpected command ${command}`);
  });
  const user = userEvent.setup();
  renderBoard();

  const columns = await screen.findByTestId("task-board-columns");
  // Newest five shown, oldest two folded.
  expect(within(columns).getByText("Done item 6", { exact: false })).toBeInTheDocument();
  expect(within(columns).queryByText("Done item 0", { exact: false })).not.toBeInTheDocument();
  await user.click(within(columns).getByRole("button", { name: /show all \(7\)/i }));
  expect(within(columns).getByText("Done item 0", { exact: false })).toBeInTheDocument();
  window.localStorage.removeItem("omniproj.task-view");
});

it("time view groups tasks by due and hides done", async () => {
  window.localStorage.setItem("omniproj.task-view", "time");
  const timeTasks = {
    revision: "9",
    tasks: [
      { ...TASKS.tasks[0], id: "t-over", text: "Overdue thing", unclear: false, tags: [], due: "2000-01-01" },
      { ...TASKS.tasks[1], id: "t-none", text: "Unscheduled thing", tags: [], due: null },
      { ...TASKS.tasks[1], id: "t-done", text: "Finished thing", status: "done", tags: [], due: "2000-01-01" },
    ],
  };
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return timeTasks;
    throw new Error(`unexpected command ${command}`);
  });
  renderBoard();
  const groups = await screen.findByTestId("task-time-groups");
  expect(within(groups).getByText("Overdue thing", { exact: false })).toBeInTheDocument();
  expect(within(groups).getByText("Unscheduled thing", { exact: false })).toBeInTheDocument();
  expect(within(groups).queryByText("Finished thing", { exact: false })).not.toBeInTheDocument();
  expect(within(groups).getByRole("heading", { name: /overdue/i })).toBeInTheDocument();
  expect(within(groups).getByRole("heading", { name: /unscheduled/i })).toBeInTheDocument();
  window.localStorage.removeItem("omniproj.task-view");
});
