// Closes the gap where component tests render without an I18nProvider and so only ever
// exercise the English fallback of the default context. Here a REAL component tree is
// rendered under a live provider in each locale, asserting the visible copy actually
// switches — this is what guards the Chinese-first default from silently regressing to
// English (a component wrongly pinned to English would pass the fallback-only tests).

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { indexItem } from "../test/fixtures";
import { ProjectsIndex } from "../components/projects/ProjectsIndex";
import { I18nProvider } from "./I18nProvider";
import type { Locale } from "./I18nProvider";

const NOW = new Date("2026-08-12T12:00:00Z");

// A long, unique, always-rendered string, so the assertion can't collide with a badge.
const SORT_ZH = "排序";
const SORT_EN = "Sort";

function renderLocalizedIndex(locale: Locale) {
  return render(
    <I18nProvider initialLocale={locale}>
      <MemoryRouter initialEntries={["/projects"]}>
        <ProjectsIndex
          projects={[indexItem()]}
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
    expect(screen.getByText(SORT_ZH)).toBeInTheDocument();
    expect(screen.getByText("已归档")).toBeInTheDocument();
    expect(screen.queryByText(SORT_EN)).not.toBeInTheDocument();
  });

  it("renders the English catalog under en", () => {
    renderLocalizedIndex("en");
    expect(screen.getByText(SORT_EN)).toBeInTheDocument();
    expect(screen.getByText("Archived")).toBeInTheDocument();
    expect(screen.queryByText(SORT_ZH)).not.toBeInTheDocument();
  });
});
