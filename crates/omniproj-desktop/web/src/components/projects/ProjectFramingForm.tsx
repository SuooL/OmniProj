// Framing = the Human's intent: objective, desired outcome, optional phase, and (in setup) the
// first commitment. In `setup` this is ONE atomic `complete_project_setup` that promotes the
// project to active in a single revision — there is no intermediate framing write. For an
// already-active project it is `save_project_framing`. Either way the save is explicit (a
// button, never blur) and the draft survives a failed write.

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
  const [objective, setObjective] = useState(overview.objective ?? "");
  const [desiredOutcome, setDesiredOutcome] = useState(overview.desired_outcome ?? "");
  const [phase, setPhase] = useState(overview.phase ?? "");
  const [firstCommitment, setFirstCommitment] = useState("");
  const [failed, setFailed] = useState(false);

  const pid = overview.project_id;
  const rev = overview.revision;

  async function save() {
    setFailed(false);
    const phaseValue = phase.trim() === "" ? null : phase.trim();
    const result = isSetup
      ? await mutation.run(
          pid,
          () =>
            api.completeProjectSetup({
              project_id: pid,
              expected_revision: rev,
              objective: objective.trim(),
              desired_outcome: desiredOutcome.trim(),
              phase: phaseValue,
              first_commitment: firstCommitment.trim(),
            }),
          t("framing.setupSuccess"),
        )
      : await mutation.run(
          pid,
          () =>
            api.saveProjectFraming({
              project_id: pid,
              expected_revision: rev,
              objective: objective.trim(),
              desired_outcome: desiredOutcome.trim(),
              phase: phaseValue,
            }),
          t("framing.saveSuccess"),
        );
    if (result.status !== "success") setFailed(true);
  }

  const canSubmit =
    objective.trim() !== "" &&
    desiredOutcome.trim() !== "" &&
    (!isSetup || firstCommitment.trim() !== "") &&
    !mutation.pending;

  return (
    <section
      className={`op-section op-form-section${isSetup ? " op-section--setup" : ""}`}
      aria-labelledby="framing-heading"
      data-testid="framing-form"
    >
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("framing.kicker")}</p>
          <h3 id="framing-heading">{isSetup ? t("framing.setupTitle") : t("framing.title")}</h3>
        </div>
      </div>
      {isSetup && (
        <p className="op-section__intro">
          {t("framing.setupIntro")}
        </p>
      )}
      <div className="op-form-grid">
        <label className="op-field op-field--wide">
          <span>{t("framing.objective")}</span>
          {/* Setup focuses the objective first (spec 9.4 order). */}
          <input
            aria-label={t("framing.objective")}
            autoFocus={isSetup}
            value={objective}
            onChange={(e) => setObjective(e.target.value)}
          />
        </label>
        <label className="op-field op-field--wide">
          <span>{t("framing.desiredOutcome")}</span>
          <input
            aria-label={t("framing.desiredOutcome")}
            value={desiredOutcome}
            onChange={(e) => setDesiredOutcome(e.target.value)}
          />
        </label>
        <label className="op-field">
          <span>
            {t("framing.phase")} <small>{t("framing.optional")}</small>
          </span>
          <input aria-label={t("framing.phase")} value={phase} onChange={(e) => setPhase(e.target.value)} />
        </label>
        {isSetup && (
          <label className="op-field op-field--wide">
            <span>{t("framing.firstCommitment")}</span>
            <input
              aria-label={t("framing.firstCommitment")}
              value={firstCommitment}
              onChange={(e) => setFirstCommitment(e.target.value)}
            />
          </label>
        )}
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
