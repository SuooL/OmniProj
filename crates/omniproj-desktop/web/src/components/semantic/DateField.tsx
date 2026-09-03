// A real date control, not a text box that demands the user type "YYYY-MM-DD".
//
// Two ways in, because both are how people actually set a due date:
//   - the native picker (a calendar, keyboard-editable, localized by the OS);
//   - one-tap relative presets, which is how a due date is usually *thought about*
//     ("by Friday", "tomorrow") rather than as a calendar coordinate.
//
// The value stays the canonical YYYY-MM-DD the backend validates, so nothing downstream
// has to know a picker exists.

import { useI18n } from "../../i18n/I18nProvider";

export interface DateFieldProps {
  value: string;
  onChange: (next: string) => void;
  /** Accessible name; the visible label is supplied by the surrounding field. */
  ariaLabel: string;
  /** Today as YYYY-MM-DD in the user's local calendar. */
  today: string;
}

function shift(dateIso: string, days: number): string {
  return new Date(Date.parse(`${dateIso}T00:00:00Z`) + days * 86_400_000).toISOString().slice(0, 10);
}

/** Days from `today` to the next occurrence of `weekday` (1=Mon..7=Sun); never 0. */
function daysUntilWeekday(today: string, weekday: number): number {
  const current = new Date(`${today}T00:00:00Z`).getUTCDay() || 7;
  return ((weekday - current + 7) % 7) || 7;
}

export function DateField({ value, onChange, ariaLabel, today }: DateFieldProps) {
  const { t } = useI18n();

  const presets: Array<{ key: string; label: string; value: string }> = [
    { key: "today", label: t("date.today"), value: today },
    { key: "tomorrow", label: t("date.tomorrow"), value: shift(today, 1) },
    { key: "friday", label: t("date.friday"), value: shift(today, daysUntilWeekday(today, 5)) },
    { key: "nextMonday", label: t("date.nextMonday"), value: shift(today, daysUntilWeekday(today, 1)) },
  ];

  return (
    <div className="op-datefield">
      <input
        type="date"
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      <div className="op-datefield__presets">
        {presets.map((preset) => (
          <button
            key={preset.key}
            type="button"
            className="op-chip op-chip--action"
            aria-pressed={value === preset.value}
            title={preset.value}
            onClick={() => onChange(preset.value)}
          >
            {preset.label}
          </button>
        ))}
        {value && (
          <button
            type="button"
            className="op-chip op-chip--action"
            onClick={() => onChange("")}
          >
            {t("date.clear")}
          </button>
        )}
      </div>
    </div>
  );
}
