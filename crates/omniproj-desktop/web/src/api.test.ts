import { afterEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { api } from "./api";
import { AppError } from "./domain/errors";
import { projectId, transitionId, workItemId } from "./domain/project";

const pid = projectId("project-1");
const wid = workItemId("work-1");
const tid = transitionId("transition-1");

afterEach(() => {
  invokeMock.mockReset();
});

describe("command names and the single snake_case input", () => {
  const cases: Array<{ name: string; run: () => Promise<unknown>; input: object }> = [
    {
      name: "get_project_overview",
      run: () => api.getProjectOverview(pid),
      input: { project_id: pid },
    },
    {
      name: "validate_project_source",
      run: () => api.validateProjectSource("/repo"),
      input: { location: "/repo" },
    },
    {
      name: "register_project",
      run: () => api.registerProject({ location: "/repo", name: "R" }),
      input: { location: "/repo", name: "R" },
    },
    {
      name: "relink_project_source",
      run: () =>
        api.relinkProjectSource({
          project_id: pid,
          expected_source_revision: 1,
          expected_location: "/a",
          new_location: "/b",
        }),
      input: {
        project_id: pid,
        expected_source_revision: 1,
        expected_location: "/a",
        new_location: "/b",
      },
    },
    {
      name: "refresh_projects",
      run: () => api.refreshProjects([pid]),
      input: { project_ids: [pid] },
    },
    {
      name: "complete_project_setup",
      run: () =>
        api.completeProjectSetup({
          project_id: pid,
          expected_revision: 0,
          objective: "o",
          desired_outcome: "d",
          first_commitment: "f",
        }),
      input: {
        project_id: pid,
        expected_revision: 0,
        objective: "o",
        desired_outcome: "d",
        first_commitment: "f",
      },
    },
    {
      name: "save_project_framing",
      run: () =>
        api.saveProjectFraming({
          project_id: pid,
          expected_revision: 1,
          objective: "o",
          desired_outcome: "d",
        }),
      input: {
        project_id: pid,
        expected_revision: 1,
        objective: "o",
        desired_outcome: "d",
      },
    },
    {
      name: "set_project_status",
      run: () =>
        api.setProjectStatus({
          project_id: pid,
          expected_revision: 1,
          status: "parked",
          reason: "later",
        }),
      input: {
        project_id: pid,
        expected_revision: 1,
        status: "parked",
        reason: "later",
      },
    },
    {
      name: "set_commitment",
      run: () => api.setCommitment({ project_id: pid, expected_revision: 1, text: "do it" }),
      input: { project_id: pid, expected_revision: 1, text: "do it" },
    },
    {
      name: "confirm_commitment",
      run: () =>
        api.confirmCommitment({ project_id: pid, expected_revision: 1, work_item_id: wid }),
      input: { project_id: pid, expected_revision: 1, work_item_id: wid },
    },
    {
      name: "complete_commitment",
      run: () =>
        api.completeCommitment({ project_id: pid, expected_revision: 1, work_item_id: wid }),
      input: { project_id: pid, expected_revision: 1, work_item_id: wid },
    },
    {
      name: "replace_commitment",
      run: () =>
        api.replaceCommitment({
          project_id: pid,
          expected_revision: 1,
          previous_work_item_id: wid,
          text: "new",
          reason: "changed my mind",
        }),
      input: {
        project_id: pid,
        expected_revision: 1,
        previous_work_item_id: wid,
        text: "new",
        reason: "changed my mind",
      },
    },
    {
      name: "clear_commitment",
      run: () =>
        api.clearCommitment({ project_id: pid, expected_revision: 1, work_item_id: wid }),
      input: { project_id: pid, expected_revision: 1, work_item_id: wid },
    },
    {
      name: "undo_commitment_transition",
      run: () =>
        api.undoCommitmentTransition({
          project_id: pid,
          expected_revision: 2,
          transition_id: tid,
        }),
      input: { project_id: pid, expected_revision: 2, transition_id: tid },
    },
  ];

  it.each(cases)("$name sends exactly one top-level `input`", async ({ name, run, input }) => {
    invokeMock.mockResolvedValue(undefined);
    await run();
    expect(invokeMock).toHaveBeenCalledWith(name, { input });
    const [, arg] = invokeMock.mock.calls[0];
    expect(Object.keys(arg as object)).toEqual(["input"]);
  });

  it("list_project_index takes no arguments", async () => {
    invokeMock.mockResolvedValue({ projects: [], review_policy: {} });
    await api.listProjectIndex();
    expect(invokeMock).toHaveBeenCalledWith("list_project_index");
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });
});

describe("error classification", () => {
  it("turns a structured rejection into a typed AppError", async () => {
    invokeMock.mockRejectedValue({
      code: "revision_conflict",
      message: "revision conflict: expected 1, found 2",
      retryable: false,
      state_applied: false,
    });
    const error = await api
      .setCommitment({ project_id: pid, expected_revision: 1, text: "x" })
      .catch((e) => e);
    expect(error).toBeInstanceOf(AppError);
    expect(error.code).toBe("revision_conflict");
    expect(error.recovery).toBe("refetch");
  });

  it("exposes durable_revision and marks audit_commit_failed for refetch, not retry", async () => {
    invokeMock.mockRejectedValue({
      code: "audit_commit_failed",
      message: "saved as revision 5 but audit failed",
      retryable: false,
      state_applied: true,
      durable_revision: 5,
    });
    const error = await api
      .confirmCommitment({ project_id: pid, expected_revision: 4, work_item_id: wid })
      .catch((e) => e);
    expect(error).toBeInstanceOf(AppError);
    expect(error.stateApplied).toBe(true);
    expect(error.durableRevision).toBe(5);
    expect(error.recovery).toBe("refetch");
    expect(error.retryable).toBe(false);
  });

  it("carries existing_project_id on a duplicate_source error", async () => {
    invokeMock.mockRejectedValue({
      code: "duplicate_source",
      message: "already registered",
      retryable: false,
      state_applied: false,
      existing_project_id: "project-2",
    });
    const error = await api
      .registerProject({ location: "/repo", name: "R" })
      .catch((e) => e);
    expect(error.code).toBe("duplicate_source");
    expect(error.existingProjectId).toBe("project-2");
  });

  it("flattens an unknown rejection to a safe generic message", async () => {
    invokeMock.mockRejectedValue(new Error("stack trace: at foo (bar.rs:1)"));
    const error = await api.listProjectIndex().catch((e) => e);
    expect(error).toBeInstanceOf(AppError);
    expect(error.code).toBe("unknown");
    expect(error.recovery).toBe("none");
    expect(error.message).not.toContain("stack trace");
  });

  it("marks a transient store_write_failed as retryable", async () => {
    invokeMock.mockRejectedValue({
      code: "store_write_failed",
      message: "disk full",
      retryable: true,
      state_applied: false,
    });
    const error = await api
      .setCommitment({ project_id: pid, expected_revision: 1, text: "x" })
      .catch((e) => e);
    expect(error.recovery).toBe("retry");
  });
});
