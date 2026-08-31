// Framing = the Human's intent: objective, desired outcome, optional phase, and (in setup) the
// first commitment. In `setup` this is ONE atomic `complete_project_setup` that promotes the
// project to active in a single revision — there is no intermediate framing write. For an
// already-active project it is `save_project_framing`. Either way the save is explicit (a
// button, never blur) and the draft survives a failed write.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview } from "../../domain/project";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";

export interface ProjectFramingFormProps {
  overview: ProjectOverview;
}

export function ProjectFramingForm({ overview }: ProjectFramingFormProps) {
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
          "Setup complete.",
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
          "Framing saved.",
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
          <p className="op-section__kicker">Human-authored intent</p>
          <h3 id="framing-heading">{isSetup ? "Complete setup" : "Project framing"}</h3>
        </div>
      </div>
      {isSetup && (
        <p className="op-section__intro">
          Define the outcome and first concrete commitment before moving this project into
          active work.
        </p>
      )}
      <div className="op-form-grid">
        <label className="op-field op-field--wide">
          <span>Objective</span>
          {/* Setup focuses the objective first (spec 9.4 order). */}
          <input
            aria-label="Objective"
            autoFocus={isSetup}
            value={objective}
            onChange={(e) => setObjective(e.target.value)}
          />
        </label>
        <label className="op-field op-field--wide">
          <span>Desired outcome</span>
          <input
            aria-label="Desired outcome"
            value={desiredOutcome}
            onChange={(e) => setDesiredOutcome(e.target.value)}
          />
        </label>
        <label className="op-field">
          <span>
            Phase <small>Optional</small>
          </span>
          <input aria-label="Phase" value={phase} onChange={(e) => setPhase(e.target.value)} />
        </label>
        {isSetup && (
          <label className="op-field op-field--wide">
            <span>First commitment</span>
            <input
              aria-label="First commitment"
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
          {isSetup ? "Complete setup" : "Save framing"}
        </button>
      </div>
      {failed && mutation.error && (
        <p role="alert" className="op-mutation-error" data-testid="framing-error">
          {mutation.error.recovery === "refetch"
            ? "This project changed since you started. The latest state is loaded; review and resubmit — your text is kept."
            : mutation.error.message}
        </p>
      )}
    </section>
  );
}
