// A neutral 16-week commit sparkline. It shows activity shape, nothing more — no
// trend line, no judgement (charter §5 原则3, §8 护栏 ii: facts, not scores).

export function Sparkline({ weeks }: { weeks: number[] }) {
  const max = Math.max(1, ...weeks);
  return (
    <div
      className="flex items-end gap-[2px] h-7"
      title={`commits per week, last ${weeks.length} weeks (oldest → newest)`}
    >
      {weeks.map((w, i) => {
        const h = w === 0 ? 2 : Math.max(3, Math.round((28 * w) / max));
        return (
          <div
            key={i}
            className="flex-1 rounded-[1px]"
            style={{
              height: `${h}px`,
              background: w === 0 ? "var(--color-edge)" : "var(--color-accent)",
              opacity: w === 0 ? 0.5 : 1,
            }}
          />
        );
      })}
    </div>
  );
}
