// Two jobs, one form.
//
// In `setup` it asks for the first step and nothing else, and completes setup in a single
// atomic write — the project goes from registered to usable in one revision. For a project
// that is already active it edits the optional one-line note under the project name.
//
// It used to demand an "objective" and a "desired outcome" before it would let you write
// anything down. That was a questionnaire in front of a to-do list.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview } from "../../domain/project";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";
import { localizeError, useI18n } from "../../i18n/I18nProvider";

export interface ProjectFramingFormProps {
  overview: ProjectOverview;
}

export function ProjectFramingForm({ overview }: ProjectFramingFormProps) {
  const { locale, t } = useI18n();
  const mutation = useOverviewMutation();
  const isSetup = overview.status === "setup";
  const [note, setNote] = useState(overview.objective ?? "");
  const [firstStep, setFirstStep] = useState("");
  const [failed, setFailed] = useState(false);

  const pid = overview.project_id;
  const rev = overview.revision;

  async function save() {
    setFailed(false);
    const result = isSetup
      ? await mutation.run(
          pid,
          () =>
            api.completeProjectSetup({
              project_id: pid,
              expected_revision: rev,
              objective: note.trim(),
              desired_outcome: "",
              phase: null,
              first_commitment: firstStep.trim(),
            }),
          t("framing.setupSuccess"),
        )
      : await mutation.run(
          pid,
          () =>
            api.saveProjectFraming({
              project_id: pid,
              expected_revision: rev,
              objective: note.trim(),
              desired_outcome: overview.desired_outcome ?? "",
              phase: overview.phase,
            }),
          t("framing.saveSuccess"),
        );
    if (result.status !== "success") setFailed(true);
  }

  const canSubmit = (!isSetup || firstStep.trim() !== "") && !mutation.pending;

  return (
    <section
      className={`op-section op-form-section${isSetup ? " op-section--setup" : ""}`}
      aria-labelledby="framing-heading"
      data-testid="framing-form"
    >
      <div className="op-section__header">
        <h3 id="framing-heading">{isSetup ? t("framing.setupTitle") : t("framing.title")}</h3>
      </div>
      {isSetup && <p className="op-section__intro">{t("framing.setupIntro")}</p>}
      <div className="op-form-grid">
        {isSetup && (
          <label className="op-field op-field--wide">
            <span>{t("framing.firstStep")}</span>
            <input
              aria-label={t("framing.firstStep")}
              autoFocus
              value={firstStep}
              onChange={(event) => setFirstStep(event.target.value)}
            />
          </label>
        )}
        <label className="op-field op-field--wide">
          <span>
            {t("framing.note")} <small>{t("framing.optional")}</small>
          </span>
          <input
            aria-label={t("framing.note")}
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
        </label>
      </div>
      <div className="op-section__footer">
        <button
          className="op-button op-button--primary"
          type="button"
          disabled={!canSubmit}
          onClick={save}
        >
          {isSetup ? t("framing.setupTitle") : t("framing.save")}
        </button>
      </div>
      {failed && mutation.error && (
        <p role="alert" className="op-mutation-error" data-testid="framing-error">
          {mutation.error.recovery === "refetch"
            ? t("framing.conflict")
            : localizeError(mutation.error, locale)}
        </p>
      )}
    </section>
  );
}
