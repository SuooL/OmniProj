// A real tag control, not a comma-separated text box.
//
// The old field made the user retype a tag they had already used elsewhere in the project,
// with no way to see or pick what existed. Tags are a small, reused vocabulary, so the
// control remembers it: every tag already used in this project is offered, filtered as you
// type, and one click away.
//
// Interaction contract:
//   - Enter or comma commits the typed tag;
//   - Backspace on an empty input removes the last chip (standard token-field behaviour);
//   - each chip has its own remove control, so it works by mouse and by keyboard;
//   - suggestions exclude what is already applied and cap at a readable number;
//   - normalization (trim, case-insensitive dedupe, limits) stays in core; this control
//     only prevents the obvious local duplicates so the UI never shows two of the same.

import { useMemo, useRef, useState } from "react";

import { useI18n } from "../../i18n/I18nProvider";

export interface TagFieldProps {
  value: string[];
  onChange: (next: string[]) => void;
  /** Every tag already used in this project, most relevant first. */
  vocabulary: string[];
  ariaLabel: string;
  /** Core's per-item cap; the control stops offering more once it is reached. */
  max?: number;
}

const SUGGESTION_LIMIT = 8;

export function TagField({ value, onChange, vocabulary, ariaLabel, max = 8 }: TagFieldProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const applied = useMemo(() => new Set(value.map((tag) => tag.toLowerCase())), [value]);
  const full = value.length >= max;

  const suggestions = useMemo(() => {
    const needle = draft.trim().toLowerCase();
    return vocabulary
      .filter((tag) => !applied.has(tag.toLowerCase()))
      .filter((tag) => !needle || tag.toLowerCase().includes(needle))
      .slice(0, SUGGESTION_LIMIT);
  }, [applied, draft, vocabulary]);

  function commit(raw: string) {
    const tag = raw.trim();
    if (!tag || full || applied.has(tag.toLowerCase())) {
      setDraft("");
      return;
    }
    onChange([...value, tag]);
    setDraft("");
  }

  function removeAt(index: number) {
    onChange(value.filter((_, i) => i !== index));
    inputRef.current?.focus();
  }

  return (
    <div className="op-tagfield">
      <div className="op-tagfield__box" onClick={() => inputRef.current?.focus()}>
        {value.map((tag, index) => (
          <span key={tag} className="op-tagfield__chip">
            {tag}
            <button
              type="button"
              aria-label={t("tags.remove", { tag })}
              onClick={(event) => { event.stopPropagation(); removeAt(index); }}
            >
              ×
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          className="op-tagfield__input"
          aria-label={ariaLabel}
          placeholder={full ? t("tags.full", { max }) : t("tags.placeholder")}
          disabled={full}
          title={full ? t("tags.full", { max }) : undefined}
          value={draft}
          onChange={(event) => {
            // A typed comma is a commit, matching how people paste lists.
            if (/[,，、]/.test(event.target.value)) commit(event.target.value.replace(/[,，、]/g, ""));
            else setDraft(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") { event.preventDefault(); commit(draft); }
            else if (event.key === "Backspace" && draft === "" && value.length > 0) {
              event.preventDefault();
              removeAt(value.length - 1);
            }
          }}
        />
      </div>
      {suggestions.length > 0 && !full && (
        <div className="op-tagfield__suggestions" role="group" aria-label={t("tags.suggestions")}>
          {suggestions.map((tag) => (
            <button key={tag} type="button" className="op-chip op-chip--action" onClick={() => commit(tag)}>
              {tag}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
