# OmniProj

**Re-enter any project in under 5 minutes.** OmniProj passively captures your work — git activity plus Claude Code / Codex sessions — and distills it into a small set of state files (`briefing.md`, `decisions.md`, `open.md`) so future-you (or your agent) knows exactly where things stand.

> The difference from "yet another AI memory tool": concrete citations are code-checked, not trusted on prompt obedience. Capture produces a deterministic `FactSheet` (real commits, real repo paths); the LLM is grounded on it, and a post-distill **verify gate** flags commit hashes and repo-path tokens the FactSheet can't vouch for. Trust-critical tokens are enforced by code, not by prompt wishes.

---

- **Platform:** macOS & Linux. **Windows is not supported yet** (the daemon uses Unix domain sockets + file locks).
- **Privacy:** persistent state stays **local** in `~/.omniproj`. Distillation sends a **scrubbed** digest to your configured LLM provider — sensitive paths (`.env*`, `*.key`, `*.pem`, `id_rsa*`, `secrets/`, …) are dropped and common secret shapes (`sk-…`, `AKIA…`, `Bearer …`) are masked, both on by default. Run `omniproj digest` to preview the **exact** outbound text before it leaves the machine, or point `default_model` at a local **Ollama** model so nothing leaves at all. → [Privacy boundary](#privacy-boundary) for the full contract.

---

## Demo

*Re-entry in seconds: register a project, then `omniproj recall` prints the stored briefing / open questions / decisions with **no LLM call**:*

```console
$ omniproj add ~/code/photon-tracer
[omniproj] registered photon-tracer [f4cab7831a56170f] -> ~/code/photon-tracer

$ omniproj status
daemon: running (pid 48213)
started: 2026-07-12T09:14:02Z
in-flight: — (idle)

PROJECT                WATCH   LAST DISTILL
photon-tracer          yes     distilled 2026-07-12T09:15:41Z

$ omniproj recall
# OmniProj recall — photon-tracer (last distilled: 2026-07-12T09:15:41Z)

## briefing
On `feat/adaptive-sampling` (HEAD f435e4e). Adaptive Monte-Carlo sampler now
converges ~2x faster on the caustics scene; the variance-threshold path landed
this session. Back-to-work first step: wire the new sampler into
`render::integrate` behind the `--adaptive` flag.

## open
- Denoiser still assumes fixed samples/pixel — the adaptive count must be threaded
  through `render::integrate` before it can ship.
- Caustics regression scene not yet in CI. ⚠ (test count self-reported this
  session, not verified against a live run)

## decisions
- 2026-07-12T09:15:41Z — Chose stratified over Halton sequences: better cache
  locality on the tiled backend (3f062b1).
```

## How it works

```
capture → ground → distill → verify → (learn / curate)
 (no LLM)  (no LLM)  (the ONE     (no LLM,
                      LLM call)    deterministic)
```

- **Capture** — git log/status/diff + Claude Code & Codex transcripts for the project directory. Passive; never writes to your repo.
- **Ground** — a `FactSheet` of verified git facts (branch, HEAD, commit hashes, repo paths) extracted deterministically; refresh gating separately tracks a dirty-worktree fingerprint.
- **Distill** — one LLM call turns the substrate into `briefing` / `decisions` / `open`. Provider-neutral: Anthropic, OpenAI, DeepSeek, Groq, OpenRouter, Gemini, Ollama, any OpenAI-compatible endpoint.
- **Verify** — commit hashes and repo-relative file paths in the output are checked against FactSheet whitelists; unverified ones get flagged `⚠`, never silently passed.
- **Learn / Curate** — your corrections become per-project heuristics (`omniproj correct`); append-only files get consolidated (`omniproj curate`).

Your files are ground truth (charter §5 原则4). AI-written state lives in `auto/`, your own notes in `notes/` — physically separate, and **the AI never silently overwrites your edits**. If you hand-edit an `auto/` file, the next distill preserves your version and parks its own in `auto/<file>.md.incoming`; run `omniproj reconcile` to see the diff and pick a side (`--keep-mine` / `--take-ai`). `omniproj recall` surfaces your `notes/` alongside the AI state.

All state lives in `~/.omniproj` — plain markdown, versioned by its own git repo (every distill is an independent, revertable commit). Nothing is ever written into your project.

The store records its on-disk layout in `~/.omniproj/SCHEMA_VERSION`. Upgrades are forward-compat-strict: a newer OmniProj migrates an older store forward (stepwise, each migration a revertable commit), but an **older** binary refuses to touch a store written by a newer one rather than silently downgrading it. A pre-versioning `~/.omniproj` is adopted as v1 non-destructively (nothing is converted).

## Quickstart

```sh
# 1. Install (build from source; binaries on the Releases page)
cargo install --git https://github.com/SuooL/OmniProj omniproj-cli

# 2. Configure a model (any OpenAI-compatible provider works)
omniproj init                          # writes ~/.omniproj/config.toml
export DEEPSEEK_API_KEY=sk-...      # or ANTHROPIC_API_KEY, OPENAI_API_KEY, ...

# 3. Register a project and get your first briefing
omniproj add ~/code/my-project
omniproj briefing ~/code/my-project --model deepseek/deepseek-chat

# 4. Let the daemon keep it fresh automatically
omniproj status                        # lazy-starts the background daemon
```

The daemon watches registered worktrees **and your agent session transcripts** and re-distills **only when something actually changed** (a deterministic change fingerprint gates every LLM call), with a 24h floor as the staleness ceiling.

### Persistent daemon

`omniproj status` lazy-starts the daemon on demand, but for always-on tracking install it as a real OS service so it **starts at login and restarts on crash**:

```sh
omniproj install-service     # macOS: launchd LaunchAgent · Linux: systemd user unit
omniproj uninstall-service   # stop + remove it
```

- **macOS** writes `~/Library/LaunchAgents/com.omniproj.daemon.plist` (`RunAtLoad` + `KeepAlive`) and loads it via `launchctl`. Check it with `launchctl list | grep omniproj`.
- **Linux** writes `~/.config/systemd/user/omniproj.service` (`Restart=always`) and runs `systemctl --user enable --now omniproj.service`. Check it with `systemctl --user status omniproj`.
- Other platforms are unsupported — run `omniproj daemon` manually.

The daemon logs to `~/.omniproj/daemon.log`. It's also crash-hardened internally: a panic in one project's distillation is caught and logged, and the worker keeps serving every other project rather than taking the whole daemon down.

### Privacy boundary

Persistent state stays local in `~/.omniproj`, but distillation sends the captured digest (git/session-derived text) to your configured LLM provider unless you select a local endpoint such as Ollama. Before sending, OmniProj scrubs the **outbound digest**: sensitive paths (`.env*`, `*.key`, `*.pem`, `id_rsa*`, `secrets/`, `credentials*`, …) are dropped and common secret shapes (`sk-…`, `AKIA…`, `Bearer …`, `KEY=value`) are masked — both on by default. Run `omniproj digest` to see the **exact** text that would leave the machine before it does. For a nothing-leaves-the-machine setup, point `default_model` at a local Ollama model. The deny-list is configurable (`[privacy] deny_globs`) and masking can be turned off per run with `--no-redact` (the deny-list still applies). Treat captured sessions as potentially sensitive.

### Agent integration (MCP + hooks)

Let your agent recall the project state automatically at session start:

```sh
claude mcp add omniproj -- omniproj mcp     # tool: project_recall · resources: briefing/decisions/open/…
```

`omniproj recall` is the no-LLM instant recall built for SessionStart/SessionEnd hooks — pair it with the `claude mcp add` line above in your `.mcp.json` / hook config.

## Commands

| Command | What it does |
|---|---|
| `omniproj add / list / remove` | register projects (state lives in `~/.omniproj`, never your repo) |
| `omniproj briefing [--depth deep]` | distill now and print the briefing |
| `omniproj refresh [--all]` | re-distill only if the substrate changed; silent otherwise |
| `omniproj daemon` / `omniproj status` | background auto-refresh; status lazy-starts the daemon |
| `omniproj install-service` / `uninstall-service` | run the daemon as a persistent OS service (launchd/systemd) that starts at login + restarts on crash |
| `omniproj dashboard` | local web cockpit (127.0.0.1) with explicit second-opinion generation |
| `omniproj opinion [--ignore dims]` | counter-convergent second opinion that challenges the briefing |
| `omniproj model` | your editable user-model profile (presentation lens) |
| `omniproj correct -m "..."` | teach OmniProj from a correction |
| `omniproj reconcile [--keep-mine\|--take-ai]` | show/resolve conflicts when you hand-edit an `auto/` file (never silently overwritten) |
| `omniproj curate` | consolidate append-only state files |
| `omniproj recall` | print the stored re-entry context + your `notes/` (no LLM, instant — built for hooks) |
| `omniproj search <query>` | full-text search across captured sessions (FTS5, local, CJK-aware) |
| `omniproj mcp` | serve project memory over MCP (stdio) for agents |
| `omniproj digest` | inspect the raw captured substrate (no LLM) |
| `omniproj eval --gold <file>` | judge a candidate briefing against a human gold handoff |
| `omniproj providers` / `omniproj init` | provider catalog / starter config |
| `omniproj doctor` | read-only setup diagnostic: store health, config, default model + key, provider connectivity (exits non-zero on FAIL, so it works as a setup gate in CI/scripts) |

### Reasoning depth

`--depth deep` (or `default_depth = "deep"` in config) turns on the deep pipeline: older sessions outside the recency window are map-reduce compressed instead of discarded, a structured extraction pass anchors the prose, and a completeness critic revises the draft. Default stays shallow — one LLM call.

## Architecture

Rust workspace, hexagonal core. One binary.

```
omniproj-core      domain types, ~/.omniproj layout, FactSheet (no async/net/llm)
omniproj-capture   git + Claude Code/Codex parsers → unified Session
omniproj-distill   the only crate that links an LLM (+ verify gate, learn, curate)
omniproj-daemon    watcher + floor timer + gated refresh orchestration
omniproj-ipc       daemon⇄CLI over a Unix socket (typed JSON)
omniproj-api       local dashboard cockpit (axum + embedded SPA; explicit opinion POST)
omniproj-index     disposable FTS5 index over raw normalized sessions
omniproj-cli       clap CLI (the `omniproj` binary)
```

## Platform support

macOS and Linux. Windows is not supported yet (the daemon uses Unix domain sockets and file locks). See the [banner](#omniproj) at the top for the privacy boundary summary.

## Status

Early but functional (`v0.1.x`). The capture→distill→verify loop, background daemon, user model, second opinion, dashboard, raw-session search, MCP recall, and deep reasoning pipeline are implemented with unit/API smoke coverage. Real-project LLM quality still depends on dogfood evals with your provider and gold handoffs. See [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE)
