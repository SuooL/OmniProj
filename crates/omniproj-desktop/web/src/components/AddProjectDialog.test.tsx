// Add Project + moved-source relink contract: the picker wrapper, every validation state,
// duplicate handling that never steals a source, failures, the success navigation, and relink.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invokeMock, openMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), openMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

import { AddProjectDialog } from "./AddProjectDialog";
import { chooseProjectDirectory } from "../platform/dialog";
import { SourceRecovery } from "./projects/SourceRecovery";
import { overview, projectSource } from "../test/fixtures";
import type { SourceValidation } from "../domain/project";

function LocationProbe() {
  const loc = useLocation();
  return <div data-testid="loc">{loc.pathname}</div>;
}

function renderDialog() {
  const onClose = vi.fn();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/projects"]}>
        <AddProjectDialog onClose={onClose} />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { onClose };
}

/** Route validate/register/relink through one command-aware invoke mock. */
function ipc(handlers: Record<string, (input: Record<string, unknown>) => unknown>) {
  invokeMock.mockImplementation(async (command: string, args?: { input?: Record<string, unknown> }) => {
    const h = handlers[command];
    if (h) return h(args?.input ?? {});
    throw new Error(`unexpected command ${command}`);
  });
}

const OK: SourceValidation = { state: "ok", location: "/repo", head: { kind: "attached", branch: "main" }, last_commit: null };

afterEach(() => {
  invokeMock.mockReset();
  openMock.mockReset();
});

describe("picker wrapper", () => {
  it("returns the chosen path, and null for cancel or an unexpected array", async () => {
    openMock.mockResolvedValueOnce("/Users/dev/repo");
    expect(await chooseProjectDirectory()).toBe("/Users/dev/repo");
    openMock.mockResolvedValueOnce(null);
    expect(await chooseProjectDirectory()).toBeNull();
    openMock.mockResolvedValueOnce(["/a", "/b"]);
    expect(await chooseProjectDirectory()).toBeNull();
  });
});

describe("validation states", () => {
  it("keeps Register disabled until a valid preview and never mutates the store", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/repo");
    ipc({ validate_project_source: () => OK });
    renderDialog();

    expect(screen.getByRole("button", { name: "Register" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));

    expect(await screen.findByTestId("valid-preview")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Register" })).toBeEnabled();
    // Validation is read-only: only validate was called, never a register.
    expect(invokeMock.mock.calls.every((c) => c[0] === "validate_project_source")).toBe(true);
  });

  it("shows a non-Git message and keeps Register disabled", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/plain");
    ipc({ validate_project_source: () => ({ state: "not_git_repository", location: "/plain" }) });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    expect(await screen.findByTestId("invalid-notice")).toHaveTextContent(/isn't a Git repository/i);
    expect(screen.getByRole("button", { name: "Register" })).toBeDisabled();
  });

  it("shows a bare-repository message", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/bare.git");
    ipc({ validate_project_source: () => ({ state: "bare_repository", location: "/bare.git" }) });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    expect(await screen.findByTestId("invalid-notice")).toHaveTextContent(/bare/i);
  });

  it("shows a missing-path message", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/gone");
    ipc({ validate_project_source: () => ({ state: "missing", location: "/gone" }) });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    expect(await screen.findByTestId("invalid-notice")).toHaveTextContent(/no longer exists/i);
  });

  it("shows an unreadable-path message", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/locked");
    ipc({ validate_project_source: () => ({ state: "unreadable", location: "/locked" }) });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    expect(await screen.findByTestId("invalid-notice")).toHaveTextContent(/can't be read/i);
  });

  it("does nothing when the picker is cancelled", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue(null);
    ipc({ validate_project_source: () => OK });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    expect(screen.queryByTestId("valid-preview")).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("duplicate", () => {
  it("offers Open existing project and never registers a second copy", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/repo");
    ipc({
      validate_project_source: () => ({
        state: "duplicate",
        location: "/repo",
        existing_project_id: "proj-existing",
        existing_name: "Existing",
      }),
    });
    const { onClose } = renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));

    const dup = within(await screen.findByTestId("duplicate-notice"));
    await user.click(dup.getByRole("button", { name: /open existing project/i }));

    expect(onClose).toHaveBeenCalled();
    expect(screen.getByTestId("loc")).toHaveTextContent("/projects/proj-existing/overview");
    expect(screen.getByTestId("loc")).not.toHaveAttribute("data-bg");
    expect(invokeMock.mock.calls.some((c) => c[0] === "register_project")).toBe(false);
  });
});

describe("failure and success", () => {
  it("retries a transient observation failure", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/repo");
    let calls = 0;
    ipc({
      validate_project_source: () => {
        calls += 1;
        return calls === 1
          ? { state: "observation_failed", location: "/repo", message: "git timed out" }
          : OK;
      },
    });
    renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    await user.click(await screen.findByRole("button", { name: /try again/i }));
    expect(await screen.findByTestId("valid-preview")).toBeInTheDocument();
  });

  it("surfaces a register failure and keeps the dialog open", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/repo");
    ipc({
      validate_project_source: () => OK,
      register_project: () => {
        throw { code: "duplicate_source", message: "already registered", retryable: false, state_applied: false };
      },
    });
    const { onClose } = renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    await screen.findByTestId("valid-preview");
    await user.click(screen.getByRole("button", { name: "Register" }));
    expect(await screen.findByTestId("add-project-error")).toHaveTextContent(/already registered/i);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("on success closes and navigates to the new setup Overview page", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/repo");
    ipc({
      validate_project_source: () => OK,
      register_project: (input) => overview({ project_id: input.location === "/repo" ? ("new-proj" as never) : ("x" as never), status: "setup" }),
    });
    const { onClose } = renderDialog();
    await user.click(screen.getByRole("button", { name: /choose directory/i }));
    await screen.findByTestId("valid-preview");
    await user.click(screen.getByRole("button", { name: "Register" }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(screen.getByTestId("loc")).toHaveTextContent("/projects/new-proj/overview");
    expect(screen.getByTestId("loc")).not.toHaveAttribute("data-bg");
  });
});

describe("relink from SourceRecovery", () => {
  function renderRecovery() {
    const ov = overview({ source: projectSource({ status: "missing", location: "/old", revision: 2 }) });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter initialEntries={["/projects/project-1/overview"]}>
          <SourceRecovery overview={ov} />
          <LocationProbe />
        </MemoryRouter>
      </QueryClientProvider>,
    );
  }

  it("relinks with explicit confirmation, the expected source revision, and old location", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/new");
    ipc({
      validate_project_source: () => ({ ...OK, location: "/new" }),
      relink_project_source: () => overview({ source: projectSource({ location: "/new", revision: 3 }) }),
    });
    renderRecovery();

    await user.click(screen.getByRole("button", { name: /choose new location/i }));
    await screen.findByTestId("relink-confirm");
    // Relink stays disabled until confirmed.
    expect(screen.getByRole("button", { name: "Relink source" })).toBeDisabled();
    await user.click(screen.getByLabelText("Confirm relink"));
    await user.click(screen.getByRole("button", { name: "Relink source" }));

    await waitFor(() => expect(invokeMock.mock.calls.some((c) => c[0] === "relink_project_source")).toBe(true));
    const call = invokeMock.mock.calls.find((c) => c[0] === "relink_project_source") as [string, { input: Record<string, unknown> }];
    expect(call[1].input).toMatchObject({
      project_id: "project-1",
      expected_source_revision: 2,
      expected_location: "/old",
      new_location: "/new",
    });
  });

  it("a duplicate new source offers Open existing and never relinks (never steals)", async () => {
    const user = userEvent.setup();
    openMock.mockResolvedValue("/new");
    ipc({
      validate_project_source: () => ({
        state: "duplicate",
        location: "/new",
        existing_project_id: "other-proj",
        existing_name: "Other",
      }),
    });
    renderRecovery();

    await user.click(screen.getByRole("button", { name: /choose new location/i }));
    const dup = within(await screen.findByTestId("relink-duplicate"));
    await user.click(dup.getByRole("button", { name: /open existing project/i }));

    expect(screen.getByTestId("loc")).toHaveTextContent("/projects/other-proj/overview");
    expect(invokeMock.mock.calls.some((c) => c[0] === "relink_project_source")).toBe(false);
  });
});
