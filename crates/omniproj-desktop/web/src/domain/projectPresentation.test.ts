import { describe, expect, it } from "vitest";

import {
  indexItem,
  observedActual,
  reviewReason,
} from "../test/fixtures";
import {
  applyReviewFilter,
  excludeArchived,
  filterByText,
  formatRelativeTime,
  hiddenReasons,
  hiddenReasonsAccessibleName,
  primaryReason,
  REVIEW_ORDER_LABEL,
} from "./projectPresentation";

describe("filterByText", () => {
  it("matches case-insensitively and leaves the input untouched", () => {
    const items = [
      indexItem({ name: "Cancer Imaging" }),
      indexItem({ name: "OmniProj" }),
    ];
    const frozen = JSON.stringify(items);

    const result = filterByText(items, "omni");

    expect(result.map((item) => item.name)).toEqual(["OmniProj"]);
    expect(JSON.stringify(items)).toBe(frozen); // immutable
  });

  it("returns everything for an empty/whitespace query", () => {
    const items = [indexItem({ name: "A" }), indexItem({ name: "B" })];
    expect(filterByText(items, "   ")).toHaveLength(2);
  });
});

describe("applyReviewFilter", () => {
  const items = [
    indexItem({ name: "Needs", status: "active", review_reasons: [reviewReason("needs_commitment")] }),
    indexItem({ name: "Clean", status: "active", review_reasons: [] }),
    indexItem({ name: "Waiting", status: "waiting", review_reasons: [] }),
    indexItem({ name: "Parked", status: "parked", review_reasons: [] }),
  ];

  it("all returns a copy of everything", () => {
    const result = applyReviewFilter(items, "all");
    expect(result).toHaveLength(4);
    expect(result).not.toBe(items);
  });

  it("needs_review keeps only rows with a review reason", () => {
    expect(applyReviewFilter(items, "needs_review").map((i) => i.name)).toEqual(["Needs"]);
  });

  it("waiting and parked filter by status", () => {
    expect(applyReviewFilter(items, "waiting").map((i) => i.name)).toEqual(["Waiting"]);
    expect(applyReviewFilter(items, "parked").map((i) => i.name)).toEqual(["Parked"]);
  });
});

describe("excludeArchived", () => {
  it("drops archived rows", () => {
    const items = [
      indexItem({ name: "Live", status: "active" }),
      indexItem({ name: "Gone", status: "archived" }),
    ];
    expect(excludeArchived(items).map((i) => i.name)).toEqual(["Live"]);
  });
});

describe("review reason display", () => {
  it("shows the backend-priority primary reason and enumerates the rest for a11y", () => {
    const item = indexItem({
      review_reasons: [
        reviewReason("source_unavailable"),
        reviewReason("needs_commitment"),
        reviewReason("scheduled_review"),
      ],
    });

    expect(primaryReason(item)?.code).toBe("source_unavailable");
    expect(hiddenReasons(item).map((r) => r.code)).toEqual([
      "needs_commitment",
      "scheduled_review",
    ]);
    expect(hiddenReasonsAccessibleName(item)).toBe(
      "2 more review reasons: Needs commitment, Scheduled review",
    );
  });

  it("has no hidden reasons for a single-reason row", () => {
    const item = indexItem({ review_reasons: [reviewReason("review_action")] });
    expect(hiddenReasonsAccessibleName(item)).toBeNull();
  });

  it("exposes a transparent, non-ranking order label", () => {
    expect(REVIEW_ORDER_LABEL.toLowerCase()).toContain("explicit decision");
  });
});

describe("formatRelativeTime", () => {
  const now = new Date("2026-08-12T12:00:00Z");

  it("formats relative text but keeps the exact timestamp as the title", () => {
    const result = formatRelativeTime("2026-08-09T12:00:00Z", now);
    expect(result).toEqual({ text: "3 days ago", title: "2026-08-09T12:00:00Z" });
  });

  it("returns null for an unparseable instant", () => {
    expect(formatRelativeTime("not-a-time", now)).toBeNull();
  });
});

describe("neutrality: commit counts are never a priority/health input", () => {
  it("ignores commits_since_commitment in reason display and filtering", () => {
    const reasons = [reviewReason("review_action")];
    const few = indexItem({
      review_reasons: reasons,
      observed_actual: observedActual({ commits_since_commitment: 0 }),
    });
    const many = indexItem({
      review_reasons: reasons,
      observed_actual: observedActual({ commits_since_commitment: 999 }),
    });

    // Same reasons -> identical primary reason regardless of activity.
    expect(primaryReason(few)?.code).toBe(primaryReason(many)?.code);
    // Both are equally "needs_review"; activity does not change membership or order.
    expect(applyReviewFilter([few, many], "needs_review")).toHaveLength(2);
  });
});
