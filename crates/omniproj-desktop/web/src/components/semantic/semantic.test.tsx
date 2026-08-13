// Behavioral contract for the constrained semantic components, plus a source scan that fails
// if any component names a raw hex color, an old --color-* variable, or an emoji. Color must
// always be redundant with visible text.

/// <reference types="vite/client" />
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ActivityStamp } from "./ActivityStamp";
import { CommitmentStateTag } from "./CommitmentStateTag";
import { FactLabel } from "./FactLabel";
import { FilterChip } from "./FilterChip";
import { ProjectStateTag } from "./ProjectStateTag";
import { ReviewSignalBadge } from "./ReviewSignalBadge";

// Raw source of each component, read through Vite's ?raw loader (no node:fs, no node types).
import activityStampSrc from "./ActivityStamp.tsx?raw";
import commitmentStateSrc from "./CommitmentStateTag.tsx?raw";
import factLabelSrc from "./FactLabel.tsx?raw";
import filterChipSrc from "./FilterChip.tsx?raw";
import projectStateSrc from "./ProjectStateTag.tsx?raw";
import reviewSignalSrc from "./ReviewSignalBadge.tsx?raw";
import toneSrc from "./tone.ts?raw";

describe("visible text independent of color", () => {
  it("ProjectStateTag shows the state word for each exception, and nothing for active", () => {
    const { rerender } = render(<ProjectStateTag status="waiting" />);
    expect(screen.getByText("Waiting")).toBeInTheDocument();
    rerender(<ProjectStateTag status="parked" />);
    expect(screen.getByText("Parked")).toBeInTheDocument();
    rerender(<ProjectStateTag status="archived" />);
    expect(screen.getByText("Archived")).toBeInTheDocument();
    rerender(<ProjectStateTag status="setup" />);
    expect(screen.getByText("Setup")).toBeInTheDocument();

    const { container } = render(<ProjectStateTag status="active" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("CommitmentStateTag names each commitment state", () => {
    const states = [
      ["planned", "Planned"],
      ["doing", "Doing"],
      ["blocked", "Blocked"],
      ["done", "Done"],
      ["abandoned", "Abandoned"],
    ] as const;
    for (const [status, label] of states) {
      const { unmount } = render(<CommitmentStateTag status={status} />);
      expect(screen.getByText(label)).toBeInTheDocument();
      unmount();
    }
  });

  it("FactLabel and ActivityStamp render neutral text with an exact title", () => {
    render(<FactLabel label="branch" value="main" title="refs/heads/main" />);
    const fact = screen.getByText(/main/);
    expect(fact).toHaveAttribute("title", "refs/heads/main");

    render(<ActivityStamp verb="Completed" text="3 days ago" title="2026-08-09T00:00:00Z" />);
    const stamp = screen.getByText(/Completed/);
    expect(stamp).toHaveTextContent("Completed · 3 days ago");
    expect(stamp).toHaveAttribute("title", "2026-08-09T00:00:00Z");
  });
});

describe("ReviewSignalBadge +N", () => {
  it("shows the primary label and a plain +N whose accessible name enumerates hidden reasons", () => {
    render(
      <ReviewSignalBadge
        reason={{ code: "needs_commitment", label: "Needs commitment" }}
        hidden={[{ label: "Review action" }, { label: "Scheduled review" }]}
      />,
    );
    expect(screen.getByText("Needs commitment")).toBeInTheDocument();
    const plusN = screen.getByText("+2");
    expect(plusN).toHaveAttribute(
      "aria-label",
      "2 more review reasons: Review action, Scheduled review",
    );
    // +N is not a second enclosed badge.
    expect(plusN).not.toHaveClass("op-badge");
  });

  it("omits +N when there are no hidden reasons", () => {
    render(<ReviewSignalBadge reason={{ code: "source_unavailable", label: "Repo unavailable" }} />);
    expect(screen.queryByText(/^\+/)).not.toBeInTheDocument();
  });
});

describe("FilterChip exposes pressed state", () => {
  it("reflects aria-pressed and fires onClick", () => {
    let clicks = 0;
    render(<FilterChip label="Needs review" pressed onClick={() => (clicks += 1)} />);
    const chip = screen.getByRole("button", { name: "Needs review" });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    chip.click();
    expect(clicks).toBe(1);
  });
});

describe("source discipline", () => {
  const files: Array<[string, string]> = [
    ["tone.ts", toneSrc],
    ["ProjectStateTag.tsx", projectStateSrc],
    ["ReviewSignalBadge.tsx", reviewSignalSrc],
    ["CommitmentStateTag.tsx", commitmentStateSrc],
    ["FactLabel.tsx", factLabelSrc],
    ["ActivityStamp.tsx", activityStampSrc],
    ["FilterChip.tsx", filterChipSrc],
  ];

  it.each(files)("%s references no raw hex, no --color-*, and no emoji", (_file, source) => {
    expect(source).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(source).not.toMatch(/--color-/);
    expect(source).not.toMatch(/\p{Extended_Pictographic}/u);
  });
});
