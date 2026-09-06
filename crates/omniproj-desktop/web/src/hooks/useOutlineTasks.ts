// Every edit to the outline goes through here.
//
// Two problems this solves that a bare `useMutation` per action does not:
//
//   1. Typing is faster than a round trip. Holding Enter queues several creates, and each
//      write is revision-checked, so firing them concurrently makes all but the first fail
//      with a conflict. Operations are therefore serialized, and each one reads the
//      revision returned by the previous one instead of the revision React last rendered.
//   2. Creating a step has to hand its id back, so the row that was just made can take
//      focus. The command returns the whole list, so the new id is the one that was not
//      there before.

import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api, AppError } from "../api";
import type { AdvanceProposal, ProjectId, Task, TaskList } from "../domain/project";
import type { MoveTarget } from "../domain/outline";
import { localizeError, useI18n } from "../i18n/I18nProvider";
import { queryKeys } from "../queryKeys";

export interface OutlineTasks {
  tasks: Task[];
  isLoading: boolean;
  /** Localized message for the last failed write, or "" when the last write succeeded. */
  error: string;
  /** Create a step and resolve with its id, or null if the write failed. */
  add: (text: string, placement?: MoveTarget) => Promise<string | null>;
  rename: (task: Task, text: string) => Promise<void>;
  setStatus: (task: Task, status: Task["status"]) => Promise<void>;
  setDetails: (task: Task, details: { due: string | null; note: string | null; tags: string[] }) => Promise<void>;
  move: (task: Task, target: MoveTarget) => Promise<void>;
  remove: (task: Task) => Promise<void>;
  /** Ask the Agent to break a step into candidates. Returns null if the call failed. */
  breakDown: (task: Task) => Promise<AdvanceProposal | null>;
  /** Adopt chosen candidates as sub-steps of `task`. */
  adopt: (task: Task, proposalId: string, texts: string[]) => Promise<void>;
}

export function useOutlineTasks(projectId: ProjectId): OutlineTasks {
  const { locale, t } = useI18n();
  const client = useQueryClient();
  const key = ["tasks", projectId] as const;
  const { data, isLoading } = useQuery({ queryKey: key, queryFn: () => api.getTasks(projectId) });
  const [error, setError] = useState("");

  const chain = useRef<Promise<unknown>>(Promise.resolve());
  const pending = useRef(0);
  const latest = useRef<TaskList | undefined>(undefined);
  // While writes are in flight the queued revision is ahead of the rendered one; adopting
  // `data` then would hand the next operation a revision the store has already moved past.
  if (pending.current === 0 && data) latest.current = data;

  const run = useCallback(
    <T,>(operation: (list: TaskList) => Promise<TaskList>, result: (list: TaskList, before: TaskList) => T, fallback: T): Promise<T> => {
      pending.current += 1;
      const next = chain.current.then(async (): Promise<T> => {
        const before = latest.current;
        if (!before) return fallback;
        try {
          const list = await operation(before);
          latest.current = list;
          client.setQueryData(key, list);
          void client.invalidateQueries({ queryKey: queryKeys.projectOverview(projectId) });
          void client.invalidateQueries({ queryKey: queryKeys.projectIndex });
          setError("");
          return result(list, before);
        } catch (cause) {
          setError(cause instanceof AppError ? localizeError(cause, locale) : t("task.conflict"));
          // The local revision is now untrustworthy; refetch before anything else runs.
          // `staleTime: 0` is not the default here by accident — without it `fetchQuery`
          // honours the client's staleTime and hands back the very data that just lost.
          const fresh = await client.fetchQuery({
            queryKey: key,
            queryFn: () => api.getTasks(projectId),
            staleTime: 0,
          });
          latest.current = fresh;
          return fallback;
        } finally {
          pending.current -= 1;
        }
      });
      chain.current = next.catch(() => undefined);
      return next;
    },
    // `key` is derived from projectId and stable for a given project.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [client, locale, projectId, t],
  );

  const add = useCallback(
    (text: string, placement?: MoveTarget) =>
      run(
        (list) =>
          api.addTask({
            project_id: projectId,
            expected_revision: list.revision,
            text: text.trim(),
            unclear: false,
            parent_id: placement?.parent_id ?? null,
            after_id: placement?.after_id ?? null,
          }),
        (list, before) => {
          const known = new Set(before.tasks.map((task) => task.id));
          return list.tasks.find((task) => !known.has(task.id))?.id ?? null;
        },
        null,
      ),
    [projectId, run],
  );

  const write = useCallback(
    (operation: (list: TaskList) => Promise<TaskList>) =>
      run(operation, () => undefined, undefined),
    [run],
  );

  return {
    tasks: data?.tasks ?? [],
    isLoading,
    error,
    add,
    rename: (task, text) =>
      write((list) =>
        api.updateTask({
          project_id: projectId,
          expected_revision: list.revision,
          id: task.id,
          text: text.trim(),
          status: task.status,
          due: task.due,
          note: task.note,
          tags: task.tags,
        }),
      ),
    setStatus: (task, status) =>
      write((list) =>
        api.updateTask({
          project_id: projectId,
          expected_revision: list.revision,
          id: task.id,
          status,
          due: task.due,
          note: task.note,
          tags: task.tags,
        }),
      ),
    setDetails: (task, details) =>
      write((list) =>
        api.updateTask({
          project_id: projectId,
          expected_revision: list.revision,
          id: task.id,
          status: task.status,
          due: details.due,
          note: details.note,
          tags: details.tags,
        }),
      ),
    move: (task, target) =>
      write((list) =>
        api.moveTask({
          project_id: projectId,
          expected_revision: list.revision,
          id: task.id,
          parent_id: target.parent_id,
          after_id: target.after_id,
        }),
      ),
    remove: (task) =>
      write((list) =>
        api.removeTask({
          project_id: projectId,
          expected_revision: list.revision,
          id: task.id,
        }),
      ),
    // Breaking a step down reads nothing and writes nothing: it is a proposal until the
    // user picks from it, so it does not go through the revision-checked write chain.
    breakDown: async (task) => {
      try {
        return await api.advanceTask({ project_id: projectId, id: task.id });
      } catch (cause) {
        setError(cause instanceof AppError ? localizeError(cause, locale) : t("task.advanceFailed"));
        return null;
      }
    },
    adopt: (task, proposalId, texts) =>
      write((list) =>
        api.adoptSubtasks({
          project_id: projectId,
          expected_revision: list.revision,
          proposal_id: proposalId,
          texts,
          // Adopted candidates are a decomposition of this step, so they land under it.
          parent_id: task.id,
        }),
      ),
  };
}
