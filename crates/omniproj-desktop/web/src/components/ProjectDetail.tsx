import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Task, type TaskStatus } from "../api";

// Record layer (M2): the project's next-action list. User ground truth — every edit is
// an explicit user action that the backend versions as one revertable store commit.
// Status cycles open → doing → done → open on click; `[/]`/due round-trip in next.md.

const NEXT: Record<TaskStatus, TaskStatus> = { open: "doing", doing: "done", done: "open" };
const GLYPH: Record<TaskStatus, string> = { open: "○", doing: "◐", done: "●" };

export function ProjectDetail({
  hash,
  name,
  onBack,
}: {
  hash: string;
  name: string;
  onBack: () => void;
}) {
  const qc = useQueryClient();
  const key = ["tasks", hash];
  const { data, isLoading, isError } = useQuery({ queryKey: key, queryFn: () => api.tasks(hash) });
  const [draft, setDraft] = useState("");
  const [unclear, setUnclear] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: key });

  const add = useMutation({
    mutationFn: () => api.addTask(hash, draft, unclear),
    onSuccess: () => {
      setDraft("");
      setUnclear(false);
      refresh();
    },
  });
  const cycle = useMutation({
    mutationFn: (t: Task) => api.setTaskStatus(hash, t.id!, NEXT[t.status]),
    onSuccess: refresh,
  });
  const setDue = useMutation({
    mutationFn: (v: { id: string; date: string | null }) => api.setTaskDue(hash, v.id, v.date),
    onSuccess: refresh,
  });
  const remove = useMutation({ mutationFn: (id: string) => api.removeTask(hash, id), onSuccess: refresh });

  const tasks = (data ?? []).filter((t) => t.id);

  return (
    <div className="min-h-full max-w-3xl mx-auto px-6 py-6">
      <header className="flex items-center gap-3 mb-5">
        <button
          onClick={onBack}
          className="text-xs rounded border border-[var(--color-edge)] px-2.5 py-1 text-[var(--color-fg)] hover:bg-[var(--color-panel)]"
        >
          ← back
        </button>
        <h1 className="text-xl font-semibold tracking-tight">{name}</h1>
        <span className="text-sm text-[var(--color-muted)]">
          {tasks.filter((t) => t.status !== "done").length} open
        </span>
      </header>

      {isLoading && <p className="text-[var(--color-muted)]">reading notes/next.md…</p>}
      {isError && <p className="text-[var(--color-flag)]">could not read tasks</p>}

      <ul className="flex flex-col gap-1.5 mb-5">
        {tasks.map((t) => (
          <li
            key={t.id}
            className="flex items-center gap-3 rounded border border-[var(--color-edge)] bg-[var(--color-panel)] px-3 py-2"
          >
            <button
              title={`status: ${t.status} (click to cycle)`}
              onClick={() => cycle.mutate(t)}
              className="text-lg leading-none w-6 text-[var(--color-active)]"
            >
              {GLYPH[t.status]}
            </button>
            <span
              className={`flex-1 ${
                t.status === "done" ? "line-through text-[var(--color-muted)]" : "text-[var(--color-fg)]"
              }`}
            >
              {t.unclear && (
                <span className="text-[var(--color-warm)]" title="未成形">
                  ?{" "}
                </span>
              )}
              {t.text}
            </span>
            <input
              type="date"
              value={t.due ?? ""}
              onChange={(e) => setDue.mutate({ id: t.id!, date: e.target.value || null })}
              className="text-xs bg-transparent border border-[var(--color-edge)] rounded px-1.5 py-0.5 text-[var(--color-muted)]"
            />
            <button
              onClick={() => remove.mutate(t.id!)}
              title="delete"
              className="text-[var(--color-muted)] hover:text-[var(--color-flag)] px-1"
            >
              ×
            </button>
          </li>
        ))}
        {data && tasks.length === 0 && (
          <li className="text-[var(--color-muted)] text-sm">no next-actions yet.</li>
        )}
      </ul>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (draft.trim()) add.mutate();
        }}
        className="flex items-center gap-2"
      >
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="add a next-action…"
          className="flex-1 bg-[var(--color-panel)] border border-[var(--color-edge)] rounded px-3 py-1.5 text-sm text-[var(--color-fg)] placeholder:text-[var(--color-muted)]"
        />
        <label className="text-xs text-[var(--color-muted)] flex items-center gap-1">
          <input type="checkbox" checked={unclear} onChange={(e) => setUnclear(e.target.checked)} /> ?未成形
        </label>
        <button
          type="submit"
          disabled={!draft.trim() || add.isPending}
          className="text-xs rounded border border-[var(--color-edge)] px-3 py-1.5 text-[var(--color-fg)] hover:bg-[var(--color-panel)] disabled:opacity-50"
        >
          add
        </button>
      </form>
      {add.isError && <p className="text-[var(--color-flag)] text-xs mt-1">{String(add.error)}</p>}
    </div>
  );
}
