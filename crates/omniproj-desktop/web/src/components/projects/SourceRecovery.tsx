// Shown only when the project's source has moved / gone missing / become unreadable. It lets
// the Human point the SAME project at a new location without changing its identity (relink uses
// the expected source revision + expected old location for a safe CAS). The full native picker
// and duplicate handling arrive in Task 12; here it is a minimal, explicit relink.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview, ProjectSource } from "../../domain/project";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";

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
  const source = overview.source;
  const [newLocation, setNewLocation] = useState("");
  const [failed, setFailed] = useState(false);

  if (!source || !RECOVERABLE.has(source.status)) return null;

  async function relink() {
    if (!source) return;
    setFailed(false);
    const result = await mutation.run(
      overview.project_id,
      () =>
        api.relinkProjectSource({
          project_id: overview.project_id,
          expected_source_revision: source.revision,
          expected_location: source.location,
          new_location: newLocation.trim(),
        }),
      "Source relinked.",
    );
    if (result.status !== "success") setFailed(true);
  }

  return (
    <section aria-labelledby="source-recovery-heading" data-testid="source-recovery">
      <h3 id="source-recovery-heading">Source unavailable</h3>
      <p>
        This project's source is <strong>{source.status}</strong>. Point it at the repository's
        new location to restore observations — the project keeps its identity and history.
      </p>
      <label>
        New location
        <input
          aria-label="New source location"
          value={newLocation}
          onChange={(e) => setNewLocation(e.target.value)}
        />
      </label>
      <button
        type="button"
        disabled={mutation.pending || newLocation.trim() === ""}
        onClick={relink}
      >
        Relink source
      </button>
      {failed && mutation.error && (
        <p role="alert" className="op-mutation-error" data-testid="source-recovery-error">
          {mutation.error.message}
        </p>
      )}
    </section>
  );
}
