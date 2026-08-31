# OmniProj 产品与 UI/UX 重构设计

> Status: historical design proposal; not the shipped-feature contract
>
> Date: 2026-08-10
>
> Current behavior and supported scope are documented in `README.md`. In particular, the shipped R0 uses full-page project Overview navigation rather than the proposed Peek interaction.
>
> Scope: product requirements, domain model, information architecture, UI system, UX interaction, delivery phases, and dogfood validation
> Supersedes UI and sequencing decisions in `docs/desktop-design.md` where this document is more specific. Product principles in `docs/omniproj-charter.md` remain authoritative until separately amended.

## 1. Executive decision

OmniProj should not become a general project manager, a Git dashboard, or an Agent chat client. Its narrow product contract is:

> **Human-authored intent + machine-observed actual + quarantined Agent proposals，在本地可审计地对账。**

Its first proven loop must be:

```text
Projects Index
    → re-enter one project
    → understand the current commitment and observed actual
    → confirm, complete, or replace one explicit next action
    → work in the real repository and tools
    → observed activity flows back
```

Long-term navigation and domain boundaries should be designed now, but features must earn visible navigation through real dogfood. The first redesign release therefore exposes only `Projects` as the primary work destination. `Attention`, global proposal review, full search, notifications, and Agent sessions arrive only after their prerequisite data and lifecycle are reliable.

The governing principle is:

> **让事实自动浮现，让 Agent 扩展思考，让 Human 保持判断。**

## 2. Problem and product reality

### 2.1 Product thesis

LLMs reduce the cost of generating code, drafts, summaries, searches, and candidate plans. They do not proportionally reduce the cost of:

- deciding whether a problem is worth solving;
- preserving a coherent understanding across several long-running projects;
- separating visible activity from meaningful progress;
- remembering why a path was chosen or rejected;
- detecting drift from the original purpose;
- accepting responsibility for an AI-assisted conclusion.

The product thesis is that, as execution becomes cheaper, attention, context reconstruction, judgment, and closure become more prominent limiting resources. OmniProj is designed to preserve control over that faster system; longitudinal dogfood must test whether this framing produces a repeated pull workflow.

### 2.2 Evidence, inference, and hypotheses

Declared target segment and evidence already available in the repository:

- The declared target user manages at least three research or development projects and uses LLMs heavily.
- Existing storage distinguishes Human and Agent artifacts, but the transient, default-selected proposal UI does not yet enforce that boundary end to end.
- The current home screen does not display the PRD-required next action or blocker.
- The current project detail flattens task editing, three Agent actions, Git graph, commit attribution, and decisions into one page.
- The current proposal review is transient React state and defaults all generated candidates to selected.

Product inferences:

- The strongest initial wedge is project re-entry and next-action commitment, not full project planning.
- Dense rows are more suitable than cards for comparing 5–30 projects.
- Accurate, low-maintenance state matters more than configurable schema.
- Agent usefulness depends more on a trustworthy proposal boundary than on the number of Agent modes.

Hypotheses that require dogfood rather than assumption:

- Users will voluntarily open OmniProj without notifications.
- One explicit next action per project reduces re-entry time without oversimplifying research work.
- Git facts plus a small number of Human project events are sufficient to reconstruct context.
- Review-order sorting is helpful and is not mistaken for priority.
- Attention signals can achieve acceptable precision.
- Agent proposals improve real follow-through rather than merely increasing planning output.

## 3. Product positioning

OmniProj is a local-first, single-user, agency-preserving project re-entry and advancement environment for researchers and independent developers.

It differs from adjacent categories:

- It is not Linear or GitHub Projects: no assignees, sprints, team roadmaps, or resource management.
- It is not Notion or Obsidian: no user-defined database schema, general PKM, or template marketplace.
- It is not Cursor, Codex, or Claude Code: it does not primarily execute work or manage Agent runs.
- It is not ActivityWatch or WakaTime: activity is evidence for re-entry, not productivity measurement.
- It is not a Git client: graph inspection and reconciliation are secondary project views.

The intended durable value, to be validated longitudinally, is not a graph or a prompt. It is a durable, auditable record in which `Intent`, `Observed`, `Proposed`, and `Adopted` never silently overwrite one another.

## 4. Design philosophy

### 4.1 Project is the stable object

Project—not date, reminder, activity stream, or Agent conversation—is the stable unit of navigation. `Today` may later be a view, but it is not the product model.

### 4.2 Projects Index is an operating index, not a dashboard

The home screen answers:

1. Which project is this?
2. What phase is it in?
3. What is the explicit current commitment?
4. What was the latest observed actual?
5. Is there a deterministic reason to review it?

It does not display vanity metrics, health scores, AI rankings, or decorative graphs.

### 4.3 Eliminate administrative friction; retain epistemic friction

Automate:

- reading commits, branches, timestamps, and working-tree facts;
- recovering recent context;
- surfacing discrepancies and missing state;
- preserving provenance and history.

Require the Human to decide:

- the project objective and desired outcome;
- what the explicit next action is;
- why an action is replaced;
- whether an Agent proposal is adopted;
- whether a project is waiting, parked, or archived.

### 4.4 Intent and actual remain distinguishable

Git is valuable evidence but not the whole of research progress. `Actual` consists of:

- machine-observed repository activity; and
- explicit meaningful project events recorded by the Human.

The interface must preserve their source labels. It must never merge them into a single activity or progress score.

### 4.5 Agent is contextual, not ambient

Agent capabilities appear from a project, work item, or decision. There is no permanent global chat box. Short interactions may use an inspector; complex research or refinement uses a routed full-page session.

### 4.6 Navigation must be earned

An ability becomes a primary page only when it represents a repeated, durable user task. Empty future destinations must not appear in the sidebar.

### 4.7 High information density is not high badge density

Density comes from alignment, typography, stable fields, and progressive disclosure. Color locates exceptions; text states facts; structure carries the visualization.

## 5. Non-goals

OmniProj will not:

- become a full task manager with custom fields, assignees, points, cycles, or calendar planning;
- infer project value, health, or priority from Git activity;
- require complete task-to-commit traceability;
- make a Human maintain data already known from Git;
- treat Agent output or proposal adoption as final progress;
- add a single-user Slack-style discussion system;
- expose multiple Agent modes as persistent row-level buttons;
- allow Agent output to write Human ground truth without explicit adoption;
- autonomously edit source repositories, run experiments, commit, or publish;
- use activity streaks, lines changed, or commit count as productivity measures.

## 6. Domain model

### 6.1 Project

A Project is a logical research or development effort, not a repository path.

R0 fields:

```text
ProjectId
name
status: setup | active | waiting | parked | archived
status_reason?: text
status_changed_at
objective?: text
desired_outcome?: text
phase?: text
current_next_action_id?: WorkItemId
review_at?: Timestamp
created_at
updated_at
```

Rules:

- `ProjectId` is stable if a repository moves.
- A newly registered project enters `setup`. It becomes `active` after the Human provides an objective, a desired outcome, and the first current commitment.
- `phase` is an optional Human-authored label. It never affects automatic review order.
- Collections are deferred until a Collection model and repeated grouping need are validated.
- `waiting` requires an external dependency or condition and a `review_at` date.
- `parked` means the Human intentionally removed the project from active work; it requires a reason and may have a review date.
- `archived` means completed or intentionally ended and is hidden from the default Index.
- An active project may have zero or one `current_next_action_id`; zero generates `Needs commitment`.

### 6.2 ProjectSource

Project and source are separate now even though the first UI supports one primary Git repository.

```text
ProjectSourceId
project_id
kind: git_repo | session | document_path
location
is_primary
status: available | moved | unreadable | missing
created_at
last_observed_at
```

R0 exposes only one primary `git_repo`. Multi-source UI is deferred until real projects demonstrate the need.

### 6.3 WorkItem and current commitment

The UI term `Current commitment` refers to the WorkItem referenced by `Project.current_next_action_id`. This avoids duplicating the next action as a string.

```text
WorkItemId
project_id
text
status: planned | doing | blocked | done | abandoned
blocker?: text
blocked_at?: Timestamp
created_at
updated_at
adopted_from_proposal_id?: ProposalId
```

Commitment transitions are immutable:

```text
CommitmentTransitionId
project_id
type: set | confirmed | completed | replaced | cleared | correction
previous_work_item_id?: WorkItemId
next_work_item_id?: WorkItemId
reason?: text
occurred_at
corrects_transition_id?: CommitmentTransitionId
```

Rules:

- A project may eventually contain multiple `doing` items, but exactly one WorkItem may be the explicit project-level next action.
- The current commitment cannot be silently overwritten.
- `Set` establishes the commitment clock. `Confirm` records that the same commitment is still valid without changing its original set time.
- `Complete` closes the WorkItem and immediately exposes the missing-commitment state; the user may close Peek without setting a replacement.
- `Replace` creates or selects a different WorkItem, preserves the previous item and reason, and changes the pointer atomically.
- `Clear` removes the pointer without deleting the WorkItem.
- Undo creates a compensating `correction` event; append-only history is never deleted.
- Commitment age and review age are derived from transition events, never from `WorkItem.updated_at`.
- R0 exposes only the current commitment and recent transition history. The full Work collection appears in R1.

### 6.4 Decision

Decision is distinct from plan and work status.

```text
DecisionId
project_id
question
status: open | decided | superseded
options[]
rationale
outcome
evidence_refs[]
supersedes_decision_id?: DecisionId
created_at
decided_at?: Timestamp
```

“Decided not to pursue” is a decided outcome, not an abandoned Decision. Decision UI begins in R1.

### 6.5 ActivityEvent

Activity events are immutable and source-labelled.

```text
ActivityEventId
project_id
type: git_commit | git_branch | working_tree | human_progress |
      work_transition | decision | proposal_disposition
source_ref
summary
occurred_at
observed_at
```

Repository facts and Human progress events remain visually distinguishable.

### 6.6 AttentionSignal and AttentionItem

Signals and triage state are separate:

```text
AttentionSignal
  AttentionSignalId
  type
  project_id
  subject_ref
  fingerprint: project_id + type + subject_ref + rule_version
  evidence_refs[]
  first_observed_at
  last_observed_at
  rule_version

AttentionItem
  signal_id
  state: open | snoozed | dismissed | resolved
  snoozed_until?
  disposition_reason?
  updated_at
```

Recomputing a Signal must not erase the Human's snooze or dismissal.

### 6.7 AgentSession and Proposal

Agent sessions have explicit scope and may create durable proposals.

```text
AgentSessionId
project_id
target_ref
mode
objective
context_manifest
provider
model
state
created_at
updated_at

ProposalId
session_id
target_ref
reasoning_summary
source_refs[]
state: draft | awaiting_review | partially_adopted |
       adopted | rejected | superseded | failed
created_at
reviewed_at?

ProposalItemId
proposal_id
payload
state: awaiting_review | adopted | rejected

AdoptionReceiptId
proposal_id
proposal_item_ids[]
created_ground_truth_refs[]
updated_ground_truth_refs[]
before_hashes[]
after_hashes[]
created_at
```

Adoption creates or changes a Human ground-truth object and records a receipt. A Proposal never changes type into a WorkItem or Decision. Undo is a receipt-based controlled inverse operation and must not overwrite later Human edits.

## 7. Long-term information architecture

```text
AppShell
├── Projects
│   ├── Dense Projects Index
│   └── Project Space
│       ├── Overview
│       ├── Work
│       ├── Decisions
│       ├── Activity
│       └── Sessions
├── Attention                 # appears only in R2
├── Review / Proposals        # appears only after volume gate in R3
├── Command palette / Search  # global action, not default L1 page
└── Settings                  # low-frequency utility
```

Stable routes:

```text
/projects
/projects/:projectId/overview
/projects/:projectId/work
/projects/:projectId/work/:workItemId
/projects/:projectId/decisions
/projects/:projectId/decisions/:decisionId
/projects/:projectId/activity
/projects/:projectId/sessions
/projects/:projectId/sessions/:sessionId
/attention
/proposals
/proposals/:proposalId
/search?q=
/settings
```

`/projects/:projectId/overview` is the canonical project URL. `/projects/:projectId` permanently redirects to it. Navigation from the Index carries a background location and renders the canonical URL as Peek; direct access or refresh renders the same content as a full page. `Open as page` clears the background location rather than navigating to a second object URL.

Routes may be reserved in the registry before their navigation items are visible, but unshipped capabilities must not register an empty page or visible destination.

## 8. Page depth and interaction surfaces

### 8.1 L1: cross-project pages

- `Projects`: portfolio scan and project selection.
- `Attention`: durable cross-project signal triage, added in R2.
- `Review`: cross-project proposal triage, added only if contextual review becomes insufficient.

### 8.2 L2: Project Space

- `Overview`: intent, current commitment, recent actual, blocker, recent rationale.
- `Work`: full first-class work collection after the current-commitment loop is proven.
- `Decisions`: append-only decision history and supersession.
- `Activity`: observed timeline and optional graph/reconciliation modes.
- `Sessions`: collection of complex Agent sessions.

### 8.3 L3: routed object detail

- Work-item detail
- Decision detail
- Proposal detail
- Agent session

### 8.4 Surface rules

| Surface | Use | Must not contain |
|---|---|---|
| Inline | Filtering, row selection, reversible low-risk state changes | Long text, destructive actions, Agent review |
| Peek / Inspector | Quick review or medium-depth editing while preserving list context | Full project model, long research, cross-object reconciliation |
| Modal | Add project, destructive confirmation, bounded adoption confirmation | Chat, browsing, long-form editing |
| Full page | Sustained work, long content, history, deep links, multi-object context | Trivial field edits |

Rules:

- Peek uses the canonical URL and a background location.
- Directly opening the same URL renders a full-page fallback.
- Closing Peek restores selection, keyboard focus, filters, sort, and scroll.
- Browser-style Back/Forward must work.
- No essential action is hover-only.

## 9. R0 UI specification

### 9.1 AppShell

R0 primary navigation contains only `Projects`. Collections and Settings remain hidden until they have specified, working behavior; theme follows the operating system in R0.

### 9.2 Dense Projects Index

Target: display 9–11 projects at 1280×800 without scrolling.

Default row height: 64–70px. Columns:

1. `Project`: name, optional phase, and branch as secondary facts.
2. `Current commitment`: explicit next action and commitment age.
3. `Observed actual`: last commit time, short SHA, subject or change delta.
4. `Review`: the highest-priority deterministic reason and optional `+N`.

The default row does not show:

- full repository path;
- complete task list;
- complete commit list;
- Git graph;
- decision rationale;
- proposal content;
- Agent controls;
- green `Healthy` or `Current` badges.

Review order is deterministic and labelled as review order, never AI priority:

1. source unavailable or read failure;
2. setup incomplete;
3. missing current commitment;
4. commitment needs review;
5. project review date reached for waiting or parked state;
6. remaining projects by transparent selected sort.

`Actual changed` is an informational comparison shown in Peek; it does not place a project in `Needs review` or affect R0 sort by itself.

#### R0 derived ReviewReason rules

| Reason | Predicate | Evidence shown | Resolution/reset | Suppression |
|---|---|---|---|---|
| `Source unavailable` | primary source is missing, unreadable, or Git refresh failed | source status, last successful refresh, error category | relink source or successful refresh | never suppressed; suppresses inactivity inference |
| `Complete setup` | project is `setup` and objective, desired outcome, or first commitment is missing | missing framing fields | complete framing and promote to active | suppressed only by source failure |
| `Needs commitment` | project is `active` and `current_next_action_id` is empty | last commitment transition | set a current commitment or change project status | suppressed for setup, waiting, parked, archived |
| `Review action` | project is `active` and the latest `set`/`confirmed` transition is older than the visible global R0 review interval | review interval, set/confirmed time | confirm, complete, replace, clear, or change status | suppressed for waiting, parked, archived and source failure |
| `Scheduled review` | waiting/parked project has `review_at <= now` | status reason and review date | set a new review date or change status | suppressed for archived and source failure |

R0 defaults the visible commitment review interval to seven days for dogfood and records the rule version with each derived reason. The default is a learning parameter, not a claim about ideal cadence.

### 9.3 Project Peek

Recommended width: 480–560px. Below the responsive threshold it becomes a full page.

Order:

1. Project identity and lifecycle state.
2. Expanded review reasons, when present.
3. Current commitment with `Complete` and `Replace`.
4. Observed actual definition list.
5. Recent commitment history rail.
6. `Open as page` for focused or direct access.

Creating or replacing a commitment requires an explicit save button. Blur must never be the sole persistence mechanism.

### 9.4 Add Project modal

Flow:

1. Select directory.
2. Validate Git repository and read permission.
3. Detect duplicate source registration.
4. Preview project name and path.
5. Register the Project in `setup` state.
6. Open Project Peek and focus framing in this order: objective → desired outcome → first current commitment.
7. Promote the Project to `active` when the three framing fields are complete.

It must distinguish non-Git directory, bare repository, duplicate registration, unreadable directory, and moved repository.

### 9.5 R0 keyboard flow

- `Tab` / `Shift+Tab`: standard control navigation.
- `Enter`: open the focused project.
- `Esc`: close Peek and restore focus.
- `⌘F`: focus the local project filter; platform-equivalent mappings are documented.
- `⌘N`: open Add Project.
- `⌘R`: refresh repository facts without replacing system-level refresh behavior outside the app window.

R0 uses a semantic list of project links and does not implement custom arrow-key navigation. Peek is a non-modal inspector: focus enters its heading or first available action, the background remains navigable, and `Esc` closes it and restores row focus. Add Project is modal, traps focus, supports `Esc`, and restores focus on close. Save, refresh, error, and Undo outcomes use an `aria-live` status region.

### 9.6 Responsive behavior

- `≥1100px`: four Index columns and Peek inspector.
- `800–1099px`: Observed actual collapses to relative time and defined delta; commit subject and SHA move to Peek.
- `<800px`: Project and commitment form the primary stacked content; Review remains visible; project detail renders as a full page.
- Horizontal page scrolling is forbidden.
- `64–70px` is a default `min-height`, not a fixed row height. Text scaling and localization may increase row height naturally.
- At 200% text scaling, no status, action, or recovery control may be clipped or overlap another element.

## 10. Semantic visual system

Do not implement a generic arbitrary-color `Badge`. Use constrained components.

### 10.1 Component taxonomy

| Component | Meaning | Examples |
|---|---|---|
| `ProjectStateTag` | Human-declared lifecycle exception | Waiting, Parked, Archived |
| `ReviewSignalBadge` | Deterministic reason requiring review | Needs commitment, Review action, Actual changed, Repo unavailable |
| `CommitmentStateTag` | Commitment lifecycle in Peek/history | Active, Completed, Replaced |
| `FactLabel` | Neutral observed fact without container | branch, SHA, time, changed files |
| `ProvenanceTag` | Source and authority boundary | Imported, Agent proposal, Adopted |
| `ActivityStamp` | Event verb and timestamp | Completed · Aug 8 |
| `FilterChip` | Interactive filtering | All, Needs review, Parked |

### 10.2 Badge budget

Per Index row:

- at most one Project state tag;
- at most one primary Review signal badge;
- at most three plain fact labels;
- at most two enclosed badges total.

Multiple review reasons use the fixed priority above, followed by a neutral `+N`. Peek expands the reasons as text rows.

`+N` is plain, uncontained count text or part of the primary badge's accessible name; it is not a third badge. Its accessible name enumerates the hidden reasons.

### 10.3 Semantic token contract

Components consume semantic tokens only. Palette hex values and status, interactive, provenance, or data-series colors must never be reused across semantic roles.

```text
--op-bg-{canvas|surface|subtle|raised}
--op-text-{primary|secondary|tertiary|inverse}
--op-border-{subtle|strong}
--op-interactive-{fg|hover|pressed|disabled}
--op-focus-ring
--op-status-{neutral|info|success|warning|danger}-{fg|bg|border|icon}
--op-provenance-agent-{fg|bg|border}
--op-data-series-{1..6}
```

Every token has Light, Dark, high-contrast, and forced-colors behavior. Component implementations must not reference raw palette values.

Core theme mapping:

| Token | Light | Dark |
|---|---|---|
| `--op-bg-canvas` | `#F5F7FA` | `#0B0E13` |
| `--op-bg-surface` | `#FFFFFF` | `#121720` |
| `--op-bg-subtle` | `#EEF2F6` | `#18202B` |
| `--op-bg-raised` | `#FFFFFF` | `#1D2632` |
| `--op-text-primary` | `#17202A` | `#F1F5F9` |
| `--op-text-secondary` | `#4B5B6B` | `#ABB8C6` |
| `--op-text-tertiary` | `#657789` | `#8795A5` |
| `--op-border-subtle` | `#D8E0E8` | `#2B3745` |
| `--op-border-strong` | `#8493A3` | `#627286` |
| `--op-interactive-fg` | `#006F8B` | `#58D3F2` |
| `--op-focus-ring` | `#006F8B` | `#58D3F2` |

Status foreground/background pairs:

| Status | Light | Dark |
|---|---|---|
| `info` | `#075985` / `#E0F2FE` | `#7DD3FC` / `#0C2B3A` |
| `success` | `#166534` / `#DCFCE7` | `#86EFAC` / `#102D20` |
| `warning` | `#7A4300` / `#FFF4CC` | `#FFD166` / `#342505` |
| `danger` | `#9F1239` / `#FFE4E6` | `#FDA4AF` / `#35151D` |
| `neutral` | `#475569` / `#F1F5F9` | `#CBD5E1` / `#202833` |

Borders for status pairs must be derived and individually contrast-tested; they are not created by reducing component opacity.

### 10.4 Semantic color rules

- Neutral slate: waiting, parked, archived.
- Amber: review is required; not a project quality judgment.
- Blue/info: observed change or informational discrepancy.
- Red/danger: read failure, unavailable source, destructive action, or confirmed deadline failure.
- Teal/green: only confirmed completion or successful persistence.
- Violet: reserved for unadopted Agent derivative provenance. Adopted ground truth uses neutral provenance text such as `Adopted from proposal P-014` and retains a link to the receipt; adoption never erases origin.

Color is always redundant with visible text; shape or icon is a secondary aid. No animation, glow, pulse, or emoji conveys status.

### 10.5 Typography, interaction states, and control minimums

- Necessary text: at least 12px / 16px line height.
- Body: 13px / 18px.
- Project/action emphasis: 14–15px.
- Nonessential micro label: minimum 11px / 14px.
- Regular controls: minimum 28px high.
- Icon-only controls: minimum 32×32px with accessible name.
- Small text contrast: at least 4.5:1.
- Focus and control boundaries: at least 3:1.
- Placeholder text must also meet 4.5:1 when it communicates format or expected input.
- Disabled state uses explicit semantic tokens, not whole-component opacity.
- Hover, pressed, selected, and focus must remain visually distinct.
- Tooltip supplements information only; it appears on hover and keyboard focus, closes with `Esc`, and never contains the sole action explanation.
- Default transitions are limited to color and border changes of 100–150ms. `prefers-reduced-motion` disables nonessential animation and smooth scrolling.

### 10.6 Permitted micro-visualizations

- Commitment history event rail.
- Review reason aggregation (`primary + N`).
- Natural-language `N repository commits observed since this commitment was set` delta. This does not claim that the commits advanced or completed the commitment.
- Source-labelled event timeline in deeper Project views.

Conditionally permitted after comprehension testing:

- a discrete 14-day activity tick strip only when at least two active dates exist and users do not misread it as progress. It uses fixed binary day cells, names the event source, uses no status color or independent normalization, and has an adjacent text summary.

Forbidden:

- health score, priority score, Agent confidence percentage;
- completion percentage inferred from Git;
- traffic-light project health;
- activity streaks, cumulative commit curves, velocity;
- independently normalized sparklines used for cross-project comparison;
- gauges, donuts, radar charts, progress rings;
- Git graph in an Index row;
- additions/deletions as productivity;
- per-project decorative colors.

## 11. Human–Agent interaction design

Agent functionality enters only after R0–R2 prerequisites.

### 11.1 Entry

One labelled `Advance` action replaces row-level `✨`, `💬`, and `📋` controls.

The system first asks what is blocking progress, then routes to:

- Clarify: expose missing definitions and unknowns.
- Challenge: pressure-test assumptions and failure modes.
- Make executable: find the smallest verifiable action.
- Refine: produce a structured specification.
- Research: gather evidence with traceable sources.

### 11.2 Depth

- Inline: start Advance and show durable status.
- Inspector: short clarification or compact proposal review.
- Full page: long research, specification, or multi-round Agent session.
- Modal: bounded adoption confirmation only; never chat.

### 11.3 Proposal review

Required properties:

- durable ID and lifecycle across navigation and restart;
- source project/object/session;
- context manifest, provider, model, time, and source references;
- default selected items: zero;
- edit before adoption;
- partial adoption;
- exact ground-truth diff;
- atomic batch write;
- adoption receipt and Undo;
- visible failure without partial silent mutation.

Only a durable `awaiting_review` artifact may enter a future global Review page. Raw chat, clarification transcripts, logs, and generation traces remain contextual.

## 12. Attention and notifications

R0 uses a `Needs review` Projects filter, not an Inbox. R2 may introduce Attention only after structured reasons and triage state exist.

Initial Signal types:

- missing current commitment;
- commitment beyond review rule;
- repository quiet beyond a visible threshold;
- source unavailable or read failure;
- durable proposal awaiting review.

R1 may add explicit-blocker signals after `blocked_at` is reliably maintained. R2 may add decision-blocking signals only after `Decision.blocking_target_ref` exists. Signal rules must not infer either condition from generic update timestamps.

Every Attention item must show:

- why it appeared;
- source evidence;
- the applicable rule and threshold;
- available actions;
- snooze/dismiss/resolve state.

Notifications are opt-in after in-app signal precision is measured. No notification repeats without a meaningful state change.

## 13. Privacy, trust, and failure boundaries

- Source repositories remain strictly read-only.
- All canonical product state remains local and human-readable.
- Remote model use must show what context leaves the machine, which provider receives it, and where the result is stored.
- Provider failure cannot mutate ground truth.
- Repository failure cannot be represented as inactivity.
- A moved path must support relinking without changing Project identity.
- Store writes are atomic; partial writes are surfaced and recoverable.
- Proposal adoption is atomic and auditable.
- Error messages use user language and recovery actions, not raw stack traces.

## 14. Error and empty states

R0 states have explicit preservation and recovery behavior:

| State | Preserve | Primary recovery |
|---|---|---|
| No registered projects | local store and onboarding context | `Add project` |
| No current commitment | Project identity and observed facts | `Commit next action` |
| Repository has no commits | Project and Source identity | explain empty Git history; continue framing |
| Repository moved or missing | ProjectId, source history, commitment | `Locate repository` |
| Unreadable repository | ProjectId and cached facts with timestamp | `Retry` or choose readable source |
| Duplicate registration | existing Project; do not create another | `Open existing project` |
| Detached HEAD | all project state and exact Git fact | explain state; no forced correction |
| Git refresh failure | last successful cached facts and timestamp | `Retry` |
| Refresh in progress | current visible facts | stable loading state; no layout shift |
| Store or commitment write failure | unsaved Human draft | `Retry` and `Copy text` |
| Invalid route | navigation history | `Back to Projects` |

These states must not collapse into one generic `inactive` or `not a Git repo` condition. Repository failure must suppress inactivity conclusions until a successful observation resumes.

## 15. Delivery phases and gates

### R0: trusted pull-based re-entry

Ship:

- AppShell, stable routes, deep links;
- Projects as the only primary destination;
- Dense Projects Index;
- URL-backed Project Peek/full-page fallback;
- Project and Source identity separation;
- setup framing: objective, desired outcome, and first commitment;
- current commitment lifecycle and recent history;
- last commit, subject, branch, and working-tree facts;
- UI project registration and source recovery;
- local persistence and Undo.

Do not ship:

- Agent, notifications, Attention Inbox, global Review;
- full Work collection, full Decision ledger, Git graph;
- manual task–commit attribution;
- sparkline or progress visualization.

Gate: at least two to four weeks, at least five real projects, and at least twenty re-entry events:

- Index-to-project selection median ≤60 seconds;
- re-entry median ≤3 minutes and P90 ≤5 minutes;
- Re-entry Resolution Rate ≥60%;
- Commitment Follow-through Rate ≥50%;
- total portfolio maintenance ≤10 minutes per week;
- stale current-commitment rate <20%, measured by the Human reporting at re-entry that the displayed commitment is no longer valid;
- voluntary opening at least three days per week for two consecutive weeks.

If R0 fails, improve Index, Peek, commitment semantics, or maintenance cost. Do not add Agent or push features to compensate.

### R1: project operations

Add only after R0 passes:

- Work collection and Work-item detail;
- blocker and minimal append-only work log;
- Decisions and supersession;
- unified Activity timeline;
- meaningful Human non-Git progress event;
- command palette/quick switcher;
- optional activity reconciliation candidates.

Gate: across at least twenty re-entry events, a prior Work, Decision, or Human-progress record materially avoids external context reconstruction in at least 30% of events, with reuse across at least three projects. R0 re-entry, maintenance, and stale-state metrics must not worsen by more than 10%. Reconciliation candidates are derived and dismissible; no reconciliation action is required to clear review state.

### R2: trustworthy attention loop

Add:

- durable Signal and AttentionItem lifecycle;
- first validate Attention through the Projects `Needs review` surface;
- expose Attention as a primary page only after at least three observed sessions require triage across at least two projects;
- open, snooze, dismiss, resolve;
- opt-in digest and notification only after the first twenty in-app items meet the precision gate.

Gate over at least twenty real items:

- useful precision ≥75%;
- false-positive rate from non-Git work, waiting, or parked state <20%;
- unchanged repeat notifications = 0;
- at least 30% lead within 72 hours to an explicit project disposition or a Human-confirmed outcome;
- R0 metrics do not degrade by more than 10%.

### R3: durable Human–Agent loop

Add:

- contextual `Advance`;
- durable AgentSession and Proposal;
- review, edit, partial adopt, reject, provenance, diff, receipt, and Undo;
- long-session full page;
- global Review only if pending-volume gate is satisfied.

Gate over at least fifteen real proposal uses:

- accepted or edited-then-accepted rate ≥30%;
- adopted proposal 72-hour meaningful-progress rate is not lower than Human-authored next actions;
- proposal consumption is no more than three times the number adopted;
- pending proposals older than seven days <20%;
- unadopted ground-truth writes = 0;
- default selected proposal items = 0;
- overwrite, partial batch adoption, or cross-project attribution failures = 0.

Global Review appears only after at least two projects repeatedly accumulate pending proposals and at least three observed workflows require cross-project triage.

## 16. Success metrics

Primary metrics:

> **Re-entry Resolution Rate:** after opening a project not viewed in OmniProj for at least 24 hours, the Human confirms or revises one current commitment, or explicitly changes the project to waiting, parked, or archived, within two minutes.

> **Commitment Follow-through Rate:** among confirmed or revised commitments, the percentage followed within 72 hours by a Human-confirmed meaningful outcome.

Meaningful progress includes a Human-confirmed outcome, such as completing the commitment, recording a substantive non-Git outcome in R1, or confirming that observed repository activity relates to the commitment. Repository activity alone is evidence, not follow-through. Waiting, parking, and abandonment are deliberate dispositions counted in Re-entry Resolution, not meaningful progress.

Opening the app, editing metadata, generating Agent output, or merely adopting a proposal never counts as progress. Maintenance time excludes initial onboarding and migration.

Supporting metrics:

- portfolio decision time;
- voluntary pull frequency before notifications;
- starting-point share;
- current-commitment coverage;
- weekly maintenance tax;
- stale-state rate;
- Attention precision and false-positive rate;
- proposal usefulness and follow-through;
- Human-control integrity violations.

These are internal dogfood learning gates. Passing them authorizes the next delivery phase; it does not establish external demand or generalizability. External validation requires repeated use by additional target users.

## 17. Current implementation disposition

Keep:

- Tauri desktop form;
- local-first storage;
- source repository read-only constraint;
- Markdown + Git auditability;
- Human/Agent author boundary;
- existing underlying capture and Git-reading capabilities where correct.

Rebuild:

- application navigation and routing;
- project identity and source model;
- Projects Index view model;
- next-action persistence and lifecycle;
- proposal durability and adoption boundary;
- staleness semantics and unified thresholds;
- semantic design tokens and accessible components.

Temporarily remove from default surfaces while preserving compatible data:

- branch-aware GitGraph;
- full Decisions component;
- Clarify and Refine inline panels;
- task-row emoji Agent actions;
- per-commit task dropdown;
- settings copy that exposes internal charter terminology.

Future placement:

- GitGraph → Project Activity graph/reconciliation mode.
- Decisions → Project Overview summary + Project Decisions page.
- Clarify/Refine → unified contextual Advance flow.
- Agent proposal review → durable Inspector or full routed Proposal detail.

## 18. Validation and testing

### 18.1 Product validation

- Five-second scan test over 8–12 projects: identify current commitment, last actual, and review reason with ≥90% correctness.
- Provenance test: distinguish Human, Observed, and Agent information with ≥95% correctness.
- Neutrality test: activity visualization must not cause users to infer health or importance without evidence.
- Pure-text versus micro-visual A/B: retain a visual only if it lowers time or error rate.
- Real-log replay using 10–20 project snapshots.

### 18.2 Interaction testing

- Keyboard-only Index → Peek → commitment update → return.
- Back/Forward, direct deep link, app restart, and restored selection/filter/scroll.
- Complete, Replace, Undo, save failure, and concurrent refresh.
- Add Project validation and moved-source recovery.
- 200% text scaling, narrow window full-page fallback, and long localized text.

### 18.3 Accessibility and visual QA

Every visual component change must pass these release gates:

- small text contrast ≥4.5:1;
- interactive boundary/focus contrast ≥3:1;
- Light/Dark × normal/high-contrast snapshots;
- keyboard-only and VoiceOver completion of the R0 core path;
- automated accessibility scan with no critical or serious findings;
- 200% scaling, long localized text, maximum badge count, and `+N` overflow fixtures;
- forced-colors mode using system colors without losing boundaries;
- protanopia, deuteranopia, tritanopia, and grayscale review with every state still identifiable when color is removed;
- `prefers-reduced-motion` with no loss of meaning;
- no hover-only essential action;
- no 9–10px essential UI text.

### 18.4 Integrity testing

- source repositories receive zero writes;
- unadopted Agent content never enters Human ground truth;
- commitment transition history is append-only and recoverable;
- Signal recomputation preserves Human triage state;
- proposal batch adoption is atomic;
- error states never masquerade as inactivity;
- every displayed metric has a defined source, unit, time window, and update point.

## 19. External product patterns used as precedent

These official references demonstrate deployed patterns, not causal proof that they will work for OmniProj:

- [Slack Activity](https://slack.com/help/articles/19693583638803-Get-your-work-done-from-the-Activity-view) and [Slack Later](https://slack.com/help/articles/360042650274-Save-messages-and-files-for-later): actionable attention queues, filtering, completion, and snooze.
- [Linear Project Overview](https://linear.app/docs/project-overview), [Peek](https://linear.app/docs/peek), [Inbox](https://linear.app/docs/inbox), and [Triage](https://linear.app/docs/triage): separate scale levels, dense scan, contextual detail, and explicit accept/decline.
- [GitHub Projects automation](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-built-in-automations) and [coding agents](https://docs.github.com/en/copilot/concepts/agents/about-third-party-coding-agents): automate high-confidence facts and preserve review boundaries.
- [Claude Code permission modes](https://code.claude.com/docs/en/permission-modes): plan/proposal before action and explicit execution authority.
- [Obsidian local storage](https://obsidian.md/help/Files%2Band%2Bfolders/How%2BObsidian%2Bstores%2Bdata) and [Bases](https://obsidian.md/help/bases): inspectable local state with views separated from canonical content.

OmniProj-specific hypotheses remain subject to the phase gates above.

## 20. Final product contract

OmniProj succeeds when it enables a user to:

1. scan a portfolio without interpreting a dashboard;
2. re-enter a project without reconstructing history manually;
3. make one explicit, owned next-action commitment;
4. compare that intent with factual observed activity;
5. use Agent assistance without surrendering authorship or judgment;
6. spend less time maintaining OmniProj than the context it saves.

If the product creates more tasks, graphs, reminders, summaries, or proposals without increasing clarity, ownership, and meaningful follow-through, it has failed regardless of feature completeness.
