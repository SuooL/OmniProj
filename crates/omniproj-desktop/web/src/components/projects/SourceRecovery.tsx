// Shown only when the project's source has moved / gone missing / become unreadable. It relinks
// the SAME project to a new location without changing its identity (CAS on expected source
// revision + expected old location). It reuses the native picker and the read-only validator,
// requires an explicit confirmation, and — if the chosen folder is already another project —
// offers "Open existing project" and NEVER steals the source.

import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { api, AppError } from "../../api";
import type { ProjectId, ProjectOverview, ProjectSource, SourceValidation } from "../../domain/project";
import { projectOverviewPath } from "../../domain/routes";
import { useOverviewMutation } from "../../hooks/useOverviewMutation";
import { chooseProjectDirectory } from "../../platform/dialog";
import { localizeError, useI18n } from "../../i18n/I18nProvider";

const RECOVERABLE: ReadonlySet<ProjectSource["status"]> = new Set([
  "missing",
  "moved",
  "unreadable",
]);

function sourceStatusLabel(status: ProjectSource["status"], locale: "zh-CN" | "en"): string {
  if (locale === "en") return status;
  switch (status) {
    case "missing": return "缺失";
    case "moved": return "已移动";
    case "unreadable": return "不可读";
    case "available": return "可用";
  }
}

function validationStateLabel(state: SourceValidation["state"], locale: "zh-CN" | "en"): string {
  if (locale === "en") return state.replace(/_/g, " ");
  const labels: Record<SourceValidation["state"], string> = {
    ok: "可用", duplicate: "已注册", missing: "目录缺失", unreadable: "无法读取",
    not_git_repository: "不是 Git 仓库", bare_repository: "裸仓库", observation_failed: "观测失败",
  };
  return labels[state];
}

export interface SourceRecoveryProps {
  overview: ProjectOverview;
}

export function SourceRecovery({ overview }: SourceRecoveryProps) {
  const { locale, t } = useI18n();
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
  const sourceStatus = sourceStatusLabel(source.status, locale);

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
      setValidateError(e instanceof AppError ? localizeError(e, locale) : t("add.validateFailed"));
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
      t("recovery.success"),
    );
    if (result.status !== "success") setFailed(true);
  }

  function openExisting(existingId: ProjectId) {
    navigate(projectOverviewPath(existingId));
  }

  return (
    <section className="op-section op-section--danger" aria-labelledby="source-recovery-heading" data-testid="source-recovery">
      <div className="op-section__header">
        <div>
          <p className="op-section__kicker">{t("recovery.kicker")}</p>
          <h3 id="source-recovery-heading">{t("recovery.title")}</h3>
        </div>
      </div>
      <p>
        {t("recovery.description", { status: sourceStatus })}
      </p>

      <button className="op-button op-button--secondary" type="button" onClick={choose} disabled={mutation.pending || validating}>
        {t("recovery.choose")}
      </button>
      {newPath && <p className="op-path-box" data-testid="relink-path">{newPath}</p>}

      {validation?.state === "duplicate" && (
        <div className="op-validation-card op-validation-card--warning" data-testid="relink-duplicate" role="alert">
          <p>{t("recovery.duplicate", { name: validation.existing_name })}</p>
          <button className="op-button op-button--secondary" type="button" onClick={() => openExisting(validation.existing_project_id)}>
            {t("add.openExisting")}
          </button>
        </div>
      )}

      {validation && validation.state !== "ok" && validation.state !== "duplicate" && (
        <p className="op-mutation-error" role="alert" data-testid="relink-invalid">
          {t("recovery.invalid", { state: validationStateLabel(validation.state, locale) })}
        </p>
      )}

      {validation?.state === "ok" && (
        <div className="op-relink-confirm" data-testid="relink-confirm">
          <label className="op-check-field">
            <input
              type="checkbox"
              aria-label={t("recovery.confirm")}
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
            />
            {t("recovery.confirmNotice")}
          </label>
          <button
            className="op-button op-button--primary"
            type="button"
            disabled={mutation.pending || !confirmed}
            onClick={relink}
          >
            {t("recovery.relink")}
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
          {localizeError(mutation.error, locale)}
        </p>
      )}
    </section>
  );
}
