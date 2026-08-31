import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api";
import type { ReminderSettings as Settings } from "../domain/project";
import { useI18n } from "../i18n/I18nProvider";

export function ReminderSettings() {
  const { t } = useI18n();
  const client = useQueryClient();
  const { data } = useQuery({ queryKey: ["reminder-settings"], queryFn: api.getReminderSettings });
  const [form, setForm] = useState<Settings | null>(null);
  useEffect(() => { if (data && !form) setForm(data); }, [data, form]);
  const save = useMutation({ mutationFn: api.setReminderSettings, onSuccess: (next) => { setForm(next); client.invalidateQueries({ queryKey: ["reminder-settings"] }); } });
  const test = useMutation({ mutationFn: api.testReminder });
  const s = form ?? data;
  if (!s) return null;
  return <section className="op-section" aria-labelledby="settings-heading" data-testid="reminder-settings"><div className="op-section__header"><div><p className="op-section__kicker">{t("settings.kicker")}</p><h3 id="settings-heading">{t("settings.title")}</h3></div></div><div className="op-settings-grid"><label><input type="checkbox" checked={s.enabled} onChange={(e) => setForm({ ...s, enabled: e.target.checked })} /> {t("settings.enabled")}</label><label>{t("settings.cadence")} <select value={s.cadence} onChange={(e) => setForm({ ...s, cadence: e.target.value as Settings["cadence"] })}><option value="daily">{t("settings.daily")}</option><option value="off">{t("settings.off")}</option></select></label><label>{t("settings.threshold")} <input type="number" min={0} max={3650} value={s.silent_days_threshold} onChange={(e) => setForm({ ...s, silent_days_threshold: Math.max(0, Number(e.target.value)) })} /></label></div><div className="op-task-actions"><button className="op-button op-button--secondary" type="button" disabled={save.isPending} onClick={() => void save.mutate(s)}>{t("settings.save")}</button><button className="op-button op-button--ghost" type="button" disabled={test.isPending} onClick={() => void test.mutate()}>{t("settings.test")}</button>{save.isSuccess && <span className="op-muted" role="status">{t("settings.saved")}</span>}{test.isSuccess && <span className="op-muted" role="status">{t("settings.tested")}</span>}</div></section>;
}
