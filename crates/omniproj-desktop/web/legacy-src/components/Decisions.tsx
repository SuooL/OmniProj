import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type DecisionStatus } from "../api";

// Plan / decision log (Record layer, M4): append-only, light ADR. Records what you chose —
// including "decided NOT to" (abandoned, marked not deleted; charter §7). User ground truth.

const STATUSES: DecisionStatus[] = ["planned", "doing", "done", "abandoned"];
const STATUS_COLOR: Record<DecisionStatus, string> = {
  planned: "var(--color-muted)",
  doing: "var(--color-accent)",
  done: "var(--color-active)",
  abandoned: "var(--color-dim)",
};

export function Decisions({ hash }: { hash: string }) {
  const qc = useQueryClient();
  const key = ["plan", hash];
  const { data } = useQuery({ queryKey: key, queryFn: () => api.plan(hash) });
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [open, setOpen] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: key });

  const add = useMutation({
    mutationFn: () => api.addDecision(hash, title, body),
    onSuccess: () => {
      setTitle("");
      setBody("");
      setOpen(false);
      refresh();
    },
  });
  const setStatus = useMutation({
    mutationFn: (v: { id: string; status: DecisionStatus }) =>
      api.setDecisionStatus(hash, v.id, v.status),
    onSuccess: refresh,
  });

  const entries = data ?? [];

  return (
    <section className="mt-8">
      <div className="mb-2 flex items-center gap-3">
        <h2 className="text-xs uppercase tracking-wide text-[var(--color-muted)]">plan · decisions</h2>
        <button
          onClick={() => setOpen((o) => !o)}
          className="rounded border border-[var(--color-edge)] px-2 py-0.5 text-[11px] text-[var(--color-fg)] hover:bg-[var(--color-raised)]"
        >
          + decision
        </button>
      </div>

      {open && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (title.trim()) add.mutate();
          }}
          className="mb-3 flex flex-col gap-2 rounded border border-[var(--color-edge)] bg-[var(--color-panel)] p-3"
        >
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="the decision — e.g. “decided NOT to add a plugin system”"
            className="rounded border border-[var(--color-edge)] bg-[var(--color-ink)] px-2 py-1 text-sm text-[var(--color-fg)] placeholder:text-[var(--color-muted)]"
          />
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder="rationale…"
            rows={2}
            className="rounded border border-[var(--color-edge)] bg-[var(--color-ink)] px-2 py-1 text-xs text-[var(--color-fg)] placeholder:text-[var(--color-muted)]"
          />
          <div>
            <button
              type="submit"
              disabled={!title.trim() || add.isPending}
              className="rounded border border-[var(--color-edge)] px-3 py-1 text-xs text-[var(--color-fg)] hover:bg-[var(--color-raised)] disabled:opacity-50"
            >
              record
            </button>
          </div>
        </form>
      )}

      <ul className="flex flex-col gap-1.5">
        {entries.map((d) => (
          <li key={d.id} className="rounded border border-[var(--color-edge)] bg-[var(--color-panel)] px-3 py-2">
            <div className="flex items-center gap-2">
              <span className="shrink-0 font-mono text-[10px] text-[var(--color-dim)]">{d.date}</span>
              <span
                className={
                  d.status === "abandoned"
                    ? "text-sm text-[var(--color-dim)] line-through"
                    : "text-sm text-[var(--color-fg)]"
                }
              >
                {d.title}
              </span>
              {d.commit && (
                <span className="shrink-0 font-mono text-[10px] text-[var(--color-accent)]">{d.commit}</span>
              )}
              <select
                value={d.status}
                onChange={(e) => setStatus.mutate({ id: d.id!, status: e.target.value as DecisionStatus })}
                style={{ color: STATUS_COLOR[d.status] }}
                className="ml-auto shrink-0 rounded border border-[var(--color-edge)] bg-[var(--color-ink)] px-1 py-0.5 text-[10px]"
              >
                {STATUSES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </div>
            {d.body && <p className="mt-1 whitespace-pre-wrap text-xs text-[var(--color-muted)]">{d.body}</p>}
          </li>
        ))}
        {data && entries.length === 0 && (
          <li className="text-sm text-[var(--color-muted)]">
            No decisions logged yet — record what you chose, and what you decided not to do.
          </li>
        )}
      </ul>
    </section>
  );
}
