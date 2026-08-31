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
import { localizeError, useI18n, type Translate } from "../i18n/I18nProvider";

export interface AddProjectDialogProps {
  onClose: () => void;
}

/** A human-readable, recoverable message per non-registerable validation state. */
function invalidMessage(v: Exclude<SourceValidation, { state: "ok" | "duplicate" }>, t: Translate): string {
  switch (v.state) {
    case "missing":
      return t("add.missing");
    case "unreadable":
      return t("add.unreadable");
    case "not_git_repository":
      return t("add.notGit");
    case "bare_repository":
      return t("add.bare");
    case "observation_failed":
      return t("add.observationFailed", { message: v.message });
  }
}

export function AddProjectDialog({ onClose }: AddProjectDialogProps) {
  const { locale, t } = useI18n();
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
      setError(e instanceof AppError ? localizeError(e, locale) : t("add.validateFailed"));
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
      announce("polite", t("add.registered"));
      navigate(projectOverviewPath(overview.project_id));
    } catch (e) {
      setError(e instanceof AppError ? localizeError(e, locale) : t("add.registerFailed"));
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
      aria-label={t("add.title")}
      data-testid="add-project-dialog"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div className="op-dialog__header">
        <div className="op-dialog__icon" aria-hidden="true">+</div>
        <div>
          <p className="op-section__kicker">{t("add.kicker")}</p>
          <h2>{t("add.title")}</h2>
          <p>{t("add.description")}</p>
        </div>
        <button className="op-window-button" type="button" onClick={onClose} aria-label={t("add.close")}>
          <span aria-hidden="true">×</span>
        </button>
      </div>

      <div className="op-dialog__body">
        <button
          aria-label={path ? t("add.chooseAgain") : t("add.choose")}
          className="op-picker"
          type="button"
          onClick={choose}
          disabled={busy}
        >
          <span className="op-picker__mark" aria-hidden="true">↗</span>
          <span>
            <strong>{path ? t("add.chooseDifferent") : t("add.chooseProject")}</strong>
            <small>{t("add.readonly")}</small>
          </span>
        </button>
        {path && <p className="op-path-box" data-testid="chosen-path">{path}</p>}

        {validation?.state === "ok" && (
          <div className="op-validation-card op-validation-card--success" data-testid="valid-preview">
            <p className="op-validation-card__title">{t("add.ready")}</p>
            <p className="op-validation-card__facts">
              <span>{validation.head.kind === "attached" || validation.head.kind === "unborn" ? validation.head.branch : t("head.detached")}</span>
              <span>{validation.last_commit ? `${validation.last_commit.short_sha} ${validation.last_commit.subject}` : t("add.noCommits")}</span>
            </p>
            <label className="op-field">
              <span>{t("add.projectName")}</span>
              <input aria-label={t("add.projectName")} value={name} onChange={(e) => setName(e.target.value)} />
            </label>
          </div>
        )}

        {validation?.state === "duplicate" && (
          <div className="op-validation-card op-validation-card--warning" data-testid="duplicate-notice" role="alert">
            <p>{t("add.duplicate", { name: validation.existing_name })}</p>
            <button className="op-button op-button--secondary" type="button" onClick={() => openExisting(validation.existing_project_id)}>
              {t("add.openExisting")}
            </button>
          </div>
        )}

        {validation && validation.state !== "ok" && validation.state !== "duplicate" && (
          <div className="op-validation-card op-validation-card--danger" data-testid="invalid-notice" role="alert">
            <p>{invalidMessage(validation, t)}</p>
            {validation.state === "observation_failed" && (
              <button className="op-button op-button--secondary" type="button" onClick={() => path && validatePath(path)} disabled={busy}>
                {t("common.tryAgain")}
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
          {t("add.register")}
        </button>
        <button className="op-button op-button--ghost" type="button" onClick={onClose}>
          {t("common.cancel")}
        </button>
      </div>
    </dialog>
  );
}
