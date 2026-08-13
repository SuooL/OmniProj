// Decay is the product's core signal (charter §8: sort by staleness, thresholds visible,
// never a health score). We derive a neutral idle fact from the commit histogram: how many
// trailing weeks have zero commits. Color + label encode it; the legend states the cutoffs.

export type Tone = "fresh" | "warm" | "cold" | "none";

export interface Staleness {
  idleWeeks: number;
  total: number;
  label: string;
  tone: Tone;
}

export function staleness(weeks: number[]): Staleness {
  const total = weeks.reduce((a, b) => a + b, 0);
  if (total === 0) return { idleWeeks: 999, total, label: "no commits yet", tone: "none" };
  let idle = 0;
  for (let i = weeks.length - 1; i >= 0; i--) {
    if (weeks[i] === 0) idle++;
    else break;
  }
  const label =
    idle === 0 ? "active this week" : idle === 1 ? "idle 1 week" : `idle ${idle} weeks`;
  const tone: Tone = idle <= 1 ? "fresh" : idle <= 4 ? "warm" : "cold";
  return { idleWeeks: idle, total, label, tone };
}

export const TONE_COLOR: Record<Tone, string> = {
  fresh: "var(--color-active)",
  warm: "var(--color-warm)",
  cold: "var(--color-cold)",
  none: "var(--color-dim)",
};
