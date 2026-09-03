import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../api";
import { useI18n } from "../i18n/I18nProvider";

export function AgentSettings() {
  const { t } = useI18n();
  const client = useQueryClient();
  const { data } = useQuery({ queryKey: ["agent-settings"], queryFn: api.getAgentSettings });
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [consent, setConsent] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => {
    if (!data || model) return;
    setModel(data.default_model);
    setConsent(data.remote_consent);
  }, [data, model]);
  const save = useMutation({
    mutationFn: () => api.setAgentSettings({ default_model: model.trim(), api_key: apiKey.trim() || null, remote_consent: consent }),
    onSuccess: (next) => {
      client.setQueryData(["agent-settings"], next);
      setApiKey("");
      setMessage(t("agent.saved"));
    },
    onError: (error) => setMessage(error instanceof Error ? error.message : t("agent.saveFailed")),
  });
  const test = useMutation({
    mutationFn: api.testAgentProvider,
    onSuccess: () => setMessage(t("agent.tested")),
    onError: (error) => setMessage(error instanceof Error ? error.message : t("agent.testFailed")),
  });
  if (!data || !Array.isArray(data.providers)) return null;
  const providerName = model.split("/", 1)[0] || data.selected_provider;
  const selected = data.providers.find((provider) => provider.name === providerName);
  const switchProvider = (provider: string) => {
    const currentModel = model.includes("/") ? model.slice(model.indexOf("/") + 1) : "";
    setModel(`${provider}/${currentModel}`);
    setMessage("");
  };
  return (
    <section className="op-section" aria-labelledby="agent-settings-heading" data-testid="agent-settings">
      <div className="op-section__header"><div><p className="op-section__kicker">{t("agent.kicker")}</p><h3 id="agent-settings-heading">{t("agent.title")}</h3></div><span className="op-section__count">{data.ready ? t("agent.ready") : t("agent.notReady")}</span></div>
      <p className="op-muted">{t("agent.privacy")}</p>
      <div className="op-settings-grid">
        <label>{t("agent.provider")}<select value={selected?.name ?? data.selected_provider} onChange={(event) => switchProvider(event.target.value)}>{data.providers.map((provider) => <option key={provider.name} value={provider.name}>{provider.name}{provider.local ? ` · ${t("agent.local")}` : ""}</option>)}</select></label>
        <label>{t("agent.model")}<input value={model.includes("/") ? model.slice(model.indexOf("/") + 1) : ""} onChange={(event) => setModel(`${selected?.name ?? data.selected_provider}/${event.target.value}`)} /></label>
        {selected?.key_required && <label>{t("agent.apiKey")}<input type="password" autoComplete="off" value={apiKey} placeholder={selected.key_present ? t("agent.keyStored") : t("agent.keyRequired")} onChange={(event) => setApiKey(event.target.value)} /></label>}
      </div>
      {selected && !selected.local && <label className="op-consent"><input type="checkbox" checked={consent} onChange={(event) => setConsent(event.target.checked)} /> {t("agent.consent")}</label>}
      <div className="op-task-actions"><button className="op-button op-button--secondary" type="button" disabled={save.isPending || !model.includes("/")} title={!model.includes("/") ? t("agent.saveDisabled") : undefined} onClick={() => save.mutate()}>{t("agent.save")}</button><button className="op-button op-button--ghost" type="button" disabled={test.isPending || !data.ready} title={!data.ready ? t("agent.testDisabled") : undefined} onClick={() => test.mutate()}>{test.isPending ? t("agent.testing") : t("agent.test")}</button>{message && <span role="status" className={save.isError || test.isError ? "op-error" : "op-muted"}>{message}</span>}</div>
    </section>
  );
}
