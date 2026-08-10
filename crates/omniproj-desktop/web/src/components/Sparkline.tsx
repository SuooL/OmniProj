// A neutral 16-week commit sparkline — activity shape only, no trend line or judgement
// (charter §5 原则3, §8 护栏 ii: facts, not scores). Colored by the project's decay tone;
// the four most recent weeks read at full strength, older weeks fade back.

export function Sparkline({ weeks, color = "var(--color-accent)" }: { weeks: number[]; color?: string }) {
  const max = Math.max(1, ...weeks);
  const n = weeks.length;
  return (
    <div
      className="flex items-end gap-[2px] h-6"
      title={`commits per week, last ${n} weeks (oldest → newest)`}
    >
      {weeks.map((w, i) => {
        const h = w === 0 ? 2 : Math.max(3, Math.round((24 * w) / max));
        const recent = i >= n - 4;
        return (
          <div
            key={i}
            className="flex-1 rounded-[1px]"
            style={{
              height: `${h}px`,
              background: w === 0 ? "var(--color-edge)" : color,
              opacity: w === 0 ? 0.6 : recent ? 1 : 0.5,
            }}
          />
        );
      })}
    </div>
  );
}
