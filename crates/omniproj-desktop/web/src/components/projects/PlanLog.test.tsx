import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { projectId } from "../../domain/project";
import { I18nProvider } from "../../i18n/I18nProvider";
import { PlanLog } from "./PlanLog";

afterEach(() => invokeMock.mockReset());

it("anchors a decision to a commit picked from the repository, with the current revision", async () => {
  const project = projectId("project-plan");
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    if (command === "get_plan") return { revision: "plan-rev-1", entries: [{ id: "d1", date: "2026-08-31", title: "Use external validation", status: "planned", commit: null, body: "Stronger applicability evidence" }] };
    if (command === "get_commit_timeline") {
      return [{ sha: "deadbeef", short_sha: "deadbee", committed_at: "2026-08-31T10:00:00Z", author: "dev", subject: "Wire external validation", attributed_task_ids: [] }];
    }
    if (command === "set_plan_commit") {
      expect(args?.input).toEqual({ project_id: project, expected_revision: "plan-rev-1", id: "d1", commit: "deadbeef" });
      return { revision: "plan-rev-2", entries: [] };
    }
    throw new Error(`unexpected command ${command}`);
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const user = userEvent.setup();
  render(<QueryClientProvider client={client}><I18nProvider initialLocale="en"><PlanLog projectId={project} /></I18nProvider></QueryClientProvider>);
  // The anchor is picked from the repository's own commits, never typed by hand.
  const picker = await screen.findByRole("combobox", { name: /anchor to a commit: use external validation/i });
  await waitFor(() => expect(screen.getByRole("option", { name: /wire external validation/i })).toBeInTheDocument());
  await user.selectOptions(picker, "deadbeef");
  await waitFor(() => expect(invokeMock.mock.calls.some((call) => call[0] === "set_plan_commit")).toBe(true));
});
