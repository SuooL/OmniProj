import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Settings as S } from "../api";

// Controlled-push knob (charter §4d / §5 原则5): reminder cadence + threshold are
// user-visible, adjustable, and switchable off. A "send test notification" button lets
// the user confirm the OS push path (and grant permission) without waiting a day.

export function Settings({ onBack }: { onBack: () => void }) {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const [form, setForm] = useState<S | null>(null);
  const s = form ?? data ?? null;

  const save = useMutation({
    mutationFn: (v: S) => api.setSettings(v),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["settings"] });
      qc.invalidateQueries({ queryKey: ["attention"] });
    },
  });
  const test = useMutation({ mutationFn: () => api.testReminder() });

  if (!s) return <p className="p-6 text-[var(--color-muted)]">loading…</p>;
  const set = (patch: Partial<S>) => setForm({ ...s, ...patch });

  return (
    <div className="min-h-full max-w-xl mx-auto px-6 py-6">
      <header className="flex items-center gap-3 mb-5">
        <button
          onClick={onBack}
          className="text-xs rounded border border-[var(--color-edge)] px-2.5 py-1 text-[var(--color-fg)] hover:bg-[var(--color-panel)]"
        >
          ← back
        </button>
        <h1 className="text-xl font-semibold tracking-tight">Reminders</h1>
      </header>

      <p className="text-xs text-[var(--color-muted)] mb-5">
        Controlled push (charter §4d): the cadence and threshold are yours to see, adjust, and turn
        off. A project with no commit within the threshold "needs attention".
      </p>

      <div className="flex flex-col gap-4">
        <label className="flex items-center gap-2 text-sm text-[var(--color-fg)]">
          <input
            type="checkbox"
            checked={s.reminders_enabled}
            onChange={(e) => set({ reminders_enabled: e.target.checked })}
          />
          Daily reminders enabled
        </label>
        <label className="flex items-center justify-between text-sm text-[var(--color-fg)]">
          <span>Silence threshold (days)</span>
          <input
            type="number"
            min={0}
            value={s.silence_days}
            onChange={(e) => set({ silence_days: Math.max(0, Number(e.target.value)) })}
            className="w-20 bg-[var(--color-panel)] border border-[var(--color-edge)] rounded px-2 py-1"
          />
        </label>
        <label className="flex items-center justify-between text-sm text-[var(--color-fg)]">
          <span>Check interval (hours)</span>
          <input
            type="number"
            min={1}
            value={s.interval_hours}
            onChange={(e) => set({ interval_hours: Math.max(1, Number(e.target.value)) })}
            className="w-20 bg-[var(--color-panel)] border border-[var(--color-edge)] rounded px-2 py-1"
          />
        </label>

        <div className="flex items-center gap-3 mt-2">
          <button
            onClick={() => save.mutate(s)}
            disabled={save.isPending}
            className="text-xs rounded border border-[var(--color-edge)] px-3 py-1.5 text-[var(--color-fg)] hover:bg-[var(--color-panel)] disabled:opacity-50"
          >
            save
          </button>
          <button
            onClick={() => test.mutate()}
            className="text-xs rounded border border-[var(--color-edge)] px-3 py-1.5 text-[var(--color-fg)] hover:bg-[var(--color-panel)]"
          >
            send test notification
          </button>
          {save.isSuccess && <span className="text-[var(--color-active)] text-xs">saved</span>}
        </div>
      </div>
    </div>
  );
}
