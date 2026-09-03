// One task in the list: a read-only summary row that opens into an edit panel. Editing
// autosaves, so the panel carries no Save control.

import type { Task } from "../../../domain/project";
import { dueSignal, parseTagsInput, type TaskDraft } from "../../../domain/taskBoardModel";
import { DateField } from "../../semantic/DateField";
import { TagField } from "../../semantic/TagField";
import { useI18n } from "../../../i18n/I18nProvider";
import { TaskDueBadge } from "./TaskDueBadge";

export interface TaskListRowProps {
  task: Task;
  draft: TaskDraft;
  open: boolean;
  today: string;
  vocabulary: string[];
  /** True when no other task is currently marked, so this one may be marked. */
  canMarkNowDoing: boolean;
  onToggle: () => void;
  onDraftChange: (draft: TaskDraft) => void;
  onStatusChange: (status: string) => void;
  onPanelBlur: (event: React.FocusEvent<HTMLDivElement>) => void;
  onMarkNowDoing: () => void;
  onAdvance: () => void;
  onRemove: () => void;
}

export function TaskListRow({
  task,
  draft,
  open,
  today,
  vocabulary,
  canMarkNowDoing,
  onToggle,
  onDraftChange,
  onStatusChange,
  onPanelBlur,
  onMarkNowDoing,
  onAdvance,
  onRemove,
}: TaskListRowProps) {
  const { t } = useI18n();
  // Only the item the commitment points at right now is locked. A task that merely *was* the
  // commitment stays fully editable.
  const locked = task.is_current_commitment;
  const signal = dueSignal(task.due, today, task.status);

  return (
    <li className={`op-task-item${open ? " op-task-item--open" : ""}`}>
      {/* Collapsed row: scannable facts only. The whole row is one button that opens editing. */}
      <div className="op-task-row">
        <button type="button" className="op-task-summary" aria-expanded={open} onClick={onToggle}>
          <span className="op-task-summary__text">
            {task.unclear && <span className="op-task-unclear" aria-label={t("task.unclear")}>?</span>}
            {task.text}
          </span>
          <span className="op-task-summary__meta">
            {task.is_current_commitment && <span className="op-task-tag">{t("task.currentCommitment")}</span>}
            <TaskDueBadge signal={signal} due={task.due} />
            {task.tags.map((tag) => <span key={tag} className="op-task-tag">{tag}</span>)}
          </span>
        </button>
        <select
          className="op-task-status"
          disabled={locked}
          aria-label={`${t("task.status")}: ${task.text}`}
          value={draft.status}
          onChange={(event) => onStatusChange(event.target.value)}
        >
          <option value="open">{t("task.open")}</option>
          <option value="doing">{t("task.doing")}</option>
          <option value="done">{t("task.done")}</option>
        </select>
      </div>

      {open && (
        <div className="op-task-edit" onBlur={onPanelBlur}>
          <div className="op-field">
            <span id={`due-label-${task.id}`}>{t("task.due")}</span>
            <DateField
              value={draft.due}
              today={today}
              ariaLabel={`${t("task.due")}: ${task.text}`}
              onChange={(due) => onDraftChange({ ...draft, due })}
            />
          </div>
          <div className="op-field">
            <span id={`tags-label-${task.id}`}>{t("task.tags")}</span>
            <TagField
              value={parseTagsInput(draft.tags)}
              vocabulary={vocabulary}
              ariaLabel={`${t("task.tags")}: ${task.text}`}
              onChange={(tags) => onDraftChange({ ...draft, tags: tags.join(", ") })}
            />
          </div>
          <label className="op-field op-field--wide">
            <span>{t("task.note")}</span>
            <input
              aria-label={`${t("task.note")}: ${task.text}`}
              placeholder={t("task.notePlaceholder")}
              value={draft.note}
              onChange={(event) => onDraftChange({ ...draft, note: event.target.value })}
            />
          </label>
          {task.adopted_from_proposal_id && (
            <small className="op-field--wide">{t("task.fromProposal", { id: task.adopted_from_proposal_id })}</small>
          )}
          <div className="op-task-actions op-field--wide">
            <span className="op-muted op-task-autosave">{t("task.autosave")}</span>
            {!locked && canMarkNowDoing && (
              <button className="op-button op-button--secondary" type="button" onClick={onMarkNowDoing}>
                {t("task.makeCommitment")}
              </button>
            )}
            {task.unclear && (
              <button className="op-button op-button--ghost" type="button" onClick={onAdvance}>
                {t("task.advance")}
              </button>
            )}
            <button className="op-button op-button--ghost" type="button" disabled={locked} onClick={onRemove}>
              {t("task.remove")}
            </button>
          </div>
        </div>
      )}
    </li>
  );
}
