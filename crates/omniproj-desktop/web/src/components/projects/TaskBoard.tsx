// The project's one list of work. It owns the data and every mutation; the head card, the
// composer, the rows and the time view are presentational and receive callbacks.
//
// Two views, not three. The board view was a status-column layout of the same rows the list
// already shows in a deterministic order — a second way to look at one list, with fewer
// controls on each card than the list itself offered.

import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api, AppError } from "../../api";
import type { AdvanceProposal, ProjectOverview, Task, TaskList } from "../../domain/project";
import { useOverviewMutation, type MutationOutcome } from "../../hooks/useOverviewMutation";
import {
  localToday,
  parseTagsInput,
  TASK_VIEW_STORAGE_KEY,
  type TaskDraft,
  type TaskViewMode,
} from "../../domain/taskBoardModel";
import { FilterChip } from "../semantic/FilterChip";
import { localizeError, useI18n } from "../../i18n/I18nProvider";
import { queryKeys } from "../../queryKeys";
import { NowDoingCard } from "./tasks/NowDoingCard";
import { TaskComposer } from "./tasks/TaskComposer";
import { TaskListRow } from "./tasks/TaskListRow";
import { TaskTimeView } from "./tasks/TaskTimeView";

interface TaskBoardProps {
  overview: ProjectOverview;
}

interface ProposalDraft extends AdvanceProposal {
  selected: boolean[];
}

function storedViewMode(): TaskViewMode {
  if (typeof window === "undefined") return "list";
  try {
    return window.localStorage.getItem(TASK_VIEW_STORAGE_KEY) === "time" ? "time" : "list";
  } catch {
    return "list";
  }
}

export function TaskBoard({ overview }: TaskBoardProps) {
  const projectId = overview.project_id;
  const commitment = overview.current_commitment;
  const { locale, t } = useI18n();
  const client = useQueryClient();
  const commitmentMutation = useOverviewMutation();
  const [text, setText] = useState("");
  const [unclear, setUnclear] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, TaskDraft>>({});
  const [proposal, setProposal] = useState<ProposalDraft | null>(null);
  const [message, setMessage] = useState("");
  const [tagFilter, setTagFilter] = useState<string[]>([]);
  const [viewMode, setViewMode] = useState<TaskViewMode>(storedViewMode);
  /** Which task's edit panel is open. A row is read-only until the user opens it. */
  const [editingId, setEditingId] = useState<string | null>(null);
  const [commitmentOutcome, setCommitmentOutcome] = useState<MutationOutcome | null>(null);
  const [retryCommitment, setRetryCommitment] = useState<(() => void) | null>(null);
  const key = ["tasks", projectId] as const;
  const { data, isLoading } = useQuery({ queryKey: key, queryFn: () => api.getTasks(projectId) });
  const tasks = data?.tasks ?? [];
  const today = localToday();

  function switchView(mode: TaskViewMode) {
    setViewMode(mode);
    try {
      window.localStorage.setItem(TASK_VIEW_STORAGE_KEY, mode);
    } catch {
      // Preference persistence is best-effort.
    }
  }

  // Every tag in the project, first-seen order, for the filter row and entry autocomplete.
  const allTags = useMemo(() => {
    const seen = new Set<string>();
    const ordered: string[] = [];
    for (const task of tasks) for (const tag of task.tags) {
      const lower = tag.toLowerCase();
      if (!seen.has(lower)) { seen.add(lower); ordered.push(tag); }
    }
    return ordered;
  }, [tasks]);

  // AND semantics: a task must carry every selected tag (case-insensitive).
  const visible = tagFilter.length === 0 ? tasks : tasks.filter((task) => {
    const carried = new Set(task.tags.map((tag) => tag.toLowerCase()));
    return tagFilter.every((tag) => carried.has(tag.toLowerCase()));
  });

  function accept(next: TaskList) {
    client.setQueryData(key, next);
    void client.invalidateQueries({ queryKey: queryKeys.projectOverview(projectId) });
    void client.invalidateQueries({ queryKey: queryKeys.projectIndex });
    setMessage("");
  }

  /** Never surface a raw backend string; the typed, localized copy is the user-facing text. */
  async function reportFailure(error: unknown) {
    setMessage(error instanceof AppError ? localizeError(error, locale) : t("task.conflict"));
    await client.invalidateQueries({ queryKey: key });
  }

  async function add() {
    if (!text.trim() || !data) return;
    try {
      accept(await api.addTask({ project_id: projectId, expected_revision: data.revision, text: text.trim(), unclear }));
      setText("");
      setUnclear(false);
    } catch (error) {
      await reportFailure(error);
    }
  }

  function draft(task: Task): TaskDraft {
    return drafts[task.id] ?? { status: task.status, due: task.due ?? "", note: task.note ?? "", tags: task.tags.join(", ") };
  }

  /** True when the open draft differs from what is stored — nothing is sent otherwise. */
  function isDirty(task: Task, value: TaskDraft) {
    return value.status !== task.status
      || value.due !== (task.due ?? "")
      || value.note !== (task.note ?? "")
      || parseTagsInput(value.tags).join("\0") !== task.tags.join("\0");
  }

  /** Persist a task. `override` lets one decisive control (the status select) send its new
   * value immediately instead of waiting for the async draft state to settle. */
  async function update(task: Task, override?: Partial<TaskDraft>) {
    if (!data) return;
    const value = { ...draft(task), ...override };
    if (!isDirty(task, value)) return;
    try {
      accept(await api.updateTask({ project_id: projectId, expected_revision: data.revision, id: task.id, status: value.status, due: value.due || null, note: value.note || null, tags: parseTagsInput(value.tags) }));
      setDrafts((all) => { const next = { ...all }; delete next[task.id]; return next; });
    } catch (error) {
      await reportFailure(error);
    }
  }

  // Blur alone is not a safe autosave trigger: on macOS a click does not move keyboard
  // focus to a button, so closing one row by opening another would never fire `blur` and the
  // edit would be silently dropped. Whenever the open row changes, the row that just closed
  // is persisted explicitly; `update` is a no-op when nothing changed.
  const previousEditingId = useRef<string | null>(null);
  useEffect(() => {
    const previous = previousEditingId.current;
    previousEditingId.current = editingId;
    if (!previous || previous === editingId) return;
    const task = tasksRef.current.find((candidate) => candidate.id === previous);
    if (task) void update(task);
  }, [editingId]);

  // The latest tasks, so the effect above can resolve a row without re-running on every fetch.
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;

  // The time view moves only the status; due/note/tags resend the task's persisted values.
  async function move(task: Task, status: string) {
    if (!data || status === task.status) return;
    try {
      accept(await api.updateTask({ project_id: projectId, expected_revision: data.revision, id: task.id, status, due: task.due, note: task.note, tags: task.tags }));
    } catch (error) {
      await reportFailure(error);
    }
  }

  async function advance(task: Task) {
    setMessage(t("task.advancing"));
    try {
      const next = await api.advanceTask({ project_id: projectId, id: task.id });
      setProposal({ ...next, selected: next.candidates.map(() => false) });
      setMessage("");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t("task.advanceFailed"));
    }
  }

  async function adopt() {
    if (!data || !proposal) return;
    const texts = proposal.candidates.filter((_, index) => proposal.selected[index]);
    try {
      accept(await api.adoptSubtasks({ project_id: projectId, expected_revision: data.revision, proposal_id: proposal.proposal_id, texts }));
      setProposal(null);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t("task.conflict"));
    }
  }

  async function promote(task: Task) {
    if (!data) return;
    try {
      await api.promoteTaskToCommitment({ project_id: projectId, task_id: task.id, expected_task_revision: data.revision, expected_project_revision: Number(data.revision) });
    } catch (error) {
      // Without this the rejection was unhandled and the click looked like it did nothing.
      await reportFailure(error);
      return;
    }
    await Promise.all([
      client.invalidateQueries({ queryKey: key }),
      client.invalidateQueries({ queryKey: queryKeys.projectOverview(projectId) }),
      client.invalidateQueries({ queryKey: queryKeys.projectIndex }),
    ]);
  }

  async function remove(task: Task) {
    if (!data) return;
    try {
      accept(await api.removeTask({ project_id: projectId, expected_revision: data.revision, id: task.id }));
    } catch (error) {
      // Without this the rejection was unhandled and the click looked like it did nothing.
      await reportFailure(error);
    }
  }

  // The commitment lifecycle runs from the list itself now. Every one of these also moves the
  // task's stored status, so the task cache is refreshed alongside the overview.
  async function runCommitment(action: () => Promise<ProjectOverview>, success: string) {
    setRetryCommitment(() => () => void runCommitment(action, success));
    const result = await commitmentMutation.run(projectId, action, success);
    setCommitmentOutcome(result);
    await client.invalidateQueries({ queryKey: key });
  }

  const rev = overview.revision;
  const commitmentAction = (
    call: () => Promise<ProjectOverview>,
    success: string,
  ) => () => void runCommitment(call, success);

  // Undo is deliberately not offered for a `set`: undoing one abandons the item, which would
  // silently delete a task that existed before it was marked. Switching away covers that case.
  const canUndo = overview.undoable_transition_id !== null && overview.last_transition?.type !== "set";

  return <section className="op-section" aria-labelledby="tasks-heading" data-testid="task-board">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("task.kicker")}</p><h3 id="tasks-heading">{t("task.title")}</h3></div><span className="op-section__count">{tasks.length}</span></div>
    <p className="op-muted">{t("task.relationship")}</p>

    {/* The single next step lives at the head of the same list it belongs to. */}
    <NowDoingCard
      commitment={commitment}
      canUndo={canUndo}
      pending={commitmentMutation.pending}
      outcome={commitmentOutcome}
      onConfirm={commitmentAction(() => api.confirmCommitment({ project_id: projectId, expected_revision: rev, work_item_id: commitment!.work_item_id }), t("commitment.confirmSuccess"))}
      onComplete={commitmentAction(() => api.completeCommitment({ project_id: projectId, expected_revision: rev, work_item_id: commitment!.work_item_id }), t("commitment.completeSuccess"))}
      onSwitchAway={commitmentAction(() => api.clearCommitment({ project_id: projectId, expected_revision: rev, work_item_id: commitment!.work_item_id }), t("commitment.clearSuccess"))}
      onUndo={commitmentAction(() => api.undoCommitmentTransition({ project_id: projectId, expected_revision: rev, transition_id: overview.undoable_transition_id! }), t("commitment.undoSuccess"))}
      onRetry={() => retryCommitment?.()}
    />

    <TaskComposer
      text={text}
      unclear={unclear}
      disabled={!data}
      onTextChange={setText}
      onUnclearChange={setUnclear}
      onSubmit={() => void add()}
    />

    <div className="op-task-tagfilter" role="group" aria-label={t("board.viewLabel")}>
      <span className="op-muted">{t("board.viewLabel")}</span>
      <FilterChip label={t("board.viewList")} pressed={viewMode === "list"} onClick={() => switchView("list")} />
      <FilterChip label={t("board.viewTime")} pressed={viewMode === "time"} onClick={() => switchView("time")} />
    </div>

    {allTags.length > 0 && <div className="op-task-tagfilter" role="group" aria-label={t("task.filterTags")}><span className="op-muted">{t("task.filterTags")}</span>{allTags.map((tag) => <FilterChip key={tag} label={tag} pressed={tagFilter.some((selected) => selected.toLowerCase() === tag.toLowerCase())} onClick={() => setTagFilter((current) => current.some((selected) => selected.toLowerCase() === tag.toLowerCase()) ? current.filter((selected) => selected.toLowerCase() !== tag.toLowerCase()) : [...current, tag])} />)}{tagFilter.length > 0 && <button className="op-button op-button--ghost" type="button" onClick={() => setTagFilter([])}>{t("task.tagFilterClear")}</button>}</div>}

    {message && <p role="status" className="op-error">{message}</p>}

    {proposal && <div className="op-proposal" role="region" aria-label={t("task.proposal")}><p><strong>{t("task.advanceReady")}</strong></p>{proposal.candidates.map((candidate, index) => <label key={`${proposal.proposal_id}-${index}`}><input type="checkbox" checked={proposal.selected[index]} onChange={(event) => setProposal({ ...proposal, selected: proposal.selected.map((value, itemIndex) => itemIndex === index ? event.target.checked : value) })} />{candidate}</label>)}<div className="op-task-actions"><button className="op-button op-button--primary" type="button" disabled={!proposal.selected.some(Boolean)} onClick={() => void adopt()}>{t("task.adoptSelected")}</button><button className="op-button op-button--ghost" type="button" onClick={() => setProposal(null)}>{t("common.cancel")}</button></div></div>}

    {isLoading ? <p className="op-muted">{t("task.loading")}</p>
      : tasks.length === 0 ? <p className="op-muted">{t("task.empty")}</p>
        : viewMode === "time" ? <TaskTimeView tasks={visible} today={today} onMove={(task, status) => void move(task, status)} />
          : <ul className="op-task-list">
              {visible.map((task) => (
                <TaskListRow
                  key={task.id}
                  task={task}
                  draft={draft(task)}
                  open={editingId === task.id}
                  today={today}
                  vocabulary={allTags}
                  canMarkNowDoing={commitment === null}
                  onToggle={() => setEditingId(editingId === task.id ? null : task.id)}
                  onDraftChange={(value) => setDrafts((all) => ({ ...all, [task.id]: value }))}
                  onStatusChange={(status) => {
                    setDrafts((all) => ({ ...all, [task.id]: { ...draft(task), status } }));
                    // Status is a single decisive change: commit it immediately.
                    void update(task, { status });
                  }}
                  onPanelBlur={(event) => {
                    // Autosave when focus leaves the panel entirely.
                    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
                    void update(task);
                  }}
                  onMarkNowDoing={() => void promote(task)}
                  onAdvance={() => void advance(task)}
                  onRemove={() => void remove(task)}
                />
              ))}
            </ul>}
  </section>;
}
