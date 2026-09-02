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
  await user.click((await screen.findAllByRole("button", { name: /make current commitment/i }))[0]);
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "promote_task_to_commitment")).toBe(true));
});

it("shows an unambiguous empty YYYY-MM-DD due-date field", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_tasks") return TASKS;
    throw new Error(`unexpected command ${command}`);
  });
  renderBoard();
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

it("sends parsed tags on save", async () => {
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
  const input = await screen.findByLabelText(/tags: Validate cohort labels/i);
  await user.clear(input);
  await user.type(input, "论文, eval");
  await user.click(screen.getAllByRole("button", { name: /save task/i })[0]);
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "update_task")).toBe(true));
});

it("parseTagsInput splits comma variants and trims", async () => {
  const { parseTagsInput } = await import("./TaskBoard");
  expect(parseTagsInput("论文, infra、eval ，  x ")).toEqual(["论文", "infra", "eval", "x"]);
  expect(parseTagsInput("   ")).toEqual([]);
});
