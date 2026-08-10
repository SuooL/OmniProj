import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "./api";
import { ProjectCard } from "./components/ProjectCard";
import { ProjectDetail } from "./components/ProjectDetail";

// Attend layer, first screen: the registered projects with their git-derived facts.
// Click a card to open its Record view (next-action list). Read via Tauri IPC. Refresh
// is a pull, never an auto-poll. Staleness thresholds / reminders land in later milestones.

export function App() {
  const [selected, setSelected] = useState<{ hash: string; name: string } | null>(null);
  const { data, isLoading, isError, isFetching, refetch } = useQuery({
    queryKey: ["projects"],
    queryFn: api.projects,
  });

  if (selected) {
    return (
      <ProjectDetail
        hash={selected.hash}
        name={selected.name}
        onBack={() => setSelected(null)}
      />
    );
  }

  return (
    <div className="min-h-full max-w-6xl mx-auto px-6 py-6">
      <header className="flex items-center gap-4 mb-5">
        <h1 className="text-xl font-semibold tracking-tight">OmniProj</h1>
        <span className="text-sm text-[var(--color-muted)]">
          {data ? `${data.length} project${data.length === 1 ? "" : "s"}` : ""}
        </span>
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="ml-auto text-xs rounded border border-[var(--color-edge)] px-2.5 py-1 text-[var(--color-fg)] hover:bg-[var(--color-panel)] disabled:opacity-50"
        >
          {isFetching ? "refreshing…" : "refresh"}
        </button>
      </header>

      {isLoading && <p className="text-[var(--color-muted)]">reading ~/.omniproj…</p>}
      {isError && (
        <p className="text-[var(--color-flag)]">could not read ~/.omniproj</p>
      )}
      {data && data.length === 0 && (
        <p className="text-[var(--color-muted)]">
          no registered projects yet — <code className="font-mono">omniproj add &lt;repo&gt;</code>
        </p>
      )}

      <div className="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(380px,1fr))]">
        {data?.map((c) => (
          <ProjectCard
            key={c.hash}
            c={c}
            onOpen={() => setSelected({ hash: c.hash, name: c.name })}
          />
        ))}
      </div>
    </div>
  );
}
