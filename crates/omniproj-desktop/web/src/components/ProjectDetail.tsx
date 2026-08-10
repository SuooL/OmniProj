import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Task, type TaskStatus } from "../api";

// Record layer (M2): the project's next-action list (intent) beside its git commit
// timeline (actual). User ground truth — every edit is an explicit action the backend
// versions as one revertable store commit. Status cycles open → doing → done on click;
// `[/]`/due/commit-attributions round-trip in next.md. Commits attribute many-to-one.

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
  const tasksKey = ["tasks", hash];
  const tasksQ = useQuery({ queryKey: tasksKey, queryFn: () => api.tasks(hash) });
  const commitsQ = useQuery({ queryKey: ["commits", hash], queryFn: () => api.commits(hash, 30) });
  const [draft, setDraft] = useState("");
  const [unclear, setUnclear] = useState(false);
  const refreshTasks = () => qc.invalidateQueries({ queryKey: tasksKey });

  const add = useMutation({
    mutationFn: () => api.addTask(hash, draft, unclear),
    onSuccess: () => {
      setDraft("");
      setUnclear(false);
      refreshTasks();
    },
  });
  const cycle = useMutation({
    mutationFn: (t: Task) => api.setTaskStatus(hash, t.id!, NEXT[t.status]),
    onSuccess: refreshTasks,
  });
  const setDue = useMutation({
    mutationFn: (v: { id: string; date: string | null }) => api.setTaskDue(hash, v.id, v.date),
    onSuccess: refreshTasks,
  });
  const setNote = useMutation({
    mutationFn: (v: { id: string; note: string | null }) => api.setTaskNote(hash, v.id, v.note),
    onSuccess: refreshTasks,
  });
  const remove = useMutation({ mutationFn: (id: string) => api.removeTask(hash, id), onSuccess: refreshTasks });
  const attribute = useMutation({
    mutationFn: (v: { id: string; sha: string }) => api.attributeCommit(hash, v.id, v.sha),
    onSuccess: refreshTasks,
  });
  const unattribute = useMutation({
    mutationFn: (v: { id: string; sha: string }) => api.unattributeCommit(hash, v.id, v.sha),
    onSuccess: refreshTasks,
  });

  const tasks = (tasksQ.data ?? []).filter((t) => t.id);
  const commits = commitsQ.data ?? [];

  return (
    <div className="min-h-full max-w-5xl mx-auto px-6 py-6">
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

      <div className="grid gap-6 [grid-template-columns:1fr] md:[grid-template-columns:1fr_320px]">
        {/* Intent: the next-action list */}
        <section>
          <h2 className="text-xs uppercase tracking-wide text-[var(--color-muted)] mb-2">next-actions (intent)</h2>
          {tasksQ.isLoading && <p className="text-[var(--color-muted)]">reading notes/next.md…</p>}
          {tasksQ.isError && <p className="text-[var(--color-flag)]">could not read tasks</p>}

          <ul className="flex flex-col gap-1.5 mb-4">
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
                <span className="flex-1 min-w-0">
                  <span
                    className={
                      t.status === "done" ? "line-through text-[var(--color-muted)]" : "text-[var(--color-fg)]"
                    }
                  >
                    {t.unclear && (
                      <span className="text-[var(--color-warm)]" title="未成形">
                        ?{" "}
                      </span>
                    )}
                    {t.text}
                  </span>
                  {t.commits.length > 0 && (
                    <span className="flex flex-wrap gap-1 mt-1">
                      {t.commits.map((sha) => (
                        <button
                          key={sha}
                          onClick={() => unattribute.mutate({ id: t.id!, sha })}
                          title="click to unlink this commit"
                          className="font-mono text-[10px] rounded bg-[var(--color-ink)] border border-[var(--color-edge)] px-1 text-[var(--color-accent)] hover:text-[var(--color-flag)]"
                        >
                          {sha} ×
                        </button>
                      ))}
                    </span>
                  )}
                  <input
                    key={t.note ?? ""}
                    defaultValue={t.note ?? ""}
                    placeholder="+ problem note…"
                    onBlur={(e) => {
                      const v = e.target.value.trim();
                      if (v !== (t.note ?? "")) setNote.mutate({ id: t.id!, note: v || null });
                    }}
                    className="block w-full mt-1 bg-transparent text-[11px] text-[var(--color-warm)] placeholder:text-[var(--color-muted)] border-none focus:outline-none"
                  />
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
            {tasksQ.data && tasks.length === 0 && (
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
        </section>

        {/* Actual: the git commit timeline */}
        <section>
          <h2 className="text-xs uppercase tracking-wide text-[var(--color-muted)] mb-2">git timeline (actual)</h2>
          {commitsQ.isLoading && <p className="text-[var(--color-muted)] text-sm">reading git log…</p>}
          <ul className="flex flex-col gap-1.5">
            {commits.map((c) => (
              <li key={c.hash} className="rounded border border-[var(--color-edge)] bg-[var(--color-panel)] px-2.5 py-1.5">
                <div className="flex items-baseline gap-2">
                  <span className="font-mono text-[11px] text-[var(--color-accent)]">{c.short}</span>
                  <span className="text-[10px] text-[var(--color-muted)]">{c.date}</span>
                </div>
                <div className="text-xs text-[var(--color-fg)] truncate" title={c.subject}>
                  {c.subject}
                </div>
                {tasks.length > 0 && (
                  <select
                    value=""
                    onChange={(e) => {
                      if (e.target.value) attribute.mutate({ id: e.target.value, sha: c.short });
                    }}
                    className="mt-1 w-full text-[10px] bg-[var(--color-ink)] border border-[var(--color-edge)] rounded px-1 py-0.5 text-[var(--color-muted)]"
                  >
                    <option value="">attribute to task…</option>
                    {tasks.map((t) => (
                      <option key={t.id} value={t.id!}>
                        {t.text.slice(0, 40)}
                      </option>
                    ))}
                  </select>
                )}
              </li>
            ))}
            {commitsQ.data && commits.length === 0 && (
              <li className="text-[var(--color-muted)] text-sm">no commits (or not a git repo).</li>
            )}
          </ul>
        </section>
      </div>
    </div>
  );
}
