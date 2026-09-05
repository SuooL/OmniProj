// One line of the outline: a checkbox, a number or bullet, the text, and — only while
// the row is open — the details that used to demand their own panel.
//
// The text is a plain span until it is clicked. Rendering fifty inputs at once turns a
// list you read into a form you fill in, and that is the difference between glancing at
// the page and working through it.

import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import type { Task } from "../../../domain/project";
import { dueSignal } from "../../../domain/taskBoardModel";
import { useI18n } from "../../../i18n/I18nProvider";
import { DateField } from "../../semantic/DateField";
import { TagField } from "../../semantic/TagField";
import { TaskDueBadge } from "./TaskDueBadge";

/** What a keystroke in the text field asks the outline to do next. */
export type RowIntent =
  | { kind: "newSibling" }
  | { kind: "indent" }
  | { kind: "outdent" }
  | { kind: "moveUp" }
  | { kind: "moveDown" }
  | { kind: "focusPrevious" }
  | { kind: "focusNext" }
  | { kind: "removeEmpty" };

export interface OutlineRowProps {
  task: Task;
  number: number | null;
  progress: { done: number; total: number } | null;
  today: string;
  vocabulary: string[];
  editing: boolean;
  expanded: boolean;
  onStartEditing: () => void;
  onStopEditing: () => void;
  onToggleExpanded: () => void;
  onRename: (text: string) => void;
  onToggleDone: () => void;
  onSetDetails: (details: { due: string | null; note: string | null; tags: string[] }) => void;
  onRemove: () => void;
  onBreakDown: () => void;
  breakingDown: boolean;
  onIntent: (intent: RowIntent, text: string) => void;
}

export function OutlineRow({
  task,
  number,
  progress,
  today,
  vocabulary,
  editing,
  expanded,
  onStartEditing,
  onStopEditing,
  onToggleExpanded,
  onRename,
  onToggleDone,
  onSetDetails,
  onRemove,
  onBreakDown,
  breakingDown,
  onIntent,
}: OutlineRowProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(task.text);
  const input = useRef<HTMLInputElement>(null);

  // A rename from elsewhere (or a discarded edit) has to win over a stale local draft.
  useEffect(() => {
    if (!editing) setDraft(task.text);
  }, [editing, task.text]);

  useEffect(() => {
    if (!editing) return;
    const field = input.current;
    if (!field) return;
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }, [editing]);

  function commit() {
    const text = draft.trim();
    if (text && text !== task.text) onRename(text);
    else if (!text) setDraft(task.text);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    const intent = ((): RowIntent | null => {
      if (event.key === "Enter" && !event.shiftKey) return { kind: "newSibling" };
      if (event.key === "Tab") return event.shiftKey ? { kind: "outdent" } : { kind: "indent" };
      if (event.altKey && event.key === "ArrowUp") return { kind: "moveUp" };
      if (event.altKey && event.key === "ArrowDown") return { kind: "moveDown" };
      if (!event.altKey && event.key === "ArrowUp") return { kind: "focusPrevious" };
      if (!event.altKey && event.key === "ArrowDown") return { kind: "focusNext" };
      // Backspace only deletes when the field is already empty, so it can never eat text.
      if (event.key === "Backspace" && draft === "") return { kind: "removeEmpty" };
      return null;
    })();

    if (event.key === "Escape") {
      setDraft(task.text);
      onStopEditing();
      return;
    }
    if (!intent) return;
    event.preventDefault();
    onIntent(intent, draft);
  }

  const signal = dueSignal(task.due, today);
  const done = task.status === "done";

  return (
    <li
      className="op-outline__row"
      data-depth={Math.min(task.depth, 4)}
      data-status={task.status}
      data-testid="outline-row"
      style={{ "--op-outline-depth": task.depth } as React.CSSProperties}
    >
      <div className="op-outline__line">
        <input
          type="checkbox"
          className="op-outline__check"
          checked={done}
          onChange={onToggleDone}
          aria-label={t("outline.toggleDone", { text: task.text })}
        />

        <span className="op-outline__marker" aria-hidden="true">
          {number !== null ? `${number}.` : "–"}
        </span>

        {editing ? (
          <input
            ref={input}
            className="op-outline__input"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={onKeyDown}
            onBlur={() => {
              commit();
              onStopEditing();
            }}
            aria-label={t("outline.editStep")}
          />
        ) : (
          <button
            type="button"
            className="op-outline__text"
            data-focus-row={task.id}
            onClick={onStartEditing}
          >
            {task.text}
          </button>
        )}

        {task.unclear && <span className="op-outline__flag" title={t("task.unclear")}>?</span>}
        {progress && (
          <span className="op-outline__progress">{progress.done}/{progress.total}</span>
        )}
        {task.commits.length > 0 && (
          <span className="op-outline__commits" title={task.commits.join(", ")}>
            {t("outline.commitCount", { count: task.commits.length })}
          </span>
        )}
        <TaskDueBadge signal={signal} due={task.due} />

        <button
          type="button"
          className="op-outline__disclose"
          aria-expanded={expanded}
          onClick={onToggleExpanded}
        >
          {expanded ? t("outline.hideDetails") : t("outline.showDetails")}
        </button>
      </div>

      {task.note && !expanded && <p className="op-outline__note">{task.note}</p>}

      {expanded && (
        <div className="op-outline__details">
          <label className="op-field">
            <span>{t("task.due")}</span>
            <DateField
              value={task.due ?? ""}
              today={today}
              ariaLabel={t("task.due")}
              onChange={(due) => onSetDetails({ due: due || null, note: task.note, tags: task.tags })}
            />
          </label>
          <label className="op-field">
            <span>{t("task.note")}</span>
            <textarea
              className="op-outline__notefield"
              defaultValue={task.note ?? ""}
              rows={2}
              onBlur={(event) => {
                const note = event.target.value.trim() || null;
                if (note !== task.note) onSetDetails({ due: task.due, note, tags: task.tags });
              }}
            />
          </label>
          <label className="op-field">
            <span>{t("task.tags")}</span>
            <TagField
              value={task.tags}
              vocabulary={vocabulary}
              ariaLabel={t("task.tags")}
              onChange={(tags) => onSetDetails({ due: task.due, note: task.note, tags })}
            />
          </label>
          <div className="op-outline__detail-actions">
            <button type="button" className="op-button" onClick={onBreakDown} disabled={breakingDown}>
              {breakingDown ? t("task.advancing") : t("task.advance")}
            </button>
            <button type="button" className="op-button op-button--ghost" onClick={onRemove}>
              {t("outline.removeStep")}
            </button>
          </div>
        </div>
      )}
    </li>
  );
}
