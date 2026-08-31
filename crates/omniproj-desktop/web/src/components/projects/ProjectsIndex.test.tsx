// Index-level contract: visible column headers, the review-order label and DTO-sourced review
// interval, deterministic order preservation (NEVER re-ranked), transparent opt-in sort, the
// text/review filters, the empty-state recovery action, and archived recovery view.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { projectId } from "../../domain/project";
import type { ReviewPolicy } from "../../domain/project";
import { REVIEW_ORDER_LABEL } from "../../domain/projectPresentation";
import { indexItem, reviewReason } from "../../test/fixtures";
import { ProjectsIndex } from "./ProjectsIndex";

const NOW = new Date("2026-08-12T12:00:00Z");
const POLICY: ReviewPolicy = { commitment_review_days: 7, rule_version: "r0-v1" };

function renderIndex(
  projects = [indexItem()],
  opts: { url?: string; policy?: ReviewPolicy; onAddProject?: () => void } = {},
) {
  return render(
    <MemoryRouter initialEntries={[opts.url ?? "/projects"]}>
      <ProjectsIndex
        projects={projects}
        reviewPolicy={opts.policy ?? POLICY}
        now={NOW}
        onAddProject={opts.onAddProject ?? (() => {})}
      />
    </MemoryRouter>,
  );
}

// The row link's accessible name is a composed summary; the stable visible name is .op-row__name.
function linkOrder(): string[] {
  return screen
    .getAllByRole("link")
    .map((l) => l.querySelector(".op-row__name")?.textContent ?? "");
}

describe("headers and policy", () => {
  it("shows a semantic Projects list without browser-style table headers", () => {
    const { container } = renderIndex();
    expect(screen.getByRole("list", { name: "Projects" })).toBeInTheDocument();
    expect(container.querySelector(".op-index__head")).not.toBeInTheDocument();
  });

  it("labels the order as review order and never as priority", () => {
    renderIndex();
    expect(screen.getByText(REVIEW_ORDER_LABEL)).toBeInTheDocument();
    expect(
      screen.getByRole("combobox", { name: /review order/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("combobox", { name: /priority/i }),
    ).not.toBeInTheDocument();
  });

  it("shows the review interval from the DTO review_policy, not a frontend constant", () => {
    renderIndex([indexItem()], { policy: { commitment_review_days: 5, rule_version: "r0-v1" } });
    expect(screen.getByText("Commitment review interval: 5 days")).toBeInTheDocument();
  });
});

describe("deterministic order and transparent sort", () => {
  const projects = [
    indexItem({ project_id: projectId("c"), name: "Charlie" }),
    indexItem({ project_id: projectId("a"), name: "Alpha" }),
    indexItem({ project_id: projectId("b"), name: "Bravo" }),
  ];

  it("preserves the backend review order by default (no re-ranking)", () => {
    renderIndex(projects);
    expect(linkOrder()).toEqual(["Charlie", "Alpha", "Bravo"]);
  });

  it("applies a transparent name sort only when opted in", () => {
    renderIndex(projects, { url: "/projects?sort=name" });
    expect(linkOrder()).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("never hoists a high-priority review reason above the received order", () => {
    // The backend would sort a source_unavailable project first; here it arrives LAST. The
    // frontend must render it last (it preserves the backend order and never re-ranks by reason).
    const outOfOrder = [
      indexItem({ project_id: projectId("x"), name: "Xray", review_reasons: [] }),
      indexItem({ project_id: projectId("y"), name: "Yankee", review_reasons: [] }),
      indexItem({
        project_id: projectId("z"),
        name: "Zulu",
        review_reasons: [reviewReason("source_unavailable")],
      }),
    ];
    renderIndex(outOfOrder);
    expect(linkOrder()).toEqual(["Xray", "Yankee", "Zulu"]);
  });
});

describe("filters", () => {
  it("filters by name from the q search param", () => {
    renderIndex(
      [indexItem({ name: "Atlas" }), indexItem({ project_id: projectId("z"), name: "Zephyr" })],
      { url: "/projects?q=zep" },
    );
    expect(linkOrder()).toEqual(["Zephyr"]);
  });

  it("filters to needs-review via the chip", async () => {
    const user = userEvent.setup();
    renderIndex([
      indexItem({ project_id: projectId("needs"), name: "Needs", review_reasons: [reviewReason("review_action")] }),
      indexItem({ project_id: projectId("clean"), name: "Clean", review_reasons: [] }),
    ]);
    expect(linkOrder()).toEqual(["Needs", "Clean"]);

    await user.click(screen.getByRole("button", { name: "Needs review" }));
    expect(linkOrder()).toEqual(["Needs"]);
  });
});

describe("empty and archived", () => {
  it("offers a focusable Add project action when there are no projects", async () => {
    const user = userEvent.setup();
    const onAddProject = vi.fn();
    renderIndex([], { onAddProject });
    const button = screen.getByRole("button", { name: /add project/i });
    button.focus();
    expect(button).toHaveFocus();
    await user.click(button);
    expect(onAddProject).toHaveBeenCalledTimes(1);
  });

  it("keeps archived projects out of the operating view but exposes an Archived filter", async () => {
    const user = userEvent.setup();
    renderIndex([
      indexItem({ project_id: projectId("live"), name: "Live", status: "active" }),
      indexItem({ project_id: projectId("gone"), name: "Gone", status: "archived" }),
    ]);
    expect(linkOrder()).toEqual(["Live"]);
    await user.click(screen.getByRole("button", { name: "Archived" }));
    expect(linkOrder()).toEqual(["Gone"]);
  });
});
