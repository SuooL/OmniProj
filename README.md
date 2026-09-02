# OmniProj

**Re-enter any project in under five minutes.** OmniProj is a local-first, single-user
desktop environment for researchers and independent developers. It holds your **human-authored
intent** next to the **machine-observed actual** state of each repository, and lets you reconcile
them — confirm, complete, replace, or clear one explicit next action — without leaving your real
tools.

> The core contract: *human-authored intent + machine-observed actual, reconciled locally and
> audibly, with the human keeping judgment.* OmniProj never writes to your source repositories; it
> only reads them, and it records its own state in a local store you control.

The current desktop loop (MVP foundation):

```text
Projects Index  →  re-enter one project  →  see the current commitment and the observed actual
                →  confirm / complete / replace / clear one action  →  work in your real repo
                →  observed activity flows back on the next refresh
```

The current development desktop includes a Human planning task list (`notes/next.md`) whose items
can be explicitly promoted to the single project-level Current Commitment, a read-only Git commit
timeline with task attribution, selective Advance proposal adoption with retained provenance, an
append-only decision log (`plan.md`), deduplicated configurable reminders, and a local re-entry
timer for the dogfood gate. The Index shows a neutral sixteen-week commit activity strip and is
ordered by factual silence; the menu-bar icon carries the current non-zero attention count. Advance
has an in-app provider/model setup, explicit remote-transmission consent, and stores API keys only
in the operating-system credential store. The source repository remains read-only throughout.

---

## Platform & privacy

- **Platform:** the Tauri desktop app targets **macOS** as the R0 acceptance platform (Linux is
  used for CI). Rust + React inside a native webview.
- **Local by default:** all persistent state lives under `~/.omniproj` (override with the
  `OMNIPROJ_HOME` environment variable). Advance sends only the selected task text and problem
  note, and only after explicit consent when a remote provider is selected. API keys stay in the
  operating-system credential store and are never written to `~/.omniproj`.
- **Source repositories are read-only.** OmniProj runs read-only Git commands against your repos
  and writes only to its own store. A move/rename never corrupts a project — you relink it.

## Local file layout

Each project gets a stable, permanent `ProjectId` and its own directory:

```text
~/.omniproj/
  meta.toml                        # (schema v2) registry — not per project
  projects/<ProjectId>/
    meta.toml                      # this project's registry record + ProjectSource envelope
    notes/project.md               # your single human-state document: TOML front matter +
                                   #   a byte-preserved Markdown body. OmniProj never rewrites
                                   #   your prose; it only edits the front matter atomically.
    notes/next.md                  # Human planning tasks; content-revision protected
    plan.md                        # Human plan/decision log with optional commit anchors
    auto/advance/                  # Agent proposals; Human adoption retains provenance
    cache/r0-observation.json      # last successful repository observation (derived, regenerable)
    learned.md                     # legacy pre-R0 document — preserved untouched
  dogfood/reentry-events.jsonl     # local append-only product-validation events
```

- **`ProjectId` is permanent.** Relinking a moved repository changes only
  `ProjectSource.location`, never the identity — every cache and index entry stays keyed by
  `ProjectId`, so history and search survive a move.
- Replaceable Human documents are written **atomically**, audited with the exact paths touched,
  and protected by an expected revision. Append-only dogfood events are serialized under the
  store lock. Commitment mutations additionally append to an immutable transition history.

## Migration & recovery

- A pre-R0 (schema v1) store migrates **idempotently** to schema v2: running it twice produces an
  identical tree, and it **never changes your legacy human/agent documents** (`notes/`, `plan.md`,
  `auto/`, `learned.md` are byte-preserved).
- If a source repository is **missing, moved, unreadable, non-Git, bare, or has an unborn/detached
  HEAD**, OmniProj keeps showing the **last successful observation** with its timestamp and offers
  a **Relink** action — it never claims "no activity" when it simply could not read the source.

## Using it

### Canonical routes

R0 uses real path-based routes (never hash routes):

| Route | What it shows |
|---|---|
| `/` | redirects to `/projects` |
| `/projects` | the dense operating **Index** (one row per project) |
| `/projects/:projectId` | redirects to that project's canonical Overview |
| `/projects/:projectId/overview` | the full-page **Project Overview** |

On restart, OmniProj returns you to the last canonical URL; an explicit deep link always wins.

### Keyboard shortcuts

| Key | Action |
|---|---|
| `Cmd/Ctrl + F` | focus the local project filter |
| `Cmd/Ctrl + N` | open **Add Project** |
| `Cmd/Ctrl + R` | pull-refresh (re-observe sources); prevents the browser reload only while the OmniProj window is focused |
| `Enter` | open the focused project |
| `Esc` | close the Add Project modal, or close the sidebar drawer on a narrow window |
| `Tab` / `Shift+Tab` | standard control navigation |

### The Index, activity, and the seven-day review rule

The default Index order is a factual **attention order**: operating projects with readable Git
observations are ordered by whole silent days, most silent first. A repository with no commits is
shown explicitly; an unavailable source is marked unknown rather than assigned fabricated
inactivity. Waiting/parked/archived projects remain available through filters. This is explicitly
not a priority or health ranking, and transparent opt-in sorts (name, recent commit) remain.

Every observed row carries exactly sixteen UTC calendar-week commit counts, oldest to newest,
rendered as a compact activity strip with an accessible text summary. The silence text states the
current reminder threshold beside the observed fact.

The default operating view omits archived projects, but the **Archived** filter and sidebar
section keep them discoverable so they can be opened and restored. OmniProj renders the persisted
last-successful observation immediately, then refreshes repositories in the background at startup;
completed rows update progressively and failures retain the last known facts.

Beside the order label the Index shows **`Commitment review interval: 7 days`**, read from the
backend `review_policy` (never a hard-coded frontend constant). A commitment with no confirmed
activity within that window surfaces a *Review action* signal. All color is redundant with visible
text, and there is no arbitrary-color badge.

### Editing state

Every human change is an **explicit Save** — a blur never persists anything. Setup completes
atomically (objective + desired outcome + first commitment in one write). Commitment actions (set,
confirm, complete, replace, clear) and **Undo** append to history. If a save conflicts with a newer
revision, OmniProj refetches and keeps your draft; if a write's audit commit fails after the state
is durable, it reloads the saved state and never re-sends your change.

## Build, run, and test

```bash
# Rust workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked

# Frontend (crates/omniproj-desktop/web)
npm ci
npm run build          # tsc + vite
npm test               # Vitest unit tests
npx playwright install chromium
npm run test:e2e       # Playwright: core loop, responsive, accessibility

# Desktop app (from crates/omniproj-desktop)
cargo tauri dev        # run the app in development
cargo tauri build      # production bundle
```

CI runs the frontend unit/build job, a separate Playwright e2e job, and the Rust workspace job on
every PR to `dev`/`main`.

## Dogfood gate

Passing the engineering gates only **begins** dogfood — it does not declare success. R1 (Agent
capabilities, deeper surfaces) stays blocked until the re-entry loop earns it in real use:

- **2–4 weeks** of daily use,
- across **at least five real projects**,
- producing **at least twenty re-entry events**, with the agreed re-entry metrics recorded.

Use the Overview's **Re-entry timer**: start it when opening a project and finish when the next
action is clear enough to begin real work. Events are stored locally in
`~/.omniproj/dogfood/reentry-events.jsonl`; the UI reports event count, distinct projects, and
median re-entry time. See [`docs/dogfood.md`](docs/dogfood.md) for the interpretation rules.

These are **product-learning thresholds to force honest evaluation — not scientific universals.**
Navigation and features are earned by a repeated, durable workflow, not added speculatively.

---

*A trimmed command-line interface (`omniproj`) also exists for registering and inspecting projects;
the desktop app is the R0 product.*
