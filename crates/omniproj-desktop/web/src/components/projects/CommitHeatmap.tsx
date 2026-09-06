// A year of commits as a calendar grid — the one picture that answers "did I actually
// touch this project, and when did it go quiet" without reading a single number.
//
// The backend sends daily counts oldest → newest plus the date of the last bucket. The
// grid is laid out from that date backwards so every column is a real Sunday-to-Saturday
// week: a naive 7-per-column chunk drifts off the calendar and the columns stop meaning
// anything.

import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import { api } from "../../api";
import type { ProjectId } from "../../domain/project";
import { useI18n, type Locale } from "../../i18n/I18nProvider";

const DAY_MS = 86_400_000;
/** Rows are weekdays, Sunday first, matching `Date.getUTCDay()`. */
const ROWS = 7;

interface Cell {
  /** `YYYY-MM-DD`. */
  date: string;
  count: number;
  /** 0 = no commits; 1–4 = quartile of the project's own active days. */
  level: 0 | 1 | 2 | 3 | 4;
  column: number;
  row: number;
}

/**
 * Quartile thresholds over the *active* days only. Scaling against the maximum instead
 * would let one 40-commit day flatten an entire ordinary year into the palest step.
 */
function thresholds(counts: number[]): [number, number, number] {
  const active = counts.filter((count) => count > 0).sort((a, b) => a - b);
  if (active.length === 0) return [1, 2, 3];
  const at = (fraction: number) =>
    active[Math.min(active.length - 1, Math.floor(active.length * fraction))];
  return [at(0.25), at(0.5), at(0.75)];
}

function levelFor(count: number, [low, mid, high]: [number, number, number]): Cell["level"] {
  if (count <= 0) return 0;
  if (count <= low) return 1;
  if (count <= mid) return 2;
  if (count <= high) return 3;
  return 4;
}

/**
 * Place each daily count on the calendar. Dates are handled in UTC throughout: the
 * backend already resolved each commit to a local calendar day, so re-interpreting the
 * resulting `YYYY-MM-DD` in the browser's zone would shift it a second time.
 */
function layout(days: number[] | undefined, lastDay: string): { cells: Cell[]; columns: number } {
  const last = Date.parse(`${lastDay}T00:00:00Z`);
  // A graphic is never worth taking the page down with it: an absent or malformed
  // payload renders as no heatmap, and the steps below still load.
  if (!Array.isArray(days) || days.length === 0 || Number.isNaN(last)) {
    return { cells: [], columns: 0 };
  }

  const lastWeekday = new Date(last).getUTCDay();
  // Days left in the final column after `lastDay`, so column boundaries fall on weeks.
  const tail = 6 - lastWeekday;
  const columns = Math.floor((days.length - 1 + tail) / ROWS) + 1;
  const scale = thresholds(days);

  const cells = days.map((count, index) => {
    const age = days.length - 1 - index;
    const date = new Date(last - age * DAY_MS);
    return {
      date: date.toISOString().slice(0, 10),
      count,
      level: levelFor(count, scale),
      row: date.getUTCDay() + 1,
      column: columns - Math.floor((age + tail) / ROWS),
    };
  });
  return { cells, columns };
}

/** The column each month starts in, for the labels above the grid. */
function monthLabels(cells: Cell[], locale: Locale): Array<{ column: number; label: string }> {
  const formatter = new Intl.DateTimeFormat(locale, { month: "short", timeZone: "UTC" });
  const labels: Array<{ column: number; label: string }> = [];
  let previous = "";
  for (const cell of cells) {
    const month = cell.date.slice(0, 7);
    if (month === previous) continue;
    previous = month;
    // A month whose first visible day sits mid-column would print over its neighbour.
    if (labels.length > 0 && cell.column - labels[labels.length - 1].column < 3) continue;
    labels.push({
      column: cell.column,
      label: formatter.format(new Date(`${cell.date}T00:00:00Z`)),
    });
  }
  return labels;
}

export interface CommitHeatmapProps {
  projectId: ProjectId;
}

export function CommitHeatmap({ projectId }: CommitHeatmapProps) {
  const { locale, t } = useI18n();
  const { data, isLoading, isError } = useQuery({
    queryKey: ["commit-heatmap", projectId],
    queryFn: () => api.getCommitHeatmap(projectId),
    // A year of `git log` is cheap but not free, and it only moves when a commit lands.
    staleTime: 5 * 60_000,
  });

  const { cells, columns } = useMemo(
    () => (data ? layout(data.days, data.last_day ?? "") : { cells: [], columns: 0 }),
    [data],
  );
  const months = useMemo(() => monthLabels(cells, locale), [cells, locale]);

  if (isLoading) return <p className="op-muted">{t("heatmap.loading")}</p>;
  // No repository, or an unreadable one. The list below is the point of the page; a
  // missing picture must not become an error banner in front of it.
  if (isError || cells.length === 0) return null;

  const total = cells.reduce((sum, cell) => sum + cell.count, 0);
  const activeDays = cells.filter((cell) => cell.count > 0).length;
  const dateFormat = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });

  return (
    <figure className="op-heatmap" data-testid="commit-heatmap">
      <div
        className="op-heatmap__grid"
        role="img"
        aria-label={t("heatmap.summary", { total, days: activeDays })}
        style={{ gridTemplateColumns: `repeat(${columns}, var(--op-heatmap-cell))` }}
      >
        {months.map((month) => (
          <span
            key={`${month.column}-${month.label}`}
            className="op-heatmap__month"
            style={{ gridColumn: month.column, gridRow: 1 }}
            aria-hidden="true"
          >
            {month.label}
          </span>
        ))}
        {cells.map((cell) => (
          <span
            key={cell.date}
            className="op-heatmap__cell"
            data-level={cell.level}
            style={{ gridColumn: cell.column, gridRow: cell.row + 1 }}
            // Native tooltip: no hover state to manage, and it survives keyboard-free use.
            title={t("heatmap.cell", {
              count: cell.count,
              date: dateFormat.format(new Date(`${cell.date}T00:00:00Z`)),
            })}
          />
        ))}
      </div>
      <figcaption className="op-heatmap__caption">
        <span>{t("heatmap.summary", { total, days: activeDays })}</span>
        <span className="op-heatmap__legend" aria-hidden="true">
          {t("heatmap.less")}
          {[0, 1, 2, 3, 4].map((level) => (
            <span key={level} className="op-heatmap__cell" data-level={level} />
          ))}
          {t("heatmap.more")}
        </span>
      </figcaption>
    </figure>
  );
}
