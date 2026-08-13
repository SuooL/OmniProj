// Row-level contract: four fields, the badge budget, the observed-fact edge cases, and the
// absence of forbidden row content (no CommitmentStateTag in the Index, no path, no ranking).

import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { projectId } from "../../domain/project";
import {
  currentCommitment,
  indexItem,
  observedActual,
  reviewReason,
} from "../../test/fixtures";
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

describe("four fields behind one canonical link", () => {
  it("links to the canonical Overview with the project name as its accessible name", () => {
    renderRow(indexItem({ project_id: projectId("p-42"), name: "Atlas" }));
    const link = screen.getByRole("link", { name: /^Atlas\b/ });
    expect(link).toHaveAttribute("href", "/projects/p-42/overview");
    // The composed accessible name conveys the whole row, not just the project name.
    expect(link).toHaveAccessibleName(/Atlas\. .*Commitment/);
  });

  it("shows the commitment text and the observed branch, SHA subject, and time", () => {
    renderRow(
      indexItem({
        name: "Atlas",
        current_commitment: currentCommitment({ text: "Wire the service" }),
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
    expect(screen.getByText("Wire the service")).toBeInTheDocument();
    expect(screen.getByText("feature/x")).toBeInTheDocument();
    expect(screen.getByText(/bbbbbbb add thing/)).toBeInTheDocument();
  });
});

describe("badge budget", () => {
  it("renders at most one state tag, one review badge, three fact labels, and no commitment tag", () => {
    const { container } = renderRow(
      indexItem({
        status: "waiting",
        review_reasons: [
          reviewReason("needs_commitment"),
          reviewReason("review_action"),
        ],
      }),
    );
    expect(container.querySelectorAll("[data-state]")).toHaveLength(1); // ProjectStateTag
    expect(container.querySelectorAll("[data-reason]")).toHaveLength(1); // primary ReviewSignalBadge
    expect(container.querySelectorAll(".op-fact").length).toBeLessThanOrEqual(3);
    expect(container.querySelectorAll("[data-commit-status]")).toHaveLength(0); // NOT in Index
  });

  it("shows a plain +N for extra review reasons, not a second badge", () => {
    renderRow(
      indexItem({
        review_reasons: [
          reviewReason("needs_commitment"),
          reviewReason("review_action"),
          reviewReason("scheduled_review"),
        ],
      }),
    );
    const plusN = screen.getByText("+2");
    expect(plusN).not.toHaveClass("op-badge");
    expect(plusN).toHaveAttribute("aria-label", expect.stringContaining("2 more review reasons"));
  });
});

describe("observed-actual edge cases", () => {
  it("labels a detached HEAD", () => {
    renderRow(indexItem({ observed_actual: observedActual({ head: { kind: "detached" } }) }));
    expect(screen.getByText("detached HEAD")).toBeInTheDocument();
  });

  it("labels an unborn branch and no commits", () => {
    renderRow(
      indexItem({
        observed_actual: observedActual({
          head: { kind: "unborn", branch: "main" },
          last_commit: null,
        }),
      }),
    );
    expect(screen.getByText("main (unborn)")).toBeInTheDocument();
    expect(screen.getByText("no commits")).toBeInTheDocument();
  });

  it("carries the exact observed timestamp as a title", () => {
    renderRow(
      indexItem({
        observed_actual: observedActual({ observed_at: "2026-08-10T08:30:00Z" }),
      }),
    );
    expect(screen.getByTitle("2026-08-10T08:30:00Z")).toBeInTheDocument();
  });

  it("says Not yet observed when there is no observation", () => {
    renderRow(indexItem({ observed_actual: null, current_commitment: null }));
    expect(screen.getByText("Not yet observed")).toBeInTheDocument();
  });
});

describe("commitment states", () => {
  it("shows a missing-commitment placeholder", () => {
    renderRow(indexItem({ current_commitment: null }));
    expect(screen.getByText("No current commitment")).toBeInTheDocument();
  });

  it("shows the natural-language commits-since delta without claiming progress", () => {
    renderRow(
      indexItem({
        observed_actual: observedActual({ commits_since_commitment: 3 }),
      }),
    );
    expect(screen.getByText("3 commits since")).toBeInTheDocument();
  });
});

describe("no forbidden content", () => {
  it("never renders the source path, a sparkline, or a ranking control", () => {
    const { container } = renderRow(
      indexItem({ name: "Atlas" }),
    );
    // The Index DTO carries no source location; assert none leaked into the row.
    const row = within(container.querySelector("li") as HTMLElement);
    expect(row.queryByText(/\/Users\//)).not.toBeInTheDocument();
    expect(container.querySelector("[data-testid='sparkline']")).toBeNull();
    expect(container.querySelector("[data-testid='health']")).toBeNull();
    expect(container.querySelector("[data-testid='git-graph']")).toBeNull();
  });
});
