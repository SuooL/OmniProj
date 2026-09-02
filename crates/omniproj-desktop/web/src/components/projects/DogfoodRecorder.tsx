import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../../api";
import type { ProjectId } from "../../domain/project";
import { useI18n } from "../../i18n/I18nProvider";

export function DogfoodRecorder({ projectId }: { projectId: ProjectId }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const startedAt = useRef<number | null>(null);
  const [running, setRunning] = useState(false);
  const key = ["dogfood-summary"] as const;
  const { data } = useQuery({ queryKey: key, queryFn: api.getDogfoodSummary });
  const record = useMutation({
    mutationFn: api.recordReentryEvent,
    onSuccess: (summary) => client.setQueryData(key, summary),
  });
  function start() {
    startedAt.current = Date.now();
    setRunning(true);
  }
  function complete() {
    if (startedAt.current === null) return;
    const durationSeconds = Math.max(1, Math.round((Date.now() - startedAt.current) / 1000));
    record.mutate({ project_id: projectId, duration_seconds: durationSeconds });
    startedAt.current = null;
    setRunning(false);
  }
  return <section className="op-section" aria-labelledby="dogfood-heading" data-testid="dogfood-recorder"><div className="op-section__header"><div><p className="op-section__kicker">{t("dogfood.kicker")}</p><h3 id="dogfood-heading">{t("dogfood.title")}</h3></div></div><p className="op-muted">{t("dogfood.summary", { events: data?.event_count ?? 0, projects: data?.project_count ?? 0, median: data?.median_duration_seconds ?? "—" })}</p>{running ? <button className="op-button op-button--primary" type="button" onClick={complete}>{t("dogfood.ready")}</button> : <button className="op-button op-button--secondary" type="button" onClick={start}>{t("dogfood.start")}</button>}{record.isSuccess && <span className="op-muted" role="status">{t("dogfood.recorded")}</span>}</section>;
}
