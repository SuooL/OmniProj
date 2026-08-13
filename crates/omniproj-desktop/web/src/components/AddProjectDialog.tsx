// Add Project: a native modal that walks select -> validate -> preview -> explicit Register.
// Validation is read-only (it never mutates the store) and Register stays disabled until a valid
// preview exists. A duplicate offers "Open existing project" and never registers a second copy.
// On success the dialog closes, navigates to the new project's canonical Overview as a Peek over
// the Index, and the setup framing focuses the objective.
//
// It is mounted only while open (AppShell gates it), so opening = mount and closing = unmount;
// the mount effect enters modal state and restores focus to the trigger on unmount.

import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { api, AppError } from "../api";
import type { ProjectId, SourceValidation } from "../domain/project";
import { projectOverviewPath } from "../domain/routes";
import { basename, chooseProjectDirectory } from "../platform/dialog";
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
  const location = useLocation();
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

  const backgroundState = { backgroundLocation: location };

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
    if (path === null || validation?.state !== "ok") return;
    setBusy(true);
    setError(null);
    try {
      const overview = await api.registerProject({ location: path, name: name.trim() });
      onClose();
      announce("polite", "Project registered.");
      navigate(projectOverviewPath(overview.project_id), { state: backgroundState });
    } catch (e) {
      setError(e instanceof AppError ? e.message : "Couldn't register that project.");
    } finally {
      setBusy(false);
    }
  }

  function openExisting(existingId: ProjectId) {
    onClose();
    navigate(projectOverviewPath(existingId), { state: backgroundState });
  }

  return (
    <dialog
      ref={dialogRef}
      aria-label="Add Project"
      data-testid="add-project-dialog"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <h2>Add Project</h2>

      <button type="button" onClick={choose} disabled={busy}>
        Choose directory…
      </button>
      {path && <p data-testid="chosen-path">{path}</p>}

      {validation?.state === "ok" && (
        <div data-testid="valid-preview">
          <label>
            Project name
            <input aria-label="Project name" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
        </div>
      )}

      {validation?.state === "duplicate" && (
        <div data-testid="duplicate-notice" role="alert">
          <p>Already registered as “{validation.existing_name}”.</p>
          <button type="button" onClick={() => openExisting(validation.existing_project_id)}>
            Open existing project
          </button>
        </div>
      )}

      {validation && validation.state !== "ok" && validation.state !== "duplicate" && (
        <div data-testid="invalid-notice" role="alert">
          <p>{invalidMessage(validation)}</p>
          {validation.state === "observation_failed" && (
            <button type="button" onClick={() => path && validatePath(path)} disabled={busy}>
              Try again
            </button>
          )}
        </div>
      )}

      {error && (
        <p role="alert" data-testid="add-project-error">
          {error}
        </p>
      )}

      <div className="op-dialog-actions">
        <button
          type="button"
          onClick={register}
          disabled={busy || validation?.state !== "ok" || name.trim() === ""}
        >
          Register
        </button>
        <button type="button" onClick={onClose}>
          Cancel
        </button>
      </div>
    </dialog>
  );
}
