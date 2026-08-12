import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Task, type TaskStatus } from "../api";
import { GitGraph } from "./GitGraph";
import { Decisions } from "./Decisions";

// Record layer (M2): the project's next-action list (intent) beside its git commit
// timeline (actual). User ground truth — every edit is an explicit action the backend
// versions as one revertable store commit. Status cycles open → doing → done on click;
// `[/]`/due/commit-attributions round-trip in next.md. Commits attribute many-to-one.

const NEXT: Record<TaskStatus, TaskStatus> = { open: "doing", doing: "done", done: "open" };
const GLYPH: Record<TaskStatus, string> = { open: "○", doing: "◐", done: "●" };
const GLYPH_COLOR: Record<TaskStatus, string> = {
  open: "var(--color-dim)",
  doing: "var(--color-accent)",
  done: "var(--color-active)",
};

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
  const graphQ = useQuery({ queryKey: ["graph", hash], queryFn: () => api.graph(hash, 40) });
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

  // Advance (M3/FR-V1): agent proposes concrete sub-steps; the user picks which to adopt.
  const [advancing, setAdvancing] = useState<
    { id: string; candidates: string[]; selected: boolean[] } | null
  >(null);
  const advance = useMutation({
    mutationFn: (id: string) => api.advanceTask(hash, id),
    onSuccess: (candidates, id) =>
      setAdvancing({ id, candidates, selected: candidates.map(() => true) }),
  });
  const adopt = useMutation({
    mutationFn: (texts: string[]) => api.adoptSubtasks(hash, texts),
    onSuccess: () => {
      setAdvancing(null);
      refreshTasks();
    },
  });

  // Advance extensions (M5): clarify (FR-V3 adversarial questioning) and refine (FR-V2
  // repo-grounded spec). Both are proposals in auto/ — the conclusion/spec is the user's.
  const [assist, setAssist] = useState<{ id: string; kind: "clarify" | "refine" } | null>(null);
  const [thought, setThought] = useState("");
  const [spec, setSpec] = useState<string | null>(null);
  const clarifyQ = useQuery({
    queryKey: ["clarify", hash, assist?.id],
    queryFn: () => api.getClarify(hash, assist!.id),
    enabled: !!assist && assist.kind === "clarify",
  });
  const clarify = useMutation({
    mutationFn: (v: { id: string; message?: string }) => api.clarifyTask(hash, v.id, v.message),
    onSuccess: (_r, v) => {
      setThought("");
      qc.invalidateQueries({ queryKey: ["clarify", hash, v.id] });
    },
  });
  const refine = useMutation({
    mutationFn: (id: string) => api.refineTask(hash, id),
    onMutate: () => setSpec(null),
    onSuccess: (s) => setSpec(s),
  });

  const tasks = (tasksQ.data ?? []).filter((t) => t.id);

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
                className="rounded border border-[var(--color-edge)] bg-[var(--color-panel)]"
              >
               <div className="flex items-center gap-3 px-3 py-2">
                <button
                  title={`status: ${t.status} (click to cycle)`}
                  onClick={() => cycle.mutate(t)}
                  style={{ color: GLYPH_COLOR[t.status] }}
                  className="w-6 text-lg leading-none"
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
                  onClick={() => advance.mutate(t.id!)}
                  disabled={advance.isPending}
                  title="Advance: break this into concrete steps (a proposal)"
                  className="px-1 text-sm text-[var(--color-accent)] hover:text-[var(--color-fg)] disabled:opacity-50"
                >
                  {advance.isPending && advance.variables === t.id ? "…" : "✨"}
                </button>
                <button
                  onClick={() =>
                    setAssist(
                      assist?.id === t.id && assist.kind === "clarify"
                        ? null
                        : { id: t.id!, kind: "clarify" },
                    )
                  }
                  title="Clarify: adversarial questions to think it through"
                  className="px-1 text-sm hover:text-[var(--color-fg)] text-[var(--color-muted)]"
                >
                  💬
                </button>
                <button
                  onClick={() => {
                    setAssist({ id: t.id!, kind: "refine" });
                    setSpec(null);
                    refine.mutate(t.id!);
                  }}
                  disabled={refine.isPending}
                  title="Refine: draft a grounded spec from this idea"
                  className="px-1 text-sm hover:text-[var(--color-fg)] text-[var(--color-muted)] disabled:opacity-50"
                >
                  📋
                </button>
                <button
                  onClick={() => remove.mutate(t.id!)}
                  title="delete"
                  className="text-[var(--color-muted)] hover:text-[var(--color-flag)] px-1"
                >
                  ×
                </button>
               </div>

               {advance.isError && advance.variables === t.id && (
                 <p className="text-[var(--color-flag)] text-[11px] px-3 pb-2">{String(advance.error)}</p>
               )}
               {advancing?.id === t.id && (
                 <div className="border-t border-[var(--color-edge)] px-3 py-2">
                   <div className="text-[11px] text-[var(--color-muted)] mb-1.5">
                     agent proposal — a suggestion, not a decision. Pick which to adopt as tasks:
                   </div>
                   {advancing.candidates.map((c, i) => (
                     <label key={i} className="flex items-start gap-2 text-xs text-[var(--color-fg)] py-0.5">
                       <input
                         type="checkbox"
                         checked={advancing.selected[i]}
                         onChange={(e) => {
                           const selected = [...advancing.selected];
                           selected[i] = e.target.checked;
                           setAdvancing({ ...advancing, selected });
                         }}
                       />
                       <span>{c}</span>
                     </label>
                   ))}
                   <div className="flex gap-2 mt-2">
                     <button
                       onClick={() =>
                         adopt.mutate(advancing.candidates.filter((_, i) => advancing.selected[i]))
                       }
                       disabled={adopt.isPending || !advancing.selected.some(Boolean)}
                       className="text-[11px] rounded border border-[var(--color-edge)] px-2 py-1 text-[var(--color-fg)] hover:bg-[var(--color-panel)] disabled:opacity-50"
                     >
                       adopt selected
                     </button>
                     <button
                       onClick={() => setAdvancing(null)}
                       className="text-[11px] text-[var(--color-muted)] px-2 py-1 hover:text-[var(--color-fg)]"
                     >
                       dismiss
                     </button>
                   </div>
                 </div>
               )}

               {assist?.id === t.id && assist.kind === "clarify" && (
                 <div className="border-t border-[var(--color-edge)] px-3 py-2 text-xs">
                   <div className="mb-1.5 text-[11px] text-[var(--color-muted)]">
                     clarify — questions to think it through, not answers. The conclusion is yours to write.
                   </div>
                   {clarify.isError && (
                     <p className="mb-1 text-[var(--color-flag)]">{String(clarify.error)}</p>
                   )}
                   <div className="max-h-56 overflow-auto whitespace-pre-wrap text-[var(--color-fg)]">
                     {clarifyQ.data?.trim() || "no rounds yet — ask one."}
                   </div>
                   <div className="mt-2 flex gap-2">
                     <input
                       value={thought}
                       onChange={(e) => setThought(e.target.value)}
                       placeholder="a thought to add (optional)…"
                       className="flex-1 rounded border border-[var(--color-edge)] bg-[var(--color-ink)] px-2 py-1 text-[var(--color-fg)] placeholder:text-[var(--color-muted)]"
                     />
                     <button
                       onClick={() => clarify.mutate({ id: t.id!, message: thought || undefined })}
                       disabled={clarify.isPending}
                       className="rounded border border-[var(--color-edge)] px-2 py-1 text-[11px] text-[var(--color-fg)] hover:bg-[var(--color-raised)] disabled:opacity-50"
                     >
                       {clarify.isPending ? "…" : "ask a round"}
                     </button>
                     <button
                       onClick={() => setAssist(null)}
                       className="px-2 py-1 text-[11px] text-[var(--color-muted)] hover:text-[var(--color-fg)]"
                     >
                       close
                     </button>
                   </div>
                 </div>
               )}

               {assist?.id === t.id && assist.kind === "refine" && (
                 <div className="border-t border-[var(--color-edge)] px-3 py-2 text-xs">
                   <div className="mb-1.5 text-[11px] text-[var(--color-muted)]">
                     refined spec (a proposal saved to auto/, grounded in the repo — not web).
                   </div>
                   {refine.isPending && <p className="text-[var(--color-muted)]">refining…</p>}
                   {refine.isError && <p className="text-[var(--color-flag)]">{String(refine.error)}</p>}
                   {spec && (
                     <pre className="max-h-72 overflow-auto whitespace-pre-wrap font-sans text-[var(--color-fg)]">
                       {spec}
                     </pre>
                   )}
                   <button
                     onClick={() => setAssist(null)}
                     className="mt-2 px-2 py-1 text-[11px] text-[var(--color-muted)] hover:text-[var(--color-fg)]"
                   >
                     close
                   </button>
                 </div>
               )}
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

        {/* Actual: the branch-aware git flow graph (the reconciliation canvas) */}
        <section>
          <h2 className="mb-2 text-xs uppercase tracking-wide text-[var(--color-muted)]">git flow graph (actual)</h2>
          {graphQ.isLoading && <p className="text-sm text-[var(--color-muted)]">reading git graph…</p>}
          {graphQ.data && (
            <GitGraph
              commits={graphQ.data}
              tasks={tasks}
              onAttribute={(id, sha) => attribute.mutate({ id, sha })}
            />
          )}
        </section>
      </div>

      <Decisions hash={hash} />
    </div>
  );
}
