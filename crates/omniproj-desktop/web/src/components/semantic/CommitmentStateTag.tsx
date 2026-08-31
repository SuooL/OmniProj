// The commitment lifecycle state, shown in Current commitment and the history
// rail. Teal/green is reserved for confirmed completion; the caller cannot pick a color.

import type { WorkItemStatus } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";
import { useI18n, workItemStatusLabel } from "../../i18n/I18nProvider";

const COMMIT_TONE: Record<WorkItemStatus, StatusTone> = {
  planned: "neutral", doing: "info", blocked: "warning", done: "success", abandoned: "neutral",
};

export function CommitmentStateTag({ status }: { status: WorkItemStatus }) {
  const { locale } = useI18n();
  return (
    <span className="op-tag" style={toneStyle(COMMIT_TONE[status])} data-commit-status={status}>
      {workItemStatusLabel(status, locale)}
    </span>
  );
}
