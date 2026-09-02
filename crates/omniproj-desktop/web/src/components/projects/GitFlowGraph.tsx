import { useQuery } from "@tanstack/react-query";
import { api } from "../../api";
import type { ProjectId } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

const ROW = 34;
export function GitFlowGraph({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const { data: rawData, isLoading } = useQuery({ queryKey: ["git-graph", projectId], queryFn: () => api.getGitGraph(projectId) });
  const data = Array.isArray(rawData) ? rawData : [];
  return <section className="op-section" aria-labelledby="git-graph-heading" data-testid="git-flow-graph"><div className="op-section__header"><div><p className="op-section__kicker">{t("graph.kicker")}</p><h3 id="git-graph-heading">{t("graph.title")}</h3></div></div>{isLoading ? <p className="op-muted">{t("graph.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("graph.empty")}</p> : <div className="op-graph"><svg width="42" height={data.length * ROW} aria-hidden="true">{data.map((commit, i) => <g key={commit.sha}><line x1="20" y1={i * ROW + 17} x2="20" y2={(i + 1) * ROW + 17} stroke="var(--op-border-strong)" />{commit.parents.length > 1 && <line x1="20" y1={i * ROW + 17} x2="34" y2={i * ROW + 29} stroke="var(--op-status-warning-fg)" />}<circle cx="20" cy={i * ROW + 17} r={commit.parents.length > 1 ? 5 : 4} fill="var(--op-interactive-fg)" /></g>)}</svg><ol className="op-graph__list">{data.map((commit) => <li key={commit.sha}><code>{commit.short_sha}</code>{commit.refs.map((ref) => <span className="op-graph__ref" key={ref}>{ref}</span>)}<span className="op-graph__subject" title={commit.subject}>{commit.subject}</span><small>{commit.committed_at} · {commit.author}</small></li>)}</ol></div>}</section>;
}
