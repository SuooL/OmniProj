// Add Project: a native modal that walks select -> validate -> preview -> explicit Register.
// Validation is read-only (it never mutates the store) and Register stays disabled until a valid
// preview exists. A duplicate offers "Open existing project" and never registers a second copy.
// On success the dialog closes, navigates to the new project's canonical Overview, and the setup
// framing focuses the objective.
//
// It is mounted only while open (AppShell gates it), so opening = mount and closing = unmount;
// the mount effect enters modal state and restores focus to the trigger on unmount.

import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { api, AppError } from "../api";
import type { ProjectId, SourceValidation } from "../domain/project";
import { projectOverviewPath } from "../domain/routes";
import { basename, chooseProjectDirectory } from "../platform/dialog";
import { applyOverviewToCaches } from "../queryClient";
import { useAnnouncer } from "./AppShell";

export interface AddProjectDialogProps {
  onClose: () => void;
}

/** A human-readable, recoverable message per non-registerable validation state. */
function invalidMessage(v: Exclude<SourceValidation, { state: "ok" | "duplicate" }>): string {
  switch (v.state) {
    case "missing":
      return "That folder no longer exists.";
    case "unreadable":
      return "That folder can't be read. Check permissions and try again.";
    case "not_git_repository":
      return "That folder isn't a Git repository.";
    case "bare_repository":
      return "Bare Git repositories aren't supported.";
    case "observation_failed":
      return `Couldn't read the repository: ${v.message}`;
  }
}

export function AddProjectDialog({ onClose }: AddProjectDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const announce = useAnnouncer();

  const [path, setPath] = useState<string | null>(null);
  const [validation, setValidation] = useState<SourceValidation | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Enter modal state on mount; restore the trigger's focus on unmount.
  useEffect(() => {
    const d = dialogRef.current;
    triggerRef.current = document.activeElement as HTMLElement | null;
    try {
      if (typeof d?.showModal === "function") d.showModal();
      else d?.setAttribute("open", "");
    } catch {
      d?.setAttribute("open", "");
    }
    return () => {
      triggerRef.current?.focus?.();
    };
  }, []);

  async function validatePath(target: string) {
    setBusy(true);
    setError(null);
    try {
      const result = await api.validateProjectSource(target);
      setValidation(result);
      if (result.state === "ok") setName(basename(target));
    } catch (e) {
      setError(e instanceof AppError ? e.message : "Couldn't validate that folder.");
    } finally {
      setBusy(false);
    }
  }

  async function choose() {
    const chosen = await chooseProjectDirectory();
    if (chosen === null) return; // cancelled — no change
    setPath(chosen);
    setValidation(null);
    await validatePath(chosen);
  }

  async function register() {
    if (busy || path === null || validation?.state !== "ok") return;
    setBusy(true);
    setError(null);
    try {
      const overview = await api.registerProject({ location: path, name: name.trim() });
      applyOverviewToCaches(queryClient, overview);
      onClose();
      announce("polite", "Project registered.");
      navigate(projectOverviewPath(overview.project_id));
    } catch (e) {
      setError(e instanceof AppError ? e.message : "Couldn't register that project.");
    } finally {
      setBusy(false);
    }
  }

  function openExisting(existingId: ProjectId) {
    onClose();
    navigate(projectOverviewPath(existingId));
  }

  return (
    <dialog
      className="op-dialog"
      ref={dialogRef}
      aria-label="Add Project"
      data-testid="add-project-dialog"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div className="op-dialog__header">
        <div className="op-dialog__icon" aria-hidden="true">+</div>
        <div>
          <p className="op-section__kicker">Local Git repository</p>
          <h2>Add Project</h2>
          <p>Register a repository without changing anything inside it.</p>
        </div>
        <button className="op-window-button" type="button" onClick={onClose} aria-label="Close Add Project">
          <span aria-hidden="true">×</span>
        </button>
      </div>

      <div className="op-dialog__body">
        <button
          aria-label={path ? "Choose directory again" : "Choose directory"}
          className="op-picker"
          type="button"
          onClick={choose}
          disabled={busy}
        >
          <span className="op-picker__mark" aria-hidden="true">↗</span>
          <span>
            <strong>{path ? "Choose a different directory" : "Choose project directory"}</strong>
            <small>OmniProj reads Git facts and keeps the repository itself read-only.</small>
          </span>
        </button>
        {path && <p className="op-path-box" data-testid="chosen-path">{path}</p>}

        {validation?.state === "ok" && (
          <div className="op-validation-card op-validation-card--success" data-testid="valid-preview">
            <p className="op-validation-card__title">Repository ready</p>
            <p className="op-validation-card__facts">
              <span>{validation.head.kind === "attached" || validation.head.kind === "unborn" ? validation.head.branch : "Detached HEAD"}</span>
              <span>{validation.last_commit ? `${validation.last_commit.short_sha} ${validation.last_commit.subject}` : "No commits yet"}</span>
            </p>
            <label className="op-field">
              <span>Project name</span>
              <input aria-label="Project name" value={name} onChange={(e) => setName(e.target.value)} />
            </label>
          </div>
        )}

        {validation?.state === "duplicate" && (
          <div className="op-validation-card op-validation-card--warning" data-testid="duplicate-notice" role="alert">
            <p>Already registered as “{validation.existing_name}”.</p>
            <button className="op-button op-button--secondary" type="button" onClick={() => openExisting(validation.existing_project_id)}>
              Open existing project
            </button>
          </div>
        )}

        {validation && validation.state !== "ok" && validation.state !== "duplicate" && (
          <div className="op-validation-card op-validation-card--danger" data-testid="invalid-notice" role="alert">
            <p>{invalidMessage(validation)}</p>
            {validation.state === "observation_failed" && (
              <button className="op-button op-button--secondary" type="button" onClick={() => path && validatePath(path)} disabled={busy}>
                Try again
              </button>
            )}
          </div>
        )}

        {error && (
          <p className="op-mutation-error" role="alert" data-testid="add-project-error">
            {error}
          </p>
        )}

      </div>
      <div className="op-dialog-actions">
        <button
          className="op-button op-button--primary"
          type="button"
          onClick={register}
          disabled={busy || validation?.state !== "ok" || name.trim() === ""}
        >
          Register
        </button>
        <button className="op-button op-button--ghost" type="button" onClick={onClose}>
          Cancel
        </button>
      </div>
    </dialog>
  );
}
