import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "./api";
import { ProjectCard } from "./components/ProjectCard";
import { ProjectDetail } from "./components/ProjectDetail";
import { Settings } from "./components/Settings";
import { staleness } from "./staleness";

// Attend layer, first screen: a situation board of every registered project, sorted by
// decay (most idle first — a neutral fact, not a ranking) so the eye lands on what's
// rotting. Click a card for its Record view. Read via Tauri IPC; refresh is a pull.

const LEGEND = [
  { tone: "var(--color-active)", label: "active ≤1w" },
  { tone: "var(--color-warm)", label: "stalling ≤4w" },
  { tone: "var(--color-cold)", label: "idle >4w" },
] as const;

export function App() {
  const [selected, setSelected] = useState<{ hash: string; name: string } | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const { data, isLoading, isError, isFetching, refetch } = useQuery({
    queryKey: ["projects"],
    queryFn: api.projects,
  });
  const attention = useQuery({ queryKey: ["attention"], queryFn: api.attention });

  if (showSettings) return <Settings onBack={() => setShowSettings(false)} />;
  if (selected) {
    return (
      <ProjectDetail hash={selected.hash} name={selected.name} onBack={() => setSelected(null)} />
    );
  }

  const needAttention = attention.data ?? [];
  const sorted = [...(data ?? [])].sort((a, b) => {
    const d = staleness(b.commit_weeks).idleWeeks - staleness(a.commit_weeks).idleWeeks;
    return d !== 0 ? d : a.name.localeCompare(b.name);
  });

  return (
    <div className="mx-auto min-h-full max-w-6xl px-6 py-6">
      <header className="mb-1 flex items-center gap-3">
        <h1 className="text-lg font-semibold tracking-tight">OmniProj</h1>
        <span className="font-mono text-xs text-[var(--color-dim)]">
          {data ? `${data.length} tracked` : ""}
        </span>
        {needAttention.length > 0 && (
          <span
            title={needAttention.join(", ")}
            className="rounded-full border border-[var(--color-warm)] px-2 py-0.5 text-[11px] text-[var(--color-warm)]"
          >
            {needAttention.length} need attention
          </span>
        )}
        <button
          onClick={() => setShowSettings(true)}
          className="ml-auto rounded border border-[var(--color-edge)] px-2.5 py-1 text-xs text-[var(--color-fg)] hover:bg-[var(--color-raised)]"
        >
          ⚙ reminders
        </button>
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="rounded border border-[var(--color-edge)] px-2.5 py-1 text-xs text-[var(--color-fg)] hover:bg-[var(--color-raised)] disabled:opacity-50"
        >
          {isFetching ? "refreshing…" : "refresh"}
        </button>
      </header>

      {/* Decay legend — thresholds are visible, never a hidden health score (charter §8 护栏 i) */}
      <div className="mb-5 flex items-center gap-4 font-mono text-[11px] text-[var(--color-muted)]">
        <span className="text-[var(--color-dim)]">sorted by decay</span>
        {LEGEND.map((l) => (
          <span key={l.label} className="flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full" style={{ background: l.tone }} />
            {l.label}
          </span>
        ))}
      </div>

      {isLoading && <p className="text-[var(--color-muted)]">reading ~/.omniproj…</p>}
      {isError && <p className="text-[var(--color-flag)]">Couldn't read ~/.omniproj. Is it initialized?</p>}
      {data && data.length === 0 && (
        <p className="text-[var(--color-muted)]">
          No projects tracked yet — register one with{" "}
          <code className="font-mono text-[var(--color-fg)]">omniproj add &lt;repo&gt;</code>.
        </p>
      )}

      <div className="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(360px,1fr))]">
        {sorted.map((c) => (
          <ProjectCard key={c.hash} c={c} onOpen={() => setSelected({ hash: c.hash, name: c.name })} />
        ))}
      </div>
    </div>
  );
}
