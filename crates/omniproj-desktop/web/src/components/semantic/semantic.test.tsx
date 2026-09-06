// Behavioral contract for the constrained semantic components, plus a source scan that fails
// if any component names a raw hex color, an old --color-* variable, or an emoji. Color must
// always be redundant with visible text.

/// <reference types="vite/client" />
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FilterChip } from "./FilterChip";
import { ProjectStateTag } from "./ProjectStateTag";

// Raw source of each component, read through Vite's ?raw loader (no node:fs, no node types).
import filterChipSrc from "./FilterChip.tsx?raw";
import projectStateSrc from "./ProjectStateTag.tsx?raw";
import toneSrc from "./tone.ts?raw";
import dateFieldSrc from "./DateField.tsx?raw";
import tagFieldSrc from "./TagField.tsx?raw";
import heatmapSrc from "../projects/CommitHeatmap.tsx?raw";
import outlineRowSrc from "../projects/tasks/OutlineRow.tsx?raw";
import dueBadgeSrc from "../projects/tasks/TaskDueBadge.tsx?raw";

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
    ["FilterChip.tsx", filterChipSrc],
    ["DateField.tsx", dateFieldSrc],
    ["TagField.tsx", tagFieldSrc],
    // The outline and the heatmap are where colour is most tempting; they are held to
    // the same rule as the semantic primitives.
    ["CommitHeatmap.tsx", heatmapSrc],
    ["OutlineRow.tsx", outlineRowSrc],
    ["TaskDueBadge.tsx", dueBadgeSrc],
  ];

  it.each(files)("%s references no raw hex, no --color-*, and no emoji", (_file, source) => {
    expect(source).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(source).not.toMatch(/--color-/);
    expect(source).not.toMatch(/\p{Extended_Pictographic}/u);
  });
});
