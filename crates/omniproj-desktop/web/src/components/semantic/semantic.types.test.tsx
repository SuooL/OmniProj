// Compile-time contract: the constrained components reject arbitrary tones, raw colors, and
// unknown status values. These `@ts-expect-error` lines only pass `tsc -b` (npm run build) if
// the following usage is genuinely a type error — Vitest transpilation alone does not typecheck.

import { expect, it } from "vitest";

import { CommitmentStateTag } from "./CommitmentStateTag";
import { FilterChip } from "./FilterChip";
import { ProjectStateTag } from "./ProjectStateTag";
import { ReviewSignalBadge } from "./ReviewSignalBadge";

// Exported so noUnusedLocals does not flag it; never rendered.
export function _rejectedUsages() {
  return (
    <>
      {/* @ts-expect-error unknown project status */}
      <ProjectStateTag status="frozen" />
      {/* @ts-expect-error tone is not a public prop */}
      <ProjectStateTag status="active" tone="danger" />
      {/* @ts-expect-error raw color is not a public prop */}
      <ProjectStateTag status="active" color="#ff0000" />
      {/* @ts-expect-error unknown review reason code */}
      <ReviewSignalBadge reason={{ code: "bogus", label: "x" }} />
      {/* @ts-expect-error unknown commitment status */}
      <CommitmentStateTag status="paused" />
      {/* @ts-expect-error tone is not a public prop */}
      <FilterChip label="All" pressed={false} onClick={() => {}} tone="info" />
    </>
  );
}

it("type constraints are enforced at build time", () => {
  expect(typeof _rejectedUsages).toBe("function");
});
