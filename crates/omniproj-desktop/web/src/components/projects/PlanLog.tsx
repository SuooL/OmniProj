import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api";
import type { PlanStatus, ProjectId } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function PlanLog({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const { data: rawData, isLoading } = useQuery({ queryKey: ["plan", projectId], queryFn: () => api.getPlan(projectId) });
  const data = Array.isArray(rawData) ? rawData : [];
  const refresh = () => client.invalidateQueries({ queryKey: ["plan", projectId] });
  async function add() { if (!title.trim()) return; await api.addPlanEntry({ project_id: projectId, title: title.trim(), body: body.trim() }); setTitle(""); setBody(""); refresh(); }
  return <section className="op-section" aria-labelledby="plan-heading" data-testid="plan-log">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("plan.kicker")}</p><h3 id="plan-heading">{t("plan.title")}</h3></div><span className="op-section__count">{data.length}</span></div>
    <div className="op-plan-add"><input aria-label={t("plan.newTitle")} placeholder={t("plan.newTitle")} value={title} onChange={(e) => setTitle(e.target.value)} /><textarea aria-label={t("plan.body")} placeholder={t("plan.body")} value={body} onChange={(e) => setBody(e.target.value)} /><button className="op-button op-button--primary" type="button" disabled={!title.trim()} onClick={() => void add()}>{t("plan.add")}</button></div>
    {isLoading ? <p className="op-muted">{t("plan.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("plan.empty")}</p> : <ol className="op-plan-list">{data.map((entry) => <li key={entry.id ?? `${entry.date}-${entry.title}`}><div><strong>{entry.title}</strong><small>{entry.date}{entry.commit ? ` · ${entry.commit}` : ""}</small>{entry.body && <p>{entry.body}</p>}</div>{entry.id && <select aria-label={`${t("plan.status")}: ${entry.title}`} value={entry.status} onChange={(e) => void api.setPlanStatus({ project_id: projectId, id: entry.id!, status: e.target.value }).then(refresh)}><option value="planned">{t("plan.planned")}</option><option value="doing">{t("plan.doing")}</option><option value="done">{t("plan.done")}</option><option value="abandoned">{t("plan.abandoned")}</option></select>}</li>)}</ol>}
  </section>;
}

export type { PlanStatus };
