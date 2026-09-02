import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../../api";
import type { ProjectId, TaskList } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";
import { queryKeys } from "../../queryKeys";

export function CommitTimeline({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const timelineKey = ["timeline", projectId] as const;
  const taskKey = ["tasks", projectId] as const;
  const { data: rawData, isLoading } = useQuery({ queryKey: timelineKey, queryFn: () => api.getCommitTimeline(projectId) });
  const { data: taskList } = useQuery({ queryKey: taskKey, queryFn: () => api.getTasks(projectId) });
  const tasks = taskList?.tasks ?? [];
  const data = Array.isArray(rawData) ? rawData : [];

  async function assign(sha: string, previousTaskId: string | undefined, nextTaskId: string) {
    if (!taskList) return;
    let next: TaskList = taskList;
    if (previousTaskId && previousTaskId !== nextTaskId) {
      next = await api.unattributeCommit({ project_id: projectId, expected_revision: next.revision, id: previousTaskId, sha });
    }
    if (nextTaskId && previousTaskId !== nextTaskId) {
      next = await api.attributeCommit({ project_id: projectId, expected_revision: next.revision, id: nextTaskId, sha });
    }
    client.setQueryData(taskKey, next);
    await Promise.all([
      client.invalidateQueries({ queryKey: timelineKey }),
      client.invalidateQueries({ queryKey: queryKeys.projectOverview(projectId) }),
      client.invalidateQueries({ queryKey: queryKeys.projectIndex }),
    ]);
  }

  return <section className="op-section" aria-labelledby="timeline-heading" data-testid="commit-timeline"><div className="op-section__header"><div><p className="op-section__kicker">{t("timeline.kicker")}</p><h3 id="timeline-heading">{t("timeline.title")}</h3></div></div>{isLoading ? <p className="op-muted">{t("timeline.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("timeline.empty")}</p> : <ol className="op-timeline">{data.map((commit) => <li key={commit.sha}><code>{commit.short_sha}</code><span><strong>{commit.subject}</strong><small>{commit.committed_at} · {commit.author}</small>{commit.attributed_task_ids.length > 0 && <small>{t("timeline.attributed", { ids: commit.attributed_task_ids.join(", ") })}</small>}<select aria-label={t("timeline.assign", { sha: commit.short_sha })} value={commit.attributed_task_ids[0] ?? ""} onChange={(event) => void assign(commit.sha, commit.attributed_task_ids[0], event.target.value)}><option value="">{t("timeline.assignNone")}</option>{tasks.map((task) => <option key={task.id} value={task.id}>{task.text}</option>)}</select></span></li>)}</ol>}</section>;
}
