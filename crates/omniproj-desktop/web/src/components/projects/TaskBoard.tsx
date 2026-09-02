import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../../api";
import type { AdvanceProposal, ProjectId, Task, TaskList } from "../../domain/project";
import {
  boardColumns,
  dueSignal,
  localToday,
  timeGroups,
  DONE_PREVIEW_COUNT,
  TASK_VIEW_STORAGE_KEY,
  type TaskViewMode,
  type TimeGroupKey,
} from "../../domain/taskBoardModel";
import { FilterChip } from "../semantic/FilterChip";
import { toneStyle } from "../semantic/tone";
import { useI18n } from "../../i18n/I18nProvider";
import { queryKeys } from "../../queryKeys";

interface TaskBoardProps {
  projectId: ProjectId;
  hasCurrentCommitment: boolean;
}

interface ProposalDraft extends AdvanceProposal {
  selected: boolean[];
}

/** Split a user-entered tag string on comma variants; trimming and dedupe happen in core. */
export function parseTagsInput(value: string): string[] {
  return value.split(/[,，、]/).map((tag) => tag.trim()).filter((tag) => tag.length > 0);
}

function storedViewMode(): TaskViewMode {
  if (typeof window === "undefined") return "list";
  try {
    const stored = window.localStorage.getItem(TASK_VIEW_STORAGE_KEY);
    return stored === "board" || stored === "time" ? stored : "list";
  } catch {
    return "list";
  }
}

export function TaskBoard({ projectId, hasCurrentCommitment }: TaskBoardProps) {
  const { t } = useI18n();
  const client = useQueryClient();
  const [text, setText] = useState("");
  const [unclear, setUnclear] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, { status: string; due: string; note: string; tags: string }>>({});
  const [proposal, setProposal] = useState<ProposalDraft | null>(null);
  const [message, setMessage] = useState("");
  const [tagFilter, setTagFilter] = useState<string[]>([]);
  const [viewMode, setViewMode] = useState<TaskViewMode>(storedViewMode);
  const [doneExpanded, setDoneExpanded] = useState(false);
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

  async function add() {
    if (!text.trim() || !data) return;
    try {
      accept(await api.addTask({ project_id: projectId, expected_revision: data.revision, text: text.trim(), unclear }));
      setText("");
      setUnclear(false);
    } catch {
      setMessage(t("task.conflict"));
      await client.invalidateQueries({ queryKey: key });
    }
  }

  function draft(task: Task) {
    return drafts[task.id] ?? { status: task.status, due: task.due ?? "", note: task.note ?? "", tags: task.tags.join(", ") };
  }

  async function update(task: Task) {
    if (!data) return;
    const value = draft(task);
    try {
      accept(await api.updateTask({ project_id: projectId, expected_revision: data.revision, id: task.id, status: value.status, due: value.due || null, note: value.note || null, tags: parseTagsInput(value.tags) }));
    } catch (error) {
      setMessage(error instanceof Error && error.message ? error.message : t("task.conflict"));
      await client.invalidateQueries({ queryKey: key });
    }
  }

  // Board move replaces only the status; due/note/tags resend the task's persisted values.
  async function move(task: Task, status: string) {
    if (!data || status === task.status) return;
    try {
      accept(await api.updateTask({ project_id: projectId, expected_revision: data.revision, id: task.id, status, due: task.due, note: task.note, tags: task.tags }));
    } catch (error) {
      setMessage(error instanceof Error && error.message ? error.message : t("task.conflict"));
      await client.invalidateQueries({ queryKey: key });
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
    await api.promoteTaskToCommitment({ project_id: projectId, task_id: task.id, expected_task_revision: data.revision, expected_project_revision: Number(data.revision) });
    await Promise.all([
      client.invalidateQueries({ queryKey: key }),
      client.invalidateQueries({ queryKey: queryKeys.projectOverview(projectId) }),
      client.invalidateQueries({ queryKey: queryKeys.projectIndex }),
    ]);
  }

  async function remove(task: Task) {
    if (!data) return;
    accept(await api.removeTask({ project_id: projectId, expected_revision: data.revision, id: task.id }));
  }

  // Shared card renderer for the board and time views: due signal, tags, commitment
  // marker, and the keyboard-accessible move control (locked for commitment-bound items).
  const card = (task: Task) => {
    const locked = task.linked_work_item_id !== null;
    const signal = dueSignal(task.due, today);
    return <li key={task.id} className="op-board-card">
      <span className={task.unclear ? "op-task-unclear" : ""}>{task.unclear ? "? " : ""}{task.text}</span>
      <span className="op-board-card__meta">
        {task.is_current_commitment && <span className="op-task-tag">{t("task.currentCommitment")}</span>}
        {task.tags.map((tag) => <span key={tag} className="op-task-tag">{tag}</span>)}
        {signal.kind === "overdue" && <span className="op-badge" style={toneStyle("danger")}>{t("board.overdue", { days: signal.days })}</span>}
        {signal.kind === "soon" && <span className="op-badge" style={toneStyle("warning")}>{signal.days === 0 ? t("board.dueToday") : t("board.dueSoon", { days: signal.days })}</span>}
        {signal.kind === "scheduled" && <span className="op-task-tag">{task.due}</span>}
      </span>
      {locked
        ? <span className="op-muted op-board-card__locked">{t("board.locked")}</span>
        : <select aria-label={`${t("board.moveTo")}: ${task.text}`} value={task.status} onChange={(event) => void move(task, event.target.value)}>
            <option value="open">{t("task.open")}</option><option value="doing">{t("task.doing")}</option><option value="done">{t("task.done")}</option>
          </select>}
    </li>;
  };

  const timeGroupLabel = (key: TimeGroupKey) =>
    key === "overdue" ? t("time.overdue") : key === "today" ? t("time.today") : key === "thisWeek" ? t("time.thisWeek") : key === "nextWeek" ? t("time.nextWeek") : key === "later" ? t("time.later") : t("time.unscheduled");

  return <section className="op-section" aria-labelledby="tasks-heading" data-testid="task-board">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("task.kicker")}</p><h3 id="tasks-heading">{t("task.title")}</h3></div><span className="op-section__count">{tasks.length}</span></div>
    <p className="op-muted">{t("task.relationship")}</p>
    <div className="op-task-add"><input aria-label={t("task.new")} placeholder={t("task.new")} value={text} onChange={(event) => setText(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void add(); }} /><label><input type="checkbox" checked={unclear} onChange={(event) => setUnclear(event.target.checked)} /> {t("task.unclear")}</label><button className="op-button op-button--primary" type="button" disabled={!text.trim() || !data} onClick={() => void add()}>{t("task.add")}</button></div>
    <div className="op-task-tagfilter" role="group" aria-label={t("board.viewLabel")}><span className="op-muted">{t("board.viewLabel")}</span><FilterChip label={t("board.viewList")} pressed={viewMode === "list"} onClick={() => switchView("list")} /><FilterChip label={t("board.viewBoard")} pressed={viewMode === "board"} onClick={() => switchView("board")} /><FilterChip label={t("board.viewTime")} pressed={viewMode === "time"} onClick={() => switchView("time")} /></div>
    {allTags.length > 0 && <div className="op-task-tagfilter" role="group" aria-label={t("task.filterTags")}><span className="op-muted">{t("task.filterTags")}</span>{allTags.map((tag) => <FilterChip key={tag} label={tag} pressed={tagFilter.some((selected) => selected.toLowerCase() === tag.toLowerCase())} onClick={() => setTagFilter((current) => current.some((selected) => selected.toLowerCase() === tag.toLowerCase()) ? current.filter((selected) => selected.toLowerCase() !== tag.toLowerCase()) : [...current, tag])} />)}{tagFilter.length > 0 && <button className="op-button op-button--ghost" type="button" onClick={() => setTagFilter([])}>{t("task.tagFilterClear")}</button>}</div>}
    {message && <p role="status" className="op-error">{message}</p>}
    {proposal && <div className="op-proposal" role="region" aria-label={t("task.proposal")}><p><strong>{t("task.advanceReady")}</strong></p>{proposal.candidates.map((candidate, index) => <label key={`${proposal.proposal_id}-${index}`}><input type="checkbox" checked={proposal.selected[index]} onChange={(event) => setProposal({ ...proposal, selected: proposal.selected.map((value, itemIndex) => itemIndex === index ? event.target.checked : value) })} />{candidate}</label>)}<div className="op-task-actions"><button className="op-button op-button--primary" type="button" disabled={!proposal.selected.some(Boolean)} onClick={() => void adopt()}>{t("task.adoptSelected")}</button><button className="op-button op-button--ghost" type="button" onClick={() => setProposal(null)}>{t("common.cancel")}</button></div></div>}
    <datalist id="op-task-tag-options">{allTags.map((tag) => <option key={tag} value={tag} />)}</datalist>
    {isLoading ? <p className="op-muted">{t("task.loading")}</p> : tasks.length === 0 ? <p className="op-muted">{t("task.empty")}</p> : viewMode === "time" ? (() => {
      const groups = timeGroups(visible, today);
      if (groups.length === 0) return <p className="op-muted">{t("time.empty")}</p>;
      return <div className="op-time-groups" data-testid="task-time-groups">
        {groups.map((group) => <section key={group.key} className="op-board-col" aria-labelledby={`time-group-${group.key}`}>
          <h4 id={`time-group-${group.key}`}>{timeGroupLabel(group.key)} <span className="op-section__count">{group.tasks.length}</span></h4>
          <ul>{group.tasks.map(card)}</ul>
        </section>)}
      </div>;
    })() : viewMode === "board" ? (() => {
      const columns = boardColumns(visible, today);
      return <div className="op-board" data-testid="task-board-columns">
        {(["open", "doing", "done"] as const).map((status) => {
          const total = columns[status].length;
          const items = status === "done" && !doneExpanded ? columns.done.slice(0, DONE_PREVIEW_COUNT) : columns[status];
          return <section key={status} className="op-board-col" aria-labelledby={`board-col-${status}`}>
            <h4 id={`board-col-${status}`}>{status === "open" ? t("task.open") : status === "doing" ? t("task.doing") : t("task.done")} <span className="op-section__count">{total}</span></h4>
            <ul>{items.map(card)}</ul>
            {status === "done" && total > DONE_PREVIEW_COUNT && <button className="op-button op-button--ghost" type="button" onClick={() => setDoneExpanded((value) => !value)}>{doneExpanded ? t("board.doneCollapse") : t("board.doneShowAll", { count: total })}</button>}
          </section>;
        })}
      </div>;
    })() : <ul className="op-task-list">{visible.map((task) => { const value = draft(task); const linked = task.linked_work_item_id !== null; return <li key={task.id} className="op-task-item"><div className="op-task-main"><span className={task.unclear ? "op-task-unclear" : ""}>{task.unclear ? "? " : ""}{task.text}{task.is_current_commitment ? ` · ${t("task.currentCommitment")}` : ""}</span>{task.tags.length > 0 && <span className="op-task-tags">{task.tags.map((tag) => <span key={tag} className="op-task-tag">{tag}</span>)}</span>}{task.adopted_from_proposal_id && <small>{t("task.fromProposal", { id: task.adopted_from_proposal_id })}</small>}<input type="text" inputMode="numeric" placeholder="YYYY-MM-DD" aria-label={`${t("task.due")}: ${task.text}`} value={value.due} onChange={(event) => setDrafts((all) => ({ ...all, [task.id]: { ...value, due: event.target.value } }))} /><input aria-label={`${t("task.note")}: ${task.text}`} placeholder={t("task.note")} value={value.note} onChange={(event) => setDrafts((all) => ({ ...all, [task.id]: { ...value, note: event.target.value } }))} /><input list="op-task-tag-options" aria-label={`${t("task.tags")}: ${task.text}`} placeholder={t("task.tagsHint")} value={value.tags} onChange={(event) => setDrafts((all) => ({ ...all, [task.id]: { ...value, tags: event.target.value } }))} /></div><div className="op-task-actions"><select disabled={linked} aria-label={`${t("task.status")}: ${task.text}`} value={value.status} onChange={(event) => setDrafts((all) => ({ ...all, [task.id]: { ...value, status: event.target.value } }))}><option value="open">{t("task.open")}</option><option value="doing">{t("task.doing")}</option><option value="done">{t("task.done")}</option></select><button className="op-button op-button--secondary" type="button" onClick={() => void update(task)}>{t("task.save")}</button>{!linked && !hasCurrentCommitment && <button className="op-button op-button--secondary" type="button" onClick={() => void promote(task)}>{t("task.makeCommitment")}</button>}{task.unclear && <button className="op-button op-button--ghost" type="button" onClick={() => void advance(task)}>{t("task.advance")}</button>}<button className="op-button op-button--ghost" type="button" disabled={linked} onClick={() => void remove(task)}>{t("task.remove")}</button></div></li>; })}</ul>}
  </section>;
}
