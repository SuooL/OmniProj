import { useQuery } from "@tanstack/react-query";
import { api } from "../../api";
import type { ProjectId } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function CommitTimeline({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const { data: rawData, isLoading } = useQuery({ queryKey: ["timeline", projectId], queryFn: () => api.getCommitTimeline(projectId) });
  const { data: rawTasks } = useQuery({ queryKey: ["tasks", projectId], queryFn: () => api.getTasks(projectId) });
  const tasks = Array.isArray(rawTasks) ? rawTasks : [];
  const data = Array.isArray(rawData) ? rawData : [];
  return <section className="op-section" aria-labelledby="timeline-heading" data-testid="commit-timeline"><div className="op-section__header"><div><p className="op-section__kicker">{t("timeline.kicker")}</p><h3 id="timeline-heading">{t("timeline.title")}</h3></div></div>{isLoading ? <p className="op-muted">{t("timeline.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("timeline.empty")}</p> : <ol className="op-timeline">{data.map((c) => <li key={c.sha}><code>{c.short_sha}</code><span><strong>{c.subject}</strong><small>{c.committed_at} · {c.author}</small>{c.attributed_task_ids.length > 0 && <small>{t("timeline.attributed", { ids: c.attributed_task_ids.join(", ") })}</small>}<select aria-label={t("timeline.assign", { sha: c.short_sha })} value={c.attributed_task_ids[0] ?? ""} onChange={(e) => { if (e.target.value) void api.attributeCommit({ project_id: projectId, id: e.target.value, sha: c.sha }); }}><option value="">{t("timeline.assignNone")}</option>{tasks.map((task) => <option key={task.id} value={task.id}>{task.text}</option>)}</select></span></li>)}</ol>}</section>;
}
