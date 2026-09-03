// One rendering of a due date, so the list and the time view cannot drift apart on how
// overdue, due-today and scheduled work look.

import type { DueSignal } from "../../../domain/taskBoardModel";
import { useI18n } from "../../../i18n/I18nProvider";
import { toneStyle } from "../../semantic/tone";

export function TaskDueBadge({ signal, due }: { signal: DueSignal; due: string | null }) {
  const { t } = useI18n();
  if (signal.kind === "overdue") {
    return <span className="op-badge" style={toneStyle("danger")}>{t("board.overdue", { days: signal.days })}</span>;
  }
  if (signal.kind === "soon") {
    return (
      <span className="op-badge" style={toneStyle("warning")}>
        {signal.days === 0 ? t("board.dueToday") : t("board.dueSoon", { days: signal.days })}
      </span>
    );
  }
  if (signal.kind === "scheduled") return <span className="op-task-due">{due}</span>;
  return null;
}
