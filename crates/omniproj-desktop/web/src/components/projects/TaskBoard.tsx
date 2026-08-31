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
  const [drafts, setDrafts] = useState<Record<string, { status: string; due: string; note: string }>>({});
  const { data: rawData, isLoading } = useQuery({ queryKey: ["tasks", projectId], queryFn: () => api.getTasks(projectId) });
  const data: Task[] = Array.isArray(rawData) ? rawData : [];
  const reload = () => client.invalidateQueries({ queryKey: ["tasks", projectId] });
  async function add() { if (!text.trim()) return; await api.addTask({ project_id: projectId, text: text.trim(), unclear }); setText(""); setUnclear(false); reload(); }
  function draft(task: Task) { return drafts[task.id] ?? { status: task.status, due: task.due ?? "", note: task.note ?? "" }; }
  async function update(task: Task) { const value = draft(task); await api.updateTask({ project_id: projectId, id: task.id, status: value.status, due: value.due || null, note: value.note || null }); reload(); }
  async function advance(task: Task) {
    const steps = await api.advanceTask({ project_id: projectId, id: task.id });
    if (steps.length) { const accepted = window.confirm(`${t("task.advanceReady")}\n\n${steps.map((s) => `• ${s}`).join("\n")}`); if (accepted) { await api.adoptSubtasks({ project_id: projectId, texts: steps }); reload(); } }
  }
  return <section className="op-section" aria-labelledby="tasks-heading" data-testid="task-board">
    <div className="op-section__header"><div><p className="op-section__kicker">{t("task.kicker")}</p><h3 id="tasks-heading">{t("task.title")}</h3></div><span className="op-section__count">{data.length}</span></div>
    <div className="op-task-add"><input aria-label={t("task.new")} placeholder={t("task.new")} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void add(); }} /><label><input type="checkbox" checked={unclear} onChange={(e) => setUnclear(e.target.checked)} /> {t("task.unclear")}</label><button className="op-button op-button--primary" type="button" disabled={!text.trim()} onClick={() => void add()}>{t("task.add")}</button></div>
    {isLoading ? <p className="op-muted">{t("task.loading")}</p> : data.length === 0 ? <p className="op-muted">{t("task.empty")}</p> : <ul className="op-task-list">{data.map((task) => { const value = draft(task); return <li key={task.id} className="op-task-item"><div className="op-task-main"><span className={task.unclear ? "op-task-unclear" : ""}>{task.unclear ? "? " : ""}{task.text}</span><input type="date" aria-label={`${t("task.due")}: ${task.text}`} value={value.due} onChange={(e) => setDrafts((all) => ({ ...all, [task.id]: { ...value, due: e.target.value } }))} /><input aria-label={`${t("task.note")}: ${task.text}`} placeholder={t("task.note")} value={value.note} onChange={(e) => setDrafts((all) => ({ ...all, [task.id]: { ...value, note: e.target.value } }))} /></div><div className="op-task-actions"><select aria-label={`${t("task.status")}: ${task.text}`} value={value.status} onChange={(e) => setDrafts((all) => ({ ...all, [task.id]: { ...value, status: e.target.value } }))}><option value="open">{t("task.open")}</option><option value="doing">{t("task.doing")}</option><option value="done">{t("task.done")}</option></select><button className="op-button op-button--secondary" type="button" onClick={() => void update(task)}>{t("task.save")}</button>{task.unclear && <button className="op-button op-button--ghost" type="button" onClick={() => void advance(task)}>{t("task.advance")}</button>}<button className="op-button op-button--ghost" type="button" onClick={() => void api.removeTask({ project_id: projectId, id: task.id }).then(reload)}>{t("task.remove")}</button></div></li>; })}</ul>}
  </section>;
}
