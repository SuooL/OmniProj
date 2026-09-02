// The cross-project focus strip (R1e, FR-A5): a read-only, collapsible aggregate of
// overdue + due-today tasks across Active projects, rendered above the Projects Index.
// It renders NOTHING when there is nothing due (zero-value states are omitted), and it
// never edits: every entry is a jump into its project, where editing lives.

import { useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";

import { api } from "../../api";
import { projectOverviewPath } from "../../domain/routes";
import { toneStyle } from "../semantic/tone";
import { useI18n } from "../../i18n/I18nProvider";

export function FocusStrip() {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const { data } = useQuery({ queryKey: ["focus-agenda"], queryFn: api.getFocusAgenda });

  if (!data || data.total_items === 0) return null;

  return (
    <section className="op-focus-strip" data-testid="focus-strip" aria-labelledby="focus-strip-heading">
      <button
        type="button"
        className="op-focus-strip__toggle"
        aria-expanded={expanded}
        onClick={() => setExpanded((value) => !value)}
      >
        <span id="focus-strip-heading" className="op-focus-strip__title">{t("focus.title")}</span>
        <span>{t("focus.summary", { projects: data.projects.length, items: data.total_items })}</span>
        <span aria-hidden="true">{expanded ? "▾" : "▸"}</span>
      </button>
      {expanded && (
        <ul className="op-focus-strip__groups">
          {data.projects.map((project) => (
            <li key={project.project_id}>
              <Link className="op-focus-strip__project" to={projectOverviewPath(project.project_id)}>
                {project.name}
              </Link>
              <ul className="op-focus-strip__items">
                {project.items.map((item) => (
                  <li key={item.id}>
                    <span
                      className="op-badge"
                      style={toneStyle(item.overdue_days > 0 ? "danger" : "warning")}
                    >
                      {item.overdue_days > 0 ? t("board.overdue", { days: item.overdue_days }) : t("board.dueToday")}
                    </span>
                    <span className="op-focus-strip__text">{item.text}</span>
                    <span className="op-focus-strip__due">{item.due}</span>
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
