// The project's steps, as one outline — the page the notebook had.
//
// It is a single list with real nesting, numbered at the top level, edited in place. No
// second view of the same rows, no board, no separate card for whichever step is current:
// the thing you look at and the thing you edit are the same lines.

import { useMemo, useState } from "react";

import type { ProjectId, Task } from "../../domain/project";
import {
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
  subtreeProgress,
  topLevelNumbers,
} from "../../domain/outline";
import { localToday } from "../../domain/taskBoardModel";
import { useOutlineTasks } from "../../hooks/useOutlineTasks";
import { useI18n } from "../../i18n/I18nProvider";
import { OutlineRow, type RowIntent } from "./tasks/OutlineRow";

export interface TaskOutlineProps {
  projectId: ProjectId;
}

export function TaskOutline({ projectId }: TaskOutlineProps) {
  const { t } = useI18n();
  const outline = useOutlineTasks(projectId);
  const { tasks } = outline;
  const [editingId, setEditingId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  // An Agent breakdown of one step: candidates the user picks from, adopted as its
  // sub-steps. Nothing is written until something is chosen.
  const [proposal, setProposal] = useState<
    { task: Task; id: string; candidates: string[]; chosen: boolean[] } | null
  >(null);
  const [breakingDownId, setBreakingDownId] = useState<string | null>(null);
  const today = localToday();

  const numbers = useMemo(() => topLevelNumbers(tasks), [tasks]);
  const vocabulary = useMemo(() => {
    const seen = new Set<string>();
    const ordered: string[] = [];
    for (const task of tasks) {
      for (const tag of task.tags) {
        const lower = tag.toLowerCase();
        if (!seen.has(lower)) {
          seen.add(lower);
          ordered.push(tag);
        }
      }
    }
    return ordered;
  }, [tasks]);

  const remaining = tasks.filter((task) => task.status !== "done").length;

  /** Open the step at `offset` from `task` in reading order, if there is one. */
  function focusNeighbour(task: Task, offset: number) {
    const index = tasks.findIndex((candidate) => candidate.id === task.id);
    const neighbour = tasks[index + offset];
    if (neighbour) setEditingId(neighbour.id);
  }

  async function handleIntent(task: Task, intent: RowIntent, text: string) {
    // Whatever the keystroke was, the text on screen is what the user meant to keep.
    const trimmed = text.trim();
    if (trimmed && trimmed !== task.text) await outline.rename(task, trimmed);

    switch (intent.kind) {
      case "newSibling": {
        const id = await outline.add("", { parent_id: task.parent_id, after_id: task.id });
        setEditingId(id);
        return;
      }
      case "indent": {
        const target = indentTarget(tasks, task);
        if (target) await outline.move(task, target);
        return;
      }
      case "outdent": {
        const target = outdentTarget(tasks, task);
        if (target) await outline.move(task, target);
        return;
      }
      case "moveUp": {
        const target = moveUpTarget(tasks, task);
        if (target) await outline.move(task, target);
        return;
      }
      case "moveDown": {
        const target = moveDownTarget(tasks, task);
        if (target) await outline.move(task, target);
        return;
      }
      case "focusPrevious":
        focusNeighbour(task, -1);
        return;
      case "focusNext":
        focusNeighbour(task, 1);
        return;
      case "removeEmpty": {
        // Deleting a step deletes its sub-steps, so an outline-collapsing Backspace is
        // only offered for a leaf. A step with detail under it must be removed on purpose.
        if (tasks.some((candidate) => candidate.parent_id === task.id)) return;
        focusNeighbour(task, -1);
        await outline.remove(task);
      }
    }
  }

  async function breakDown(task: Task) {
    setBreakingDownId(task.id);
    const next = await outline.breakDown(task);
    setBreakingDownId(null);
    if (next) {
      setProposal({
        task,
        id: next.proposal_id,
        candidates: next.candidates,
        chosen: next.candidates.map(() => false),
      });
    }
  }

  async function adoptChosen() {
    if (!proposal) return;
    const texts = proposal.candidates.filter((_, index) => proposal.chosen[index]);
    setProposal(null);
    await outline.adopt(proposal.task, proposal.id, texts);
  }

  async function addAtEnd() {
    const text = draft.trim();
    if (!text) return;
    const lastTopLevel = [...tasks].reverse().find((task) => task.parent_id === null);
    setDraft("");
    await outline.add(text, { parent_id: null, after_id: lastTopLevel?.id ?? null });
  }

  return (
    <section className="op-outline" aria-labelledby="outline-heading" data-testid="task-outline">
      <div className="op-section__header">
        <h2 id="outline-heading">{t("outline.title")}</h2>
        <span className="op-section__count">
          {t("outline.remaining", { count: remaining, total: tasks.length })}
        </span>
      </div>

      {outline.error && (
        <p role="alert" className="op-error" data-testid="outline-error">{outline.error}</p>
      )}

      {proposal && (
        <div className="op-proposal" role="region" aria-label={t("task.proposal")}>
          <p>{t("task.advanceReady", { text: proposal.task.text })}</p>
          {proposal.candidates.map((candidate, index) => (
            <label key={`${proposal.id}-${index}`}>
              <input
                type="checkbox"
                checked={proposal.chosen[index]}
                onChange={(event) =>
                  setProposal({
                    ...proposal,
                    chosen: proposal.chosen.map((value, at) =>
                      at === index ? event.target.checked : value,
                    ),
                  })
                }
              />
              {candidate}
            </label>
          ))}
          <div className="op-outline__detail-actions">
            <button
              type="button"
              className="op-button op-button--primary"
              disabled={!proposal.chosen.some(Boolean)}
              onClick={() => void adoptChosen()}
            >
              {t("task.adoptSelected")}
            </button>
            <button type="button" className="op-button op-button--ghost" onClick={() => setProposal(null)}>
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {outline.isLoading ? (
        <p className="op-muted">{t("task.loading")}</p>
      ) : tasks.length === 0 ? (
        <p className="op-muted">{t("outline.empty")}</p>
      ) : (
        <ul className="op-outline__list">
          {tasks.map((task) => (
            <OutlineRow
              key={task.id}
              task={task}
              number={numbers.get(task.id) ?? null}
              progress={subtreeProgress(tasks, task)}
              today={today}
              vocabulary={vocabulary}
              editing={editingId === task.id}
              expanded={expandedId === task.id}
              onStartEditing={() => setEditingId(task.id)}
              onStopEditing={() => setEditingId((current) => (current === task.id ? null : current))}
              onToggleExpanded={() => setExpandedId((current) => (current === task.id ? null : task.id))}
              onRename={(text) => void outline.rename(task, text)}
              onToggleDone={() => void outline.setStatus(task, task.status === "done" ? "open" : "done")}
              onSetDetails={(details) => void outline.setDetails(task, details)}
              onRemove={() => void outline.remove(task)}
              onBreakDown={() => void breakDown(task)}
              breakingDown={breakingDownId === task.id}
              onIntent={(intent, text) => void handleIntent(task, intent, text)}
            />
          ))}
        </ul>
      )}

      <form
        className="op-outline__compose"
        onSubmit={(event) => {
          event.preventDefault();
          void addAtEnd();
        }}
      >
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder={t("outline.addPlaceholder")}
          aria-label={t("outline.addStep")}
        />
        <button
          type="submit"
          className="op-button"
          disabled={!draft.trim()}
          // A disabled control has to say why it is disabled, not just look inert.
          title={draft.trim() ? undefined : t("outline.addDisabled")}
        >
          {t("outline.addStep")}
        </button>
      </form>

      <p className="op-outline__hint">{t("outline.keys")}</p>
    </section>
  );
}
