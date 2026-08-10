import type { ProjectCard as Card } from "../api";
import { Sparkline } from "./Sparkline";

function ago(iso: string | null): string {
  if (!iso) return "never";
  const d = (Date.now() - new Date(iso).getTime()) / 86400000;
  if (d < 1) return "today";
  return `${Math.round(d)}d ago`;
}

export function ProjectCard({ c }: { c: Card }) {
  return (
    <div className="rounded-lg border border-[var(--color-edge)] bg-[var(--color-panel)] p-4 flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <span className="font-semibold text-[var(--color-fg)] truncate">{c.name}</span>
        {c.branch && (
          <span className="font-mono text-xs text-[var(--color-muted)] truncate">
            {c.branch}
          </span>
        )}
      </div>

      <Sparkline weeks={c.commit_weeks} />

      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-[var(--color-muted)] items-center">
        {c.dirty > 0 && (
          <span className="rounded border border-[var(--color-warm)] text-[var(--color-warm)] px-1.5">
            {c.dirty} uncommitted
          </span>
        )}
        <span className="ml-auto">distilled {ago(c.last_distilled)}</span>
      </div>

      <div className="text-[11px] text-[var(--color-cold)] font-mono truncate" title={c.path}>
        {c.path}
      </div>
    </div>
  );
}
