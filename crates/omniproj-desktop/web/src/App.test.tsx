// Route behavior contract for the canonical desktop AppShell: URL shape, full-page project
// navigation, restart restoration, and deep-link precedence.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { App } from "./App";
import { projectId, type ProjectIndexItem } from "./domain/project";
import { queryKeys } from "./queryKeys";
import { indexItem, indexResponse, overview, reviewPolicy } from "./test/fixtures";

// Command-aware IPC mock: the index is seeded per-test via setQueryData; the Overview is
// fetched by the full page, so echo a valid Overview for the requested id.
function mockIpc() {
  invokeMock.mockImplementation(async (command: string, args?: { input?: { project_id?: string } }) => {
    if (command === "get_project_overview") {
      return overview({ project_id: (args?.input?.project_id ?? "project-1") as never });
    }
    if (command === "refresh_projects") return [];
    return { projects: [], review_policy: reviewPolicy };
  });
}

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

function currentUrl(): string {
  return window.location.pathname + window.location.search;
}

beforeEach(() => {
  mockIpc();
  window.sessionStorage.clear();
  window.localStorage.clear();
  window.localStorage.setItem("omniproj.locale", "en");
  window.history.replaceState(null, "", "/");
});

afterEach(() => {
  invokeMock.mockReset();
});

describe("canonical routes", () => {
  it("opens in Chinese by default and can switch to English", async () => {
    window.localStorage.removeItem("omniproj.locale");
    const user = userEvent.setup();
    renderAppAt("/projects");

    expect(await screen.findByRole("heading", { name: "项目" })).toBeInTheDocument();
    const language = screen.getByRole("combobox", { name: "界面语言" });
    expect(language).toHaveValue("zh-CN");

    await user.selectOptions(language, "en");
    expect(screen.getByRole("heading", { name: "Projects" })).toBeInTheDocument();
    expect(window.localStorage.getItem("omniproj.locale")).toBe("en");
  });

  it("redirects / to /projects", async () => {
    renderAppAt("/");
    expect(await screen.findByTestId("projects-index")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/projects");
  });

  it("replaces the bare project path with the canonical Overview", async () => {
    renderAppAt("/projects/p1");
    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/projects/p1/overview");
  });

  it("renders a direct Overview route as a full page (not a Peek)", async () => {
    renderAppAt("/projects/p1/overview");
    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(screen.queryByTestId("overview-peek")).not.toBeInTheDocument();
    expect(screen.queryByTestId("projects-index")).not.toBeInTheDocument();
  });

  it("shows Back to Projects on an unknown route", async () => {
    renderAppAt("/does-not-exist");
    expect(await screen.findByTestId("not-found")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: /back to projects/i }),
    ).toBeInTheDocument();
  });
});

describe("desktop project navigation", () => {
  it("opens an Index-origin project in the main content surface", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);

    const index = await screen.findByTestId("projects-index");
    await user.click(within(index).getByRole("link", { name: /^Alpha\b/ }));

    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(screen.queryByTestId("projects-index")).not.toBeInTheDocument();
    expect(window.location.pathname).toBe("/projects/project-1/overview");
  });

  it("shows the selected project as active in the sidebar", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Alpha" })).toHaveAttribute("data-active", "true");
  });

  it("Back and Forward restore the prior screen", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-page");

    await act(async () => {
      window.history.back();
    });
    await waitFor(() =>
      expect(screen.queryByTestId("overview-page")).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId("projects-index")).toBeInTheDocument();

    await act(async () => {
      window.history.forward();
    });
    await waitFor(() =>
      expect(screen.getByTestId("overview-page")).toBeInTheDocument(),
    );
  });

  it("round-trips a project id with special characters through link and route param", async () => {
    const user = userEvent.setup();
    const weirdId = "weird/id %#";
    renderAppAt("/projects", [
      indexItem({ project_id: projectId(weirdId), name: "Weird" }),
    ]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Weird\b/,
      }),
    );

    await screen.findByTestId("overview-page");
    // The id round-trips decoded through the route param into the IPC call.
    expect(invokeMock).toHaveBeenCalledWith("get_project_overview", {
      input: { project_id: weirdId },
    });

    expect(window.location.pathname).toBe(
      `/projects/${encodeURIComponent(weirdId)}/overview`,
    );
  });

  it("selects a project in the sidebar after opening it", async () => {
    const user = userEvent.setup();
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    await user.click(
      within(await screen.findByTestId("projects-index")).getByRole("link", {
        name: /^Alpha\b/,
      }),
    );
    await screen.findByTestId("overview-page");
    expect(screen.getByRole("button", { name: "Alpha" })).toHaveAttribute("data-active", "true");
  });

  it("restores the Index scroller and row focus after returning", async () => {
    window.sessionStorage.setItem(
      "omniproj.nav.indexView",
      JSON.stringify({ scrollY: 96, focusId: "project-1" }),
    );
    renderAppAt("/projects", [indexItem({ name: "Alpha" })]);
    const row = within(await screen.findByTestId("projects-index")).getByRole("link", { name: /^Alpha\b/ });
    await waitFor(() => expect(row).toHaveFocus());
    expect(document.querySelector<HTMLElement>(".app-shell__content")?.scrollTop).toBe(96);
  });
});

describe("filter/sort in search params", () => {
  it("reflects q and sort from the canonical search params", async () => {
    renderAppAt("/projects?q=alpha&sort=name", [indexItem({ name: "Alpha" })]);
    await screen.findByTestId("projects-index");
    expect(screen.getByLabelText(/filter projects/i)).toHaveValue("alpha");
    expect(screen.getByRole("combobox", { name: /attention order/i })).toHaveValue(
      "name",
    );
  });
});

describe("restart restoration and deep-link precedence", () => {
  it("restores the last canonical pathname+search when restarting at /", async () => {
    window.localStorage.setItem(
      "omniproj.nav.canonical",
      "/projects?q=beta",
    );
    renderAppAt("/");
    await screen.findByTestId("projects-index");
    expect(currentUrl()).toBe("/projects?q=beta");
    expect(screen.getByLabelText(/filter projects/i)).toHaveValue("beta");
  });

  it("lets an explicit non-root deep link win over saved session state", async () => {
    window.localStorage.setItem(
      "omniproj.nav.canonical",
      "/projects?q=beta",
    );
    renderAppAt("/projects/p9/overview");
    expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/projects/p9/overview");
    expect(invokeMock).toHaveBeenCalledWith("get_project_overview", {
      input: { project_id: "p9" },
    });
  });
});
