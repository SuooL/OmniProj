import type { ProjectCard as Card } from "../api";
import { Sparkline } from "./Sparkline";
import { staleness, TONE_COLOR } from "../staleness";

// A row on the situation board. Led by a decay rail (color = idle tone) so the eye lands on
// what's rotting first. Neutral facts only: last activity, uncommitted lines, branch, path.

export function ProjectCard({ c, onOpen }: { c: Card; onOpen?: () => void }) {
  const s = staleness(c.commit_weeks);
  const tone = TONE_COLOR[s.tone];
  return (
    <button
      onClick={onOpen}
      className="group flex w-full overflow-hidden rounded-lg border border-[var(--color-edge)] bg-[var(--color-panel)] text-left transition-colors hover:border-[var(--color-accent)]"
    >
      <span className="w-1 shrink-0" style={{ background: tone }} aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col gap-2 px-3.5 py-3">
        <div className="flex items-baseline gap-2">
          <span className="min-w-0 truncate font-semibold text-[var(--color-fg)]">{c.name}</span>
          {c.branch && (
            <span className="shrink-0 font-mono text-[11px] text-[var(--color-muted)]">{c.branch}</span>
          )}
        </div>

        <Sparkline weeks={c.commit_weeks} color={tone} />

        <div className="flex items-center gap-2 whitespace-nowrap font-mono text-[11px]">
          <span style={{ color: tone }}>{s.label}</span>
          {c.dirty > 0 && (
            <span className="text-[var(--color-warm)]">· {c.dirty} uncommitted</span>
          )}
        </div>

        <div className="truncate font-mono text-[10px] text-[var(--color-dim)]" title={c.path}>
          {c.path}
        </div>
      </div>
    </button>
  );
}
