import { useI18n } from "../../i18n/I18nProvider";

export function ActivitySparkline({ weeks }: { weeks: number[] }) {
  const { t } = useI18n();
  const normalized = [...weeks.slice(-16)];
  while (normalized.length < 16) normalized.unshift(0);
  const max = Math.max(1, ...normalized);
  const total = normalized.reduce((sum, count) => sum + count, 0);
  const summary = t("activity.summary", { total });

  return (
    <span
      className="op-activity-sparkline"
      data-testid="activity-sparkline"
      role="img"
      aria-label={summary}
      title={summary}
    >
      {normalized.map((count, index) => (
        <span
          key={index}
          className="op-activity-sparkline__bar"
          data-empty={count === 0 ? "true" : undefined}
          style={{ height: `${count === 0 ? 2 : Math.max(3, Math.round((14 * count) / max))}px` }}
        />
      ))}
    </span>
  );
}
