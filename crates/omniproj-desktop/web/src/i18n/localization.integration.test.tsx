// Closes the gap where component tests render without an I18nProvider and so only ever
// exercise the English fallback of the default context. Here a REAL component tree is
// rendered under a live provider in each locale, asserting the visible copy actually
// switches — this is what guards the Chinese-first default from silently regressing to
// English (a component wrongly pinned to English would pass the fallback-only tests).

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import type { ReviewPolicy } from "../domain/project";
import { indexItem } from "../test/fixtures";
import { ProjectsIndex } from "../components/projects/ProjectsIndex";
import { I18nProvider } from "./I18nProvider";
import type { Locale } from "./I18nProvider";

const NOW = new Date("2026-08-12T12:00:00Z");
const POLICY: ReviewPolicy = { commitment_review_days: 7, rule_version: "r0-v1" };

// A long, unique, always-rendered string, so the assertion can't collide with a badge.
const REVIEW_ORDER_ZH = "关注顺序（按静默事实，不代表优先级或健康度）";
const REVIEW_ORDER_EN = "Attention order (factual silence, not priority or health)";

function renderLocalizedIndex(locale: Locale) {
  return render(
    <I18nProvider initialLocale={locale}>
      <MemoryRouter initialEntries={["/projects"]}>
        <ProjectsIndex
          projects={[indexItem()]}
          reviewPolicy={POLICY}
          now={NOW}
          onAddProject={() => {}}
        />
      </MemoryRouter>
    </I18nProvider>,
  );
}

describe("localized rendering of a real component tree", () => {
  it("renders the Chinese catalog under the zh-CN default", () => {
    renderLocalizedIndex("zh-CN");
    expect(screen.getByText(REVIEW_ORDER_ZH)).toBeInTheDocument();
    expect(screen.getByText("全部")).toBeInTheDocument();
    expect(screen.queryByText(REVIEW_ORDER_EN)).not.toBeInTheDocument();
  });

  it("renders the English catalog under en", () => {
    renderLocalizedIndex("en");
    expect(screen.getByText(REVIEW_ORDER_EN)).toBeInTheDocument();
    expect(screen.getByText("All")).toBeInTheDocument();
    expect(screen.queryByText(REVIEW_ORDER_ZH)).not.toBeInTheDocument();
  });
});
