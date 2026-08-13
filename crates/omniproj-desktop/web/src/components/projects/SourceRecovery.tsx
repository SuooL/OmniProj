// Shown only when the project's source has moved / gone missing / become unreadable. It relinks
// the SAME project to a new location without changing its identity (CAS on expected source
// revision + expected old location). It reuses the native picker and the read-only validator,
// requires an explicit confirmation, and — if the chosen folder is already another project —
// offers "Open existing project" and NEVER steals the source.

import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { api, AppError } from "../../api";
import type { ProjectId, ProjectOverview, ProjectSource, SourceValidation } from "../../domain/project";
import { projectOverviewPath, projectsPath } from "../../domain/routes";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";
import { chooseProjectDirectory } from "../../platform/dialog";

const RECOVERABLE: ReadonlySet<ProjectSource["status"]> = new Set([
  "missing",
  "moved",
  "unreadable",
]);

export interface SourceRecoveryProps {
  overview: ProjectOverview;
}

export function SourceRecovery({ overview }: SourceRecoveryProps) {
  const mutation = useOverviewMutation();
  const navigate = useNavigate();
  const source = overview.source;
  const [newPath, setNewPath] = useState<string | null>(null);
  const [validation, setValidation] = useState<SourceValidation | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validateError, setValidateError] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  if (!source || !RECOVERABLE.has(source.status)) return null;

  async function choose() {
    if (validating || mutation.pending) return;
    const chosen = await chooseProjectDirectory();
    if (chosen === null) return;
    setNewPath(chosen);
    setValidation(null);
    setConfirmed(false);
    setFailed(false);
    setValidateError(null);
    setValidating(true);
    try {
      setValidation(await api.validateProjectSource(chosen));
    } catch (e) {
      setValidateError(e instanceof AppError ? e.message : "Couldn't validate that folder.");
    } finally {
      setValidating(false);
    }
  }

  async function relink() {
    if (mutation.pending || !source || newPath === null || validation?.state !== "ok") return;
    setFailed(false);
    const result = await mutation.run(
      overview.project_id,
      () =>
        api.relinkProjectSource({
          project_id: overview.project_id,
          expected_source_revision: source.revision,
          expected_location: source.location,
          new_location: newPath,
        }),
      "Source relinked.",
    );
    if (result.status !== "success") setFailed(true);
  }

  function openExisting(existingId: ProjectId) {
    // Open the existing project as a Peek over the Index (consistent with Add Project).
    navigate(projectOverviewPath(existingId), {
      state: { backgroundLocation: { pathname: projectsPath() } },
    });
  }

  return (
    <section className="op-section op-section--danger" aria-labelledby="source-recovery-heading" data-testid="source-recovery">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">Recovery required</p>
          <h3 id="source-recovery-heading">Source unavailable</h3>
        </div>
      </div>
      <p>
        This project's source is <strong>{source.status}</strong>. Point it at the repository's new
        location to restore observations — the project keeps its identity and history.
      </p>

      <button className="op-button op-button--secondary" type="button" onClick={choose} disabled={mutation.pending || validating}>
        Choose new location…
      </button>
      {newPath && <p className="op-path-box" data-testid="relink-path">{newPath}</p>}

      {validation?.state === "duplicate" && (
        <div className="op-validation-card op-validation-card--warning" data-testid="relink-duplicate" role="alert">
          <p>That folder is already registered as “{validation.existing_name}”.</p>
          <button className="op-button op-button--secondary" type="button" onClick={() => openExisting(validation.existing_project_id)}>
            Open existing project
          </button>
        </div>
      )}

      {validation && validation.state !== "ok" && validation.state !== "duplicate" && (
        <p className="op-mutation-error" role="alert" data-testid="relink-invalid">
          That folder can't be used ({validation.state.replace(/_/g, " ")}).
        </p>
      )}

      {validation?.state === "ok" && (
        <div className="op-relink-confirm" data-testid="relink-confirm">
          <label className="op-check-field">
            <input
              type="checkbox"
              aria-label="Confirm relink"
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
            />
            I confirm this is the same project's repository.
          </label>
          <button
            className="op-button op-button--primary"
            type="button"
            disabled={mutation.pending || !confirmed}
            onClick={relink}
          >
            Relink source
          </button>
        </div>
      )}

      {validateError && (
        <p role="alert" data-testid="relink-validate-error">
          {validateError}
        </p>
      )}
      {failed && mutation.error && (
        <p role="alert" data-testid="source-recovery-error">
          {mutation.error.message}
        </p>
      )}
    </section>
  );
}
