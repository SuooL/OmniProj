import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
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
  await user.click(await screen.findByRole("button", { name: /make current commitment/i }));
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
