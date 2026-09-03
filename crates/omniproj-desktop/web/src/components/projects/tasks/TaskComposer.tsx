// The single way a task enters the list.

import { useI18n } from "../../../i18n/I18nProvider";

export interface TaskComposerProps {
  text: string;
  unclear: boolean;
  disabled: boolean;
  onTextChange: (text: string) => void;
  onUnclearChange: (unclear: boolean) => void;
  onSubmit: () => void;
}

export function TaskComposer({
  text,
  unclear,
  disabled,
  onTextChange,
  onUnclearChange,
  onSubmit,
}: TaskComposerProps) {
  const { t } = useI18n();
  return (
    <div className="op-task-add">
      <input
        aria-label={t("task.new")}
        placeholder={t("task.new")}
        value={text}
        onChange={(event) => onTextChange(event.target.value)}
        onKeyDown={(event) => { if (event.key === "Enter") onSubmit(); }}
      />
      <label>
        <input type="checkbox" checked={unclear} onChange={(event) => onUnclearChange(event.target.checked)} />
        {" "}{t("task.unclear")}
      </label>
      <button
        className="op-button op-button--primary"
        type="button"
        disabled={disabled || text.trim() === ""}
        title={text.trim() === "" ? t("task.addDisabled") : undefined}
        onClick={onSubmit}
      >
        {t("task.add")}
      </button>
    </div>
  );
}
