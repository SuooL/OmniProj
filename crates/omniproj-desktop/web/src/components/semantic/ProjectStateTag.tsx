// A Human-declared lifecycle exception. `active` is the norm and renders nothing (the badge
// budget allows at most one project-state tag, often zero). Tone is fixed per status; the
// caller cannot choose a color.

import type { ProjectStatus } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";

const STATE_TAG: Partial<Record<ProjectStatus, { label: string; tone: StatusTone }>> = {
  setup: { label: "Setup", tone: "info" },
  waiting: { label: "Waiting", tone: "neutral" },
  parked: { label: "Parked", tone: "neutral" },
  archived: { label: "Archived", tone: "neutral" },
};

export function ProjectStateTag({ status }: { status: ProjectStatus }) {
  const tag = STATE_TAG[status];
  if (!tag) return null;
  return (
    <span className="op-tag" style={toneStyle(tag.tone)} data-state={status}>
      {tag.label}
    </span>
  );
}
