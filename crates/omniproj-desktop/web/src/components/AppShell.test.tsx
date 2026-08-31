// AppShell keyboard, navigation, and announcement contract. The shell is exercised through
// <App/> so the router and the shortcuts run exactly as they ship.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, startDraggingMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  startDraggingMock: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: startDraggingMock }),
}));

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
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 1024 });
  startDraggingMock.mockResolvedValue(undefined);
  invokeMock.mockImplementation(async (command: string, args?: { input?: { project_id?: string } }) => {
    if (command === "get_project_overview") {
      return overview({ project_id: (args?.input?.project_id ?? "project-1") as never });
    }
    if (command === "refresh_projects") return [];
    return { projects: [], review_policy: reviewPolicy };
  });
  window.sessionStorage.clear();
  window.localStorage.clear();
  window.localStorage.setItem("omniproj.locale", "en");
  window.history.replaceState(null, "", "/");
});

afterEach(() => {
  invokeMock.mockReset();
  startDraggingMock.mockClear();
});

describe("primary navigation", () => {
  it("exposes Projects as the sidebar navigation group", async () => {
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");
    const nav = screen.getByRole("navigation", { name: /primary/i });
    expect(within(nav).getByText("Projects")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /settings|attention|agents?/i }),
    ).not.toBeInTheDocument();
  });
});

describe("desktop sidebar", () => {
  it("marks the current project without advertising unavailable subpages", async () => {
    renderAppAt("/projects/p1/overview", [indexItem({ project_id: "p1" as never, name: "Alpha" })]);
    await screen.findByTestId("overview-page");
    expect(screen.getByRole("button", { name: "Alpha" })).toHaveAttribute("data-active", "true");
    expect(screen.queryByText("Commitment")).not.toBeInTheDocument();
    expect(screen.queryByText("Activity")).not.toBeInTheDocument();
  });

  it("can be hidden and restored from the main toolbar", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));
    expect(document.querySelector(".app-shell")).toHaveAttribute("data-sidebar-open", "false");

    await user.click(screen.getByRole("button", { name: "Show sidebar" }));
    expect(document.querySelector(".app-shell")).toHaveAttribute("data-sidebar-open", "true");
  });

  it("filters the project tree on detail routes", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects/p1/overview", [
      indexItem({ project_id: "p1" as never, name: "Alpha" }),
      indexItem({ project_id: "p2" as never, name: "Beta" }),
    ]);
    await screen.findByTestId("overview-page");
    await user.type(screen.getByLabelText(/filter projects/i), "beta");
    expect(screen.queryByRole("button", { name: "Alpha" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Beta" })).toBeInTheDocument();
  });

  it("keeps archived projects discoverable in a separate tree section", async () => {
    renderAppAt("/projects", [indexItem({ name: "Old study", status: "archived" })]);
    await screen.findByTestId("projects-index");
    const nav = screen.getByRole("navigation", { name: /primary/i });
    expect(within(nav).getByText("Archived")).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Old study" })).toBeInTheDocument();
  });

  it("closes the narrow drawer after choosing a project", async () => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 640 });
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await screen.findByTestId("projects-index");
    expect(document.querySelector(".app-shell")).toHaveAttribute("data-sidebar-open", "false");
    await user.click(screen.getByRole("button", { name: "Show sidebar" }));
    await user.click(screen.getByRole("button", { name: "Alpha" }));
    expect(document.querySelector(".app-shell")).toHaveAttribute("data-sidebar-open", "false");
  });
});

describe("native window chrome", () => {
  it("starts native dragging from the sidebar chrome", async () => {
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    const toolbar = document.querySelector<HTMLElement>(".app-shell__sidebar-chrome");
    expect(toolbar).not.toBeNull();
    await userEvent.setup().click(toolbar!);

    expect(startDraggingMock).toHaveBeenCalledOnce();
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
    renderAppAt("/projects", [indexItem({ project_id: "p1" as never, name: "Alpha" })]);
    await screen.findByTestId("projects-index");
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("refresh_projects", { input: { project_ids: ["p1"] } }));
    invokeMock.mockClear();

    // Unfocused window: neither the reload default nor a refresh.
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const unfocused = dispatchChord("r");
    expect(unfocused.defaultPrevented).toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();

    // Focused window: default reload is prevented and a refresh is announced.
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const focused = dispatchChord("r");
    expect(focused.defaultPrevented).toBe(true);
    expect(screen.getByTestId("live-polite")).toHaveTextContent(/refreshing/i);
    expect(invokeMock).toHaveBeenCalledWith("refresh_projects", { input: { project_ids: ["p1"] } });
  });

  it("Cmd/Ctrl+R still refreshes while a text input is focused", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ project_id: "p1" as never, name: "Alpha" })]);
    await screen.findByTestId("projects-index");
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("refresh_projects", { input: { project_ids: ["p1"] } }));
    invokeMock.mockClear();
    await user.click(screen.getByLabelText(/filter projects/i));

    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    dispatchChord("r");
    expect(screen.getByTestId("live-polite")).toHaveTextContent(/refreshing/i);
  });
});

describe("stacked Escape", () => {
  it("closes the Add Project modal while a project page remains open", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-page");

    await user.keyboard("{Control>}n{/Control}");
    expect(screen.getByRole("dialog", { name: /add project/i })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("dialog", { name: /add project/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("overview-page")).toBeInTheDocument();
  });
});

describe("visible desktop navigation", () => {
  it("returns to Projects with the visible back control", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-page");

    await user.click(screen.getByRole("button", { name: /back to projects/i }));
    await waitFor(() =>
      expect(screen.queryByTestId("overview-page")).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId("projects-index")).toBeInTheDocument();
  });

  it("closes the Add Project sheet with a visible control", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects");
    await screen.findByTestId("projects-index");

    await user.click(screen.getAllByRole("button", { name: "Add Project" })[0]);
    await user.click(screen.getByRole("button", { name: /close add project/i }));

    expect(screen.queryByRole("dialog", { name: /add project/i })).not.toBeInTheDocument();
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

  it("Escape on a project page does not unexpectedly navigate", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-page");

    await user.keyboard("{Escape}");
    expect(screen.getByTestId("overview-page")).toBeInTheDocument();
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
