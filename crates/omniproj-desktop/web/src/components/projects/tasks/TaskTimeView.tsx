// "What comes due next", grouped against the local calendar. Done tasks are excluded: this
// answers a forward question, not a retrospective.

import type { Task } from "../../../domain/project";
import { dueSignal, timeGroups, type TimeGroupKey } from "../../../domain/taskBoardModel";
import { useI18n } from "../../../i18n/I18nProvider";
import { TaskDueBadge } from "./TaskDueBadge";

export interface TaskTimeViewProps {
  tasks: Task[];
  today: string;
  onMove: (task: Task, status: string) => void;
}

export function TaskTimeView({ tasks, today, onMove }: TaskTimeViewProps) {
  const { t } = useI18n();
  const groups = timeGroups(tasks, today);
  if (groups.length === 0) return <p className="op-muted">{t("time.empty")}</p>;

  const label = (key: TimeGroupKey) =>
    key === "overdue" ? t("time.overdue")
      : key === "today" ? t("time.today")
        : key === "thisWeek" ? t("time.thisWeek")
          : key === "nextWeek" ? t("time.nextWeek")
            : key === "later" ? t("time.later")
              : t("time.unscheduled");

  return (
    <div className="op-time-groups" data-testid="task-time-groups">
      {groups.map((group) => (
        <section key={group.key} className="op-board-col" aria-labelledby={`time-group-${group.key}`}>
          <h4 id={`time-group-${group.key}`}>
            {label(group.key)} <span className="op-section__count">{group.tasks.length}</span>
          </h4>
          <ul>
            {group.tasks.map((task) => (
              <li key={task.id} className="op-board-card">
                <span className={task.unclear ? "op-task-unclear" : ""}>{task.unclear ? "? " : ""}{task.text}</span>
                <span className="op-board-card__meta">
                  {task.is_current_commitment && <span className="op-task-tag">{t("task.currentCommitment")}</span>}
                  {task.tags.map((tag) => <span key={tag} className="op-task-tag">{tag}</span>)}
                  <TaskDueBadge signal={dueSignal(task.due, today, task.status)} due={task.due} />
                </span>
                {task.is_current_commitment
                  ? <span className="op-muted op-board-card__locked">{t("board.locked")}</span>
                  : (
                    <select
                      aria-label={`${t("board.moveTo")}: ${task.text}`}
                      value={task.status}
                      onChange={(event) => onMove(task, event.target.value)}
                    >
                      <option value="open">{t("task.open")}</option>
                      <option value="doing">{t("task.doing")}</option>
                      <option value="done">{t("task.done")}</option>
                    </select>
                  )}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
