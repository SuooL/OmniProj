import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../../api";
import type { PlanList, PlanStatus, ProjectId } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function PlanLog({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [commitDrafts, setCommitDrafts] = useState<Record<string, string>>({});
  const key = ["plan", projectId] as const;
  const { data, isLoading } = useQuery({ queryKey: key, queryFn: () => api.getPlan(projectId) });
  const entries = data?.entries ?? [];
  const accept = (next: PlanList) => client.setQueryData(key, next);

  async function add() {
    if (!title.trim() || !data) return;
    accept(await api.addPlanEntry({ project_id: projectId, expected_revision: data.revision, title: title.trim(), body: body.trim() }));
    setTitle("");
    setBody("");
  }

  async function setStatus(id: string, status: string) {
    if (!data) return;
    accept(await api.setPlanStatus({ project_id: projectId, expected_revision: data.revision, id, status }));
  }

  async function setCommit(id: string, commit: string) {
    if (!data) return;
    accept(await api.setPlanCommit({ project_id: projectId, expected_revision: data.revision, id, commit: commit.trim() || null }));
  }

  return <section className="op-section" aria-labelledby="plan-heading" data-testid="plan-log">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("plan.kicker")}</p><h3 id="plan-heading">{t("plan.title")}</h3></div><span className="op-section__count">{entries.length}</span></div>
    <div className="op-plan-add"><input aria-label={t("plan.newTitle")} placeholder={t("plan.newTitle")} value={title} onChange={(event) => setTitle(event.target.value)} /><textarea aria-label={t("plan.body")} placeholder={t("plan.body")} value={body} onChange={(event) => setBody(event.target.value)} /><button className="op-button op-button--primary" type="button" disabled={!title.trim() || !data} title={!title.trim() ? t("plan.addDisabled") : undefined} onClick={() => void add()}>{t("plan.add")}</button></div>
    {isLoading ? <p className="op-muted">{t("plan.loading")}</p> : entries.length === 0 ? <p className="op-muted">{t("plan.empty")}</p> : <ol className="op-plan-list">{entries.map((entry) => <li key={entry.id ?? `${entry.date}-${entry.title}`}><div><strong>{entry.title}</strong><small>{entry.date}{entry.commit ? ` · ${entry.commit}` : ""}</small>{entry.body && <p>{entry.body}</p>}</div>{entry.id && <div className="op-task-actions"><select aria-label={`${t("plan.status")}: ${entry.title}`} value={entry.status} onChange={(event) => void setStatus(entry.id!, event.target.value)}><option value="planned">{t("plan.planned")}</option><option value="doing">{t("plan.doing")}</option><option value="done">{t("plan.done")}</option><option value="abandoned">{t("plan.abandoned")}</option></select><input aria-label={`${t("plan.commit")}: ${entry.title}`} placeholder={t("plan.commit")} value={commitDrafts[entry.id] ?? entry.commit ?? ""} onChange={(event) => setCommitDrafts((all) => ({ ...all, [entry.id!]: event.target.value }))} /><button className="op-button op-button--secondary" type="button" onClick={() => void setCommit(entry.id!, commitDrafts[entry.id!] ?? entry.commit ?? "")}>{t("plan.saveCommit")}</button></div>}</li>)}</ol>}
  </section>;
}

export type { PlanStatus };
