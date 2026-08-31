import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api";
import type { ProjectId, Task } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function TaskBoard({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const [text, setText] = useState("");
  const [unclear, setUnclear] = useState(false);
  const { data: rawData, isLoading } = useQuery({ queryKey: ["tasks", projectId], queryFn: () => api.getTasks(projectId) });
  const data: Task[] = Array.isArray(rawData) ? rawData : [];
  const reload = () => client.invalidateQueries({ queryKey: ["tasks", projectId] });
  async function add() { if (!text.trim()) return; await api.addTask({ project_id: projectId, text: text.trim(), unclear }); setText(""); setUnclear(false); reload(); }
  async function update(task: Task, status: string) { await api.updateTask({ project_id: projectId, id: task.id, status, due: task.due, note: task.note }); reload(); }
  async function advance(task: Task) {
    const steps = await api.advanceTask({ project_id: projectId, id: task.id });
    if (steps.length) { const accepted = window.confirm(`${t("task.advanceReady")}\n\n${steps.map((s) => `• ${s}`).join("\n")}`); if (accepted) { await api.adoptSubtasks({ project_id: projectId, texts: steps }); reload(); } }
  }
  return <section className="op-section" aria-labelledby="tasks-heading" data-testid="task-board">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("task.kicker")}</p><h3 id="tasks-heading">{t("task.title")}</h3></div><span className="op-section__count">{data.length}</span></div>
    <div className="op-task-add"><input aria-label={t("task.new")} placeholder={t("task.new")} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void add(); }} /><label><input type="checkbox" checked={unclear} onChange={(e) => setUnclear(e.target.checked)} /> {t("task.unclear")}</label><button className="op-button op-button--primary" type="button" disabled={!text.trim()} onClick={() => void add()}>{t("task.add")}</button></div>
    {isLoading ? <p className="op-muted">{t("task.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("task.empty")}</p> : <ul className="op-task-list">{data.map((task) => <li key={task.id} className="op-task-item"><div className="op-task-main"><span className={task.unclear ? "op-task-unclear" : ""}>{task.unclear ? "? " : ""}{task.text}</span>{task.note && <small>{task.note}</small>}{task.due && <small>{t("task.due", { date: task.due })}</small>}</div><div className="op-task-actions"><select aria-label={`${t("task.status")}: ${task.text}`} value={task.status} onChange={(e) => void update(task, e.target.value)}><option value="open">{t("task.open")}</option><option value="doing">{t("task.doing")}</option><option value="done">{t("task.done")}</option></select>{task.unclear && <button className="op-button op-button--ghost" type="button" onClick={() => void advance(task)}>{t("task.advance")}</button>}<button className="op-button op-button--ghost" type="button" onClick={() => void api.removeTask({ project_id: projectId, id: task.id }).then(reload)}>{t("task.remove")}</button></div></li>)}</ul>}
  </section>;
}
