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
  -> see current commitment + why review + what changed
  -> open one project
  -> confirm, revise, or complete the commitment; move the project to wait/park when needed
  -> continue work in the real repository or research tool
```

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

### Project Re-entry

The first screen contains:

- objective and desired outcome as framing, not editable form chrome;
- the current commitment and its lifecycle actions;
- a compact factual delta since that commitment;
- the primary review reason or blocker;
- explicit commitment dispositions: confirm, revise, complete, or clear;
- a direct Project settings path for less-frequent lifecycle moves such as wait and park.

Planning, full activity, decisions, and settings are secondary views. The product-validation
recorder is instrumentation and is never a primary user feature.

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
