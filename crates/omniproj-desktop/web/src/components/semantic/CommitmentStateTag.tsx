// The commitment lifecycle state, shown in Current commitment and the history
// rail. Teal/green is reserved for confirmed completion; the caller cannot pick a color.

import type { WorkItemStatus } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";

const COMMIT_TAG: Record<WorkItemStatus, { label: string; tone: StatusTone }> = {
  planned: { label: "Planned", tone: "neutral" },
  doing: { label: "Doing", tone: "info" },
  blocked: { label: "Blocked", tone: "warning" },
  done: { label: "Done", tone: "success" },
  abandoned: { label: "Abandoned", tone: "neutral" },
};

export function CommitmentStateTag({ status }: { status: WorkItemStatus }) {
  const tag = COMMIT_TAG[status];
  return (
    <span className="op-tag" style={toneStyle(tag.tone)} data-commit-status={status}>
      {tag.label}
    </span>
  );
}
