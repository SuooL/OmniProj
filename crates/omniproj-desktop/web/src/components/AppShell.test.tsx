// AppShell keyboard, navigation, and announcement contract. The shell is exercised through
// <App/> so the router and the shortcuts run exactly as they ship.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { App } from "../App";
import type { ProjectIndexItem } from "../domain/project";
import { queryKeys } from "../queryKeys";
import { indexItem, indexResponse, overview, reviewPolicy } from "../test/fixtures";
import { LiveStatus } from "./LiveStatus";

function renderAppAt(path: string, index: ProjectIndexItem[] = []) {
  window.history.replaceState(null, "", path);
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, refetchOnMount: false },
    },
  });
  client.setQueryData(queryKeys.projectIndex, indexResponse(index));
  return render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

/** Dispatch a raw modified keydown so we can inspect `defaultPrevented`. */
function dispatchChord(key: string): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key,
    ctrlKey: true,
    bubbles: true,
    cancelable: true,
  });
  act(() => {
    window.dispatchEvent(event);
  });
  return event;
}

beforeEach(() => {
  invokeMock.mockImplementation(async (command: string, args?: { input?: { project_id?: string } }) => {
    if (command === "get_project_overview") {
      return overview({ project_id: (args?.input?.project_id ?? "project-1") as never });
    }
    return { projects: [], review_policy: reviewPolicy };
  });
  window.sessionStorage.clear();
  window.history.replaceState(null, "", "/");
});

afterEach(() => {
  invokeMock.mockReset();
});

describe("primary navigation", () => {
  it("exposes Projects as the only primary destination", async () => {
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    const nav = screen.getByRole("navigation", { name: /primary/i });
    const links = within(nav).getAllByRole("link");
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAccessibleName("Projects");
    expect(
      screen.queryByRole("link", { name: /settings|attention|agents?/i }),
    ).not.toBeInTheDocument();
  });
});

describe("keyboard shortcuts", () => {
  it("Cmd/Ctrl+F focuses the local filter", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    await user.keyboard("{Control>}f{/Control}");
    expect(screen.getByLabelText(/filter projects/i)).toHaveFocus();
  });

  it("Cmd/Ctrl+N opens Add Project, even while a text input is focused", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    await user.click(screen.getByLabelText(/filter projects/i));
    expect(screen.getByLabelText(/filter projects/i)).toHaveFocus();

    await user.keyboard("{Control>}n{/Control}");
    expect(screen.getByRole("dialog", { name: /add project/i })).toBeInTheDocument();
  });

  it("Cmd/Ctrl+R prevents default and pull-refreshes only while the window is focused", async () => {
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    // Unfocused window: neither the reload default nor a refresh.
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const unfocused = dispatchChord("r");
    expect(unfocused.defaultPrevented).toBe(false);
    expect(screen.getByTestId("live-polite")).toHaveTextContent("");

    // Focused window: default reload is prevented and a refresh is announced.
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const focused = dispatchChord("r");
    expect(focused.defaultPrevented).toBe(true);
    expect(screen.getByTestId("live-polite")).toHaveTextContent(/refreshing/i);
    expect(invokeMock).toHaveBeenCalledWith("list_project_index");
  });

  it("Cmd/Ctrl+R still refreshes while a text input is focused", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    await user.click(screen.getByLabelText(/filter projects/i));

    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    dispatchChord("r");
    expect(screen.getByTestId("live-polite")).toHaveTextContent(/refreshing/i);
  });
});

describe("stacked Escape", () => {
  it("closes the Add Project modal before the Peek", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-peek");

    await user.keyboard("{Control>}n{/Control}");
    expect(screen.getByRole("dialog", { name: /add project/i })).toBeInTheDocument();

    // First Escape: only the modal closes; the Peek stays open.
    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("dialog", { name: /add project/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("overview-peek")).toBeInTheDocument();

    // Second Escape: the Peek closes and the Index remains.
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId("projects-index")).toBeInTheDocument();
  });
});

describe("review-fix regressions", () => {
  it("writes the filter back to the canonical q search param", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    await user.type(screen.getByLabelText(/filter projects/i), "beta");

    expect(window.location.search).toBe("?q=beta");
  });

  it("dismissing a Peek with Escape pops history so Back does not reopen it", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-peek");

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument(),
    );

    // Because Escape popped (rather than pushed) the Index entry, Back must not resurrect it.
    await act(async () => {
      window.history.back();
    });
    await waitFor(() =>
      expect(screen.getByTestId("projects-index")).toBeInTheDocument(),
    );
    expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument();
  });

  it("closes an Add Project modal opened over the plain Index, and is a no-op with nothing open", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    await user.keyboard("{Control>}n{/Control}");
    expect(screen.getByRole("dialog", { name: /add project/i })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("dialog", { name: /add project/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("projects-index")).toBeInTheDocument();

    // Escape with nothing open neither throws nor navigates away from the Index.
    await user.keyboard("{Escape}");
    expect(screen.getByTestId("projects-index")).toBeInTheDocument();
  });
});

describe("announcements", () => {
  it("keeps two persistent visually-hidden live regions with the right politeness", async () => {
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    const polite = screen.getByTestId("live-polite");
    const assertive = screen.getByTestId("live-assertive");
    expect(polite).toHaveAttribute("aria-live", "polite");
    expect(polite).toHaveAttribute("role", "status");
    expect(assertive).toHaveAttribute("aria-live", "assertive");
    expect(assertive).toHaveAttribute("role", "alert");
  });

  it("routes assertive messages (errors) to the assertive region", () => {
    render(<LiveStatus polite="" assertive="Couldn't save" />);
    expect(screen.getByTestId("live-assertive")).toHaveTextContent("Couldn't save");
    expect(screen.getByTestId("live-polite")).toHaveTextContent("");
  });
});
