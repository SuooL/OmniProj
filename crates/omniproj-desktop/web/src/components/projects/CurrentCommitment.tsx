// The Current commitment surface: the single explicit action the Human is committing to, plus
// its lifecycle actions. Rules: drafts are local and cleared only on success; only the SUBMITTED
// action is disabled (not the whole surface); a failed write keeps the readable draft and offers
// Retry + Copy; a revision conflict keeps the draft after refetching; a durable audit failure
// clears the draft (the change is saved) and never resends; Undo is offered only for the newest
// returned undoable transition.

import { useState } from "react";

import { api } from "../../api";
import type { ProjectOverview } from "../../domain/project";
import { useOverviewMutation, type MutationOutcome } from "../../hooks/useOverviewMutation";
import { CommitmentStateTag } from "../semantic/CommitmentStateTag";

type ActionId = "set" | "confirm" | "complete" | "replace" | "clear" | "undo";

function copyText(text: string) {
  void navigator?.clipboard?.writeText?.(text);
}

export interface CurrentCommitmentProps {
  overview: ProjectOverview;
}

export function CurrentCommitment({ overview }: CurrentCommitmentProps) {
  const mutation = useOverviewMutation();
  const commitment = overview.current_commitment;
  const [setText, setSetText] = useState("");
  const [replacing, setReplacing] = useState(false);
  const [replaceText, setReplaceText] = useState("");
  const [replaceReason, setReplaceReason] = useState("");
  const [busy, setBusy] = useState<ActionId | null>(null);
  const [outcome, setOutcome] = useState<MutationOutcome | null>(null);
  const [lastAction, setLastAction] = useState<(() => void) | null>(null);

  const pid = overview.project_id;
  const rev = overview.revision;

  async function run(
    id: ActionId,
    action: () => Promise<MutationOutcome>,
    onSuccess?: () => void,
  ) {
    setLastAction(() => () => void run(id, action, onSuccess));
    setBusy(id);
    const result = await action();
    setBusy(null);
    setOutcome(result);
    // Clear the draft when the change is durable (success OR durable-but-unaudited).
    if (result.status === "success" || result.status === "durable_audit_failed") onSuccess?.();
  }

  const doSet = () =>
    run(
      "set",
      () => mutation.run(pid, () => api.setCommitment({ project_id: pid, expected_revision: rev, text: setText.trim() }), "Commitment set."),
      () => setSetText(""),
    );

  const doConfirm = () =>
    run("confirm", () =>
      mutation.run(pid, () => api.confirmCommitment({ project_id: pid, expected_revision: rev, work_item_id: commitment!.work_item_id }), "Commitment confirmed."),
    );

  const doComplete = () =>
    run("complete", () =>
      mutation.run(pid, () => api.completeCommitment({ project_id: pid, expected_revision: rev, work_item_id: commitment!.work_item_id }), "Commitment completed."),
    );

  const doReplace = () =>
    run(
      "replace",
      () =>
        mutation.run(
          pid,
          () =>
            api.replaceCommitment({
              project_id: pid,
              expected_revision: rev,
              previous_work_item_id: commitment!.work_item_id,
              text: replaceText.trim(),
              reason: replaceReason.trim(),
            }),
          "Commitment replaced.",
        ),
      () => {
        setReplacing(false);
        setReplaceText("");
        setReplaceReason("");
      },
    );

  const doClear = () =>
    run("clear", () =>
      mutation.run(pid, () => api.clearCommitment({ project_id: pid, expected_revision: rev, work_item_id: commitment!.work_item_id }), "Commitment cleared."),
    );

  const doUndo = () =>
    run("undo", () =>
      mutation.run(
        pid,
        () => api.undoCommitmentTransition({ project_id: pid, expected_revision: rev, transition_id: overview.undoable_transition_id! }),
        "Last change undone.",
      ),
    );

  const draft = replacing ? replaceText : setText;

  return (
    <section className="op-section op-section--commitment" aria-labelledby="commitment-heading" data-testid="current-commitment">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">Human commitment</p>
          <h3 id="commitment-heading">Current commitment</h3>
        </div>
        {commitment && <CommitmentStateTag status={commitment.status} />}
      </div>

      {commitment ? (
        <div>
          <p className="op-commitment-text">{commitment.text}</p>
          <div className="op-commitment-actions">
            {commitment.confirmed_at === null && (
              <button className="op-button op-button--secondary" type="button" disabled={busy === "confirm"} onClick={doConfirm}>
                Confirm
              </button>
            )}
            <button className="op-button op-button--primary" type="button" disabled={busy === "complete"} onClick={doComplete}>
              Complete
            </button>
            <button className="op-button op-button--secondary" type="button" disabled={busy === "replace"} onClick={() => setReplacing(true)}>
              Replace
            </button>
            <button className="op-button op-button--ghost" type="button" disabled={busy === "clear"} onClick={doClear}>
              Clear
            </button>
          </div>

          {replacing && (
            <div className="op-replace-form" data-testid="replace-form">
              <label className="op-field">
                <span>New commitment</span>
                <input aria-label="New commitment" value={replaceText} onChange={(e) => setReplaceText(e.target.value)} />
              </label>
              <label className="op-field">
                <span>Reason <small>Required</small></span>
                <input aria-label="Replace reason" value={replaceReason} onChange={(e) => setReplaceReason(e.target.value)} />
              </label>
              <button
                className="op-button op-button--primary"
                type="button"
                disabled={busy === "replace" || replaceText.trim() === "" || replaceReason.trim() === ""}
                onClick={doReplace}
              >
                Save replacement
              </button>
              <button className="op-button op-button--ghost" type="button" onClick={() => setReplacing(false)}>
                Cancel
              </button>
            </div>
          )}
        </div>
      ) : (
        <div className="op-set-form" data-testid="set-form">
          <label className="op-field">
            <span>New commitment</span>
            <input aria-label="New commitment" value={setText} onChange={(e) => setSetText(e.target.value)} />
          </label>
          <button className="op-button op-button--primary" type="button" disabled={busy === "set" || setText.trim() === ""} onClick={doSet}>
            Save commitment
          </button>
        </div>
      )}

      {overview.undoable_transition_id && (
        <button className="op-button op-button--ghost op-undo-button" type="button" disabled={busy === "undo"} onClick={doUndo} data-testid="undo-button">
          Undo last change
        </button>
      )}

      {outcome && outcome.status === "durable_audit_failed" && (
        <p role="status" className="op-mutation-note" data-testid="audit-failed-note">
          State saved; the audit commit failed. Your change is durable — no need to resend.
        </p>
      )}
      {outcome && outcome.status === "conflict" && (
        <p role="alert" className="op-mutation-error" data-testid="conflict-note">
          This project changed since you started. The latest state is loaded; review it and
          resubmit — your text is kept.
        </p>
      )}
      {outcome && outcome.status === "error" && (
        <div role="alert" className="op-mutation-error" data-testid="write-error">
          <p>{outcome.error.message}</p>
          {outcome.error.recovery === "retry" && (
            <div className="op-mutation-error__actions">
              <button className="op-button op-button--secondary" type="button" onClick={() => lastAction?.()}>
                Retry
              </button>
              <button className="op-button op-button--ghost" type="button" onClick={() => copyText(draft)}>
                Copy text
              </button>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
