# OmniProj Dogfood Protocol

## Purpose

The dogfood gate tests the product claim—project re-entry becomes reliably actionable in under
five minutes. Passing engineering tests is necessary but is not evidence for that claim.

## Recording one event

1. Open a real project Overview and select **Start re-entry** before reconstructing context.
2. Read the Current Commitment, planning tasks, observed Git facts, and relevant rationale.
3. Select **Next action is clear; start work** only when you can immediately begin a concrete
   action in the real repository or research tool.
4. Do not stop the timer merely because the page loaded. Abandoned or interrupted attempts should
   not be recorded as successful re-entry events; note them separately during qualitative review.

Events are appended locally to `~/.omniproj/dogfood/reentry-events.jsonl`. Each row records only
the stable ProjectId, completion timestamp, and duration in seconds. No source-repository content
is copied and nothing is uploaded.

## Gate and interpretation

- Run for 2–4 weeks of ordinary use.
- Include at least five real projects and at least twenty completed re-entry events.
- Primary descriptive metric: median re-entry duration; target `< 300 seconds`.
- Also inspect the distribution and interrupted attempts. A fast median must not hide repeated
  failures on older, moved, waiting, or poorly framed projects.
- Treat these thresholds as internal product-learning criteria, not scientific generalizations.

Passing the numeric gate permits deeper product work; it does not prove general effectiveness.
