# OmniProj R0 Product Reset Contract

## Outcome

R0 exists to reduce project re-entry cost. It is complete only when a researcher can open
OmniProj, choose a project, recover the relevant context, and own one executable next action
without reconstructing the project from raw history.

Engineering completeness, feature presence, and passing interaction tests are necessary but do
not establish this outcome.

## Primary loop

```text
Projects
  -> see which projects need a decision
  -> open one project
  -> recover direction and observed change around one current next step
  -> keep, revise, or complete that step; move the project to wait/park when needed
```

The loop ends inside OmniProj. Opening a repository, terminal, editor, Finder, or another app is
not part of the product contract and must not become the primary call to action.

The default path must not require reading a full commit history, editing project metadata,
configuring an Agent, or maintaining manual task-to-commit links.

## Default surfaces

### Projects Index

Each row answers only:

1. What project is this?
2. What is the current Human commitment?
3. Why does it need review now?
4. What factual change occurred since the commitment was set?

The default order follows deterministic review reasons. Repository silence is supporting
evidence, not a priority or value proxy. Activity strips are not shown by default.

Search belongs to this surface rather than permanent application chrome. The default view
separates projects needing a decision from the rest; lifecycle filters and alternative sorting
are progressively disclosed.

### Project Re-entry

The first screen has one visual endpoint: the current next step. It contains:

- the current commitment and only its currently relevant lifecycle actions;
- objective and desired outcome as compact framing, not editable form chrome;
- a compact factual delta since that commitment;
- the primary review reason or blocker;
- explicit commitment dispositions when relevant: keep, revise, or complete;
- progressively disclosed project controls for less-frequent lifecycle moves such as wait and park.

Zero-value state such as “no review required” is omitted. Planning, full activity, decisions,
and project settings are secondary disclosures on the same page, not equal-weight tabs. The
product-validation recorder is instrumentation and is never a primary user feature.

## Canonical work model

`WorkItem` is the only task object. `Project.current_next_action_id` points to one WorkItem.
There is no separate UI task model that must be reconciled with Current Commitment.

Legacy `notes/next.md` tasks are imported once into WorkItems while the original file remains
untouched. New task mutations write only the canonical project state. A WorkItem referenced by
commitment history is append-only and cannot be silently deleted or have its lifecycle status
rewritten outside commitment transitions.

## Agent boundary

Advance is contextual recovery, not generic task generation. It must start from a visible blocker
or unclear WorkItem and use project framing plus recent factual change. Proposals start with zero
selected items, remain derivative, and require explicit Human adoption.

Provider credentials and consent are global settings and never appear in the default project
re-entry path.

## Scope disposition

Keep:

- local-first, human-readable state;
- source-repository read-only boundary;
- stable project/source identity and relink;
- repository observation cache;
- atomic writes, revision checks, audit history, and Undo;
- credential-store and remote-consent boundaries;
- accessibility and CI foundations.

Move out of the default path:

- full commit timeline and topology graph;
- complete planning collection and decision ledger;
- reminder and Agent configuration;
- project framing and lifecycle forms;
- dogfood recorder UI.

## Acceptance

Before expanding scope again, dogfood at least five real projects and twenty re-entry events.
Evaluate:

- portfolio selection time;
- re-entry resolution time and failure rate;
- stale commitment correction rate;
- commitment follow-through;
- weekly OmniProj maintenance cost;
- voluntary opening before notifications.

If these fail, improve the Index, re-entry view, commitment semantics, or maintenance cost. Do not
add more Agent, visualization, notification, or planning surface to compensate.

## R1 amendment (2026-09-02)

The product owner ruled that four project-management capabilities ship **before** the dogfood
gate starts: overdue-work review reasons, task tags, board/time task views, and a read-only
cross-project focus strip. Rationale: the gate requires real daily use, and the owner's daily
use is multi-project planning, scheduling, and tracking — these capabilities are a precondition
of the gate, not compensation for its failure. Design and boundaries:
`docs/superpowers/specs/2026-09-02-r1-project-management.md`.

Everything else in this contract stands unchanged: one visual endpoint per project page,
progressive disclosure, no new top-level navigation, zero-value states omitted, proposals at
zero selected, and the acceptance evaluation above — which now begins after R1 lands and gates
R2 (FR-V2/V3, cross-project editing, further visualization).

## R2 amendment — desktop idiom (2026-09-02)

Two clauses of this contract are **withdrawn**: "removes the permanent project sidebar" and
the accordion form of progressive disclosure. Reviewing the built app in a real window showed
they had produced a responsive web page inside a native window — content locked to a 760px
centered column in an 1100px window, project switching as a full page transition, and the task
list (the surface daily use lives in) costing a click on every entry because the disclosure
defaulted closed and reset on navigation.

The reasoning that produced those clauses was sound about *density of meaning* and wrong about
*idiom*. The object the user manipulates is a **collection** of parallel projects, not a
document, so the collection stays on screen:

- a permanent, searchable, keyboard-navigable project rail (master) beside the detail pane;
- the detail pane fills the window it was given;
- workspace sections are a segmented control that remembers the chosen pane, not accordions;
- every form control carries a visible affordance — the control style was previously scoped to
  three containers, so fields elsewhere rendered as bare text.

What the original clauses were protecting still holds and is unchanged: the project page has
one visual endpoint (the current next step), the rail is navigation only and carries no
portfolio reasoning, zero-value states stay omitted, and no new top-level destination was
added. Search moved *into* the rail rather than being duplicated.
