// A Human-declared lifecycle exception. `active` is the norm and renders nothing (the badge
// budget allows at most one project-state tag, often zero). Tone is fixed per status; the
// caller cannot choose a color.

import type { ProjectStatus } from "../../domain/project";
import { toneStyle, type StatusTone } from "./tone";
import { projectStatusLabel, useI18n } from "../../i18n/I18nProvider";

const STATE_TONE: Partial<Record<ProjectStatus, StatusTone>> = {
  setup: "info",
  waiting: "neutral",
  parked: "neutral",
  archived: "neutral",
};

export function ProjectStateTag({ status }: { status: ProjectStatus }) {
  const { locale } = useI18n();
  const tone = STATE_TONE[status];
  if (!tone) return null;
  return (
    <span className="op-tag" style={toneStyle(tone)} data-state={status}>
      {projectStatusLabel(status, locale)}
    </span>
  );
}
