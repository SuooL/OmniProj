import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { I18nProvider } from "../../i18n/I18nProvider";
import { FocusStrip } from "./FocusStrip";

const AGENDA = {
  total_items: 2,
  projects: [
    {
      project_id: "p-late",
      name: "Late project",
      items: [
        { id: "w1", text: "Ship the milestone", due: "2026-08-01", overdue_days: 32 },
        { id: "w2", text: "Review the draft", due: "2026-09-02", overdue_days: 0 },
      ],
    },
  ],
};

function renderStrip() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <I18nProvider initialLocale="en">
        <MemoryRouter initialEntries={["/projects"]}>
          <FocusStrip />
        </MemoryRouter>
      </I18nProvider>
    </QueryClientProvider>,
  );
}

afterEach(() => invokeMock.mockReset());

it("renders nothing at all when nothing is due", async () => {
  invokeMock.mockResolvedValue({ total_items: 0, projects: [] });
  renderStrip();
  await waitFor(() => expect(invokeMock).toHaveBeenCalled());
  expect(screen.queryByTestId("focus-strip")).not.toBeInTheDocument();
});

it("collapses to a one-line summary and expands to project-grouped jump links", async () => {
  invokeMock.mockResolvedValue(AGENDA);
  const user = userEvent.setup();
  renderStrip();

  const toggle = await screen.findByRole("button", { expanded: false });
  expect(toggle).toHaveTextContent("2 task(s) overdue or due today across 1 project(s)");
  // Collapsed: no detail rows yet.
  expect(screen.queryByText("Ship the milestone")).not.toBeInTheDocument();

  await user.click(toggle);
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByText("Ship the milestone")).toBeInTheDocument();
  expect(screen.getByText("Overdue 32d")).toBeInTheDocument();
  expect(screen.getByText("Due today")).toBeInTheDocument();
  // Read-only: the project name is a jump link into the project, not an editor.
  expect(screen.getByRole("link", { name: "Late project" })).toHaveAttribute(
    "href",
    "/projects/p-late/overview",
  );
});
