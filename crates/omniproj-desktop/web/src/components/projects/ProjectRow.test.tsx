// Row-level contract: what the row reports, the observed-fact edge cases, and the absence
// of the signals the redesign removed (no review badge, no commitment text, no ranking).

import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { projectId } from "../../domain/project";
import { indexItem, observedActual, reviewReason } from "../../test/fixtures";
import { ProjectRow } from "./ProjectRow";

const NOW = new Date("2026-08-12T12:00:00Z");

function renderRow(item = indexItem()) {
  return render(
    <MemoryRouter>
      <ul>
        <ProjectRow item={item} now={NOW} />
      </ul>
    </MemoryRouter>,
  );
}

describe("what the row reports", () => {
  it("links to the canonical Overview, and its accessible name conveys the whole row", () => {
    renderRow(indexItem({ project_id: projectId("p-42"), name: "Atlas" }));
    const link = screen.getByRole("link", { name: /^Atlas\b/ });
    expect(link).toHaveAttribute("href", "/projects/p-42/overview");
    expect(link).toHaveAccessibleName(/Atlas\. .*steps left/);
  });

  it("shows how much is left, the branch, and when it was last touched", () => {
    renderRow(
      indexItem({
        name: "Atlas",
        open_steps: 3,
        total_steps: 7,
        observed_actual: observedActual({
          head: { kind: "attached", branch: "feature/x" },
          last_commit: {
            sha: "b".repeat(40),
            short_sha: "bbbbbbb",
            subject: "add thing",
            committed_at: "2026-08-11T00:00:00Z",
          },
        }),
      }),
    );
    expect(screen.getByText("3 of 7 steps left")).toBeInTheDocument();
    expect(screen.getByText("feature/x")).toBeInTheDocument();
    // Commit subjects belong on the project page, not in a list row.
    expect(screen.queryByText(/add thing/)).not.toBeInTheDocument();
  });

  it("says so plainly when a project has no steps at all", () => {
    renderRow(indexItem({ open_steps: 0, total_steps: 0 }));
    expect(screen.getByText("No steps yet")).toBeInTheDocument();
  });

  it("carries a compact activity strip drawn from the observed weekly counts", () => {
    const { container } = renderRow(
      indexItem({ observed_actual: observedActual({ commit_activity_weeks: [0, 1, 4, 2] }) }),
    );
    expect(container.querySelector("[data-testid='activity-sparkline']")).not.toBeNull();
  });
});

describe("observed-actual edge cases", () => {
  it("labels a detached HEAD", () => {
    renderRow(indexItem({ observed_actual: observedActual({ head: { kind: "detached" } }) }));
    expect(screen.getByText("Detached HEAD")).toBeInTheDocument();
  });

  it("labels an unborn branch", () => {
    renderRow(
      indexItem({
        observed_actual: observedActual({
          head: { kind: "unborn", branch: "main" },
          last_commit: null,
        }),
      }),
    );
    expect(screen.getByText("main (unborn, no commits yet)")).toBeInTheDocument();
  });

  it("carries the exact observed timestamp as a title", () => {
    renderRow(
      indexItem({ observed_actual: observedActual({ observed_at: "2026-08-10T08:30:00Z" }) }),
    );
    expect(screen.getByTitle("2026-08-10T08:30:00Z")).toBeInTheDocument();
  });

  it("says Not yet observed when there is no observation", () => {
    renderRow(indexItem({ observed_actual: null }));
    expect(screen.getByText("Not yet observed")).toBeInTheDocument();
  });
});

describe("signals the row must not carry", () => {
  it("shows no review badge, no commitment text, no source path, and no ranking", () => {
    const { container } = renderRow(
      indexItem({
        name: "Atlas",
        status: "waiting",
        review_reasons: [reviewReason("needs_commitment"), reviewReason("review_action")],
      }),
    );
    const row = within(container.querySelector("li") as HTMLElement);
    expect(container.querySelectorAll("[data-reason]")).toHaveLength(0);
    expect(row.queryByText(/commitment/i)).not.toBeInTheDocument();
    expect(row.queryByText(/\/Users\//)).not.toBeInTheDocument();
    expect(container.querySelector("[data-testid='health']")).toBeNull();
    expect(container.querySelector("[data-testid='git-graph']")).toBeNull();
    // One lifecycle tag is still allowed; it is a fact the user set, not a judgement.
    expect(container.querySelectorAll("[data-state]")).toHaveLength(1);
  });
});
