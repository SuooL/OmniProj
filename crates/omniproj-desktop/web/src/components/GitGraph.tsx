import type { GraphCommit, Task } from "../api";

// The branch-aware flow graph (M4) — the *actual* line, and the canvas task↔commit
// reconciliation happens on (charter §3). Lanes are assigned with the standard
// commit-graph sweep (newest → oldest); edges connect each commit to its parents. This is
// deliberately a compact reconciliation view, not a gitk clone (§6 non-goal).

const R = 30; // row height (px)
const G = 15; // lane gap
const PAD = 11;
const LANES = ["#4cc9f0", "#46d67f", "#f0b429", "#c792ea", "#f2635a", "#7bd4ff"];

interface Node {
  c: GraphCommit;
  col: number;
  parentCols: number[];
}

function layout(commits: GraphCommit[]): { nodes: Node[]; width: number } {
  const lanes: (string | null)[] = []; // lane -> the hash it's currently waiting for
  const nodes: Node[] = [];
  let width = 1;
  const freeLane = () => {
    const i = lanes.indexOf(null);
    if (i !== -1) return i;
    lanes.push(null);
    return lanes.length - 1;
  };
  for (const c of commits) {
    let col = lanes.indexOf(c.hash);
    if (col === -1) col = freeLane();
    // other lanes awaiting this same commit converge here
    for (let i = 0; i < lanes.length; i++) if (i !== col && lanes[i] === c.hash) lanes[i] = null;
    const parentCols: number[] = [];
    if (c.parents.length === 0) {
      lanes[col] = null; // root frees its lane
    } else {
      c.parents.forEach((p, idx) => {
        if (idx === 0) {
          lanes[col] = p;
          parentCols.push(col);
        } else {
          let pc = lanes.indexOf(p);
          if (pc === -1) {
            pc = freeLane();
            lanes[pc] = p;
          }
          parentCols.push(pc);
        }
      });
    }
    width = Math.max(width, lanes.length, col + 1, ...parentCols.map((x) => x + 1));
    nodes.push({ c, col, parentCols });
  }
  return { nodes, width };
}

function RefBadge({ label }: { label: string }) {
  const head = label === "HEAD";
  return (
    <span
      className="shrink-0 rounded px-1 font-mono text-[9px] leading-4"
      style={{
        color: head ? "var(--color-ink)" : "var(--color-accent)",
        background: head ? "var(--color-accent)" : "transparent",
        border: head ? "none" : "1px solid var(--color-edge)",
      }}
    >
      {label}
    </span>
  );
}

export function GitGraph({
  commits,
  tasks,
  onAttribute,
}: {
  commits: GraphCommit[];
  tasks: Task[];
  onAttribute: (taskId: string, sha: string) => void;
}) {
  const { nodes, width } = layout(commits);
  const idx = new Map(commits.map((c, i) => [c.hash, i]));
  const graphW = PAD * 2 + (width - 1) * G;
  const totalH = commits.length * R;
  const cx = (col: number) => PAD + col * G;
  const cy = (row: number) => row * R + R / 2;
  const lane = (col: number) => LANES[col % LANES.length];

  return (
    <div className="flex">
      <svg width={graphW} height={totalH} className="shrink-0" aria-hidden>
        {nodes.map((n, i) =>
          n.c.parents.map((p, pi) => {
            const col = n.parentCols[pi];
            const fx = cx(n.col);
            const fy = cy(i);
            const to = idx.get(p);
            const tx = cx(col);
            const ty = to === undefined ? totalH : cy(to);
            const d = `M ${fx} ${fy} C ${tx} ${fy + R * 0.5}, ${tx} ${ty - R * 0.5}, ${tx} ${ty}`;
            return <path key={p + pi} d={d} fill="none" stroke={lane(col)} strokeWidth={1.75} opacity={0.85} />;
          }),
        )}
        {nodes.map((n, i) => (
          <circle
            key={n.c.hash}
            cx={cx(n.col)}
            cy={cy(i)}
            r={n.c.parents.length > 1 ? 5 : 4}
            fill={lane(n.col)}
            stroke="var(--color-panel)"
            strokeWidth={2}
          />
        ))}
      </svg>

      <ul className="min-w-0 flex-1">
        {commits.map((c) => (
          <li
            key={c.hash}
            style={{ height: R }}
            className="flex min-w-0 items-center gap-2 border-b border-[var(--color-edge)]"
          >
            <span className="shrink-0 font-mono text-[11px] text-[var(--color-accent)]">{c.short}</span>
            {c.refs.map((r) => (
              <RefBadge key={r} label={r} />
            ))}
            <span className="min-w-0 flex-1 truncate text-xs text-[var(--color-fg)]" title={c.subject}>
              {c.subject}
            </span>
            <span className="shrink-0 font-mono text-[10px] text-[var(--color-dim)]">{c.date}</span>
            {tasks.length > 0 && (
              <select
                value=""
                title="attribute this commit to a task"
                onChange={(e) => {
                  if (e.target.value) onAttribute(e.target.value, c.short);
                }}
                className="shrink-0 rounded border border-[var(--color-edge)] bg-[var(--color-ink)] px-1 py-0.5 text-[10px] text-[var(--color-muted)]"
              >
                <option value="">→ task</option>
                {tasks.map((t) => (
                  <option key={t.id} value={t.id!}>
                    {t.text.slice(0, 32)}
                  </option>
                ))}
              </select>
            )}
          </li>
        ))}
        {commits.length === 0 && (
          <li className="text-sm text-[var(--color-muted)]">no commits (or not a git repo).</li>
        )}
      </ul>
    </div>
  );
}
