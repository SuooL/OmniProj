# OmniProj

**Re-enter any project in under 5 minutes.** OmniProj passively captures your work — git activity plus Claude Code / Codex sessions — and distills it into a small set of state files (`briefing.md`, `decisions.md`, `open.md`) so future-you (or your agent) knows exactly where things stand.

> The difference from "yet another AI memory tool": concrete citations are code-checked, not trusted on prompt obedience. Capture produces a deterministic `FactSheet` (real commits, real repo paths); the LLM is grounded on it, and a post-distill **verify gate** flags commit hashes and repo-path tokens the FactSheet can't vouch for. Trust-critical tokens are enforced by code, not by prompt wishes.

---

- **Platform:** macOS & Linux; the Tauri desktop app (in progress) targets macOS first.
- **Privacy:** persistent state stays **local** in `~/.omniproj`. Distillation sends a **scrubbed** digest to your configured LLM provider — sensitive paths (`.env*`, `*.key`, `*.pem`, `id_rsa*`, `secrets/`, …) are dropped and common secret shapes (`sk-…`, `AKIA…`, `Bearer …`) are masked, both on by default. Run `omniproj digest` to preview the **exact** outbound text before it leaves the machine, or point `default_model` at a local **Ollama** model so nothing leaves at all. → [Privacy boundary](#privacy-boundary) for the full contract.

---

## Demo

*Re-entry in seconds: register a project, then `omniproj recall` prints the stored briefing / open questions / decisions with **no LLM call**:*

```console
$ omniproj add ~/code/photon-tracer
[omniproj] registered photon-tracer [f4cab7831a56170f] -> ~/code/photon-tracer

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
> The distill/verify pipeline above ships as library code in `omniproj-distill`; the CLI
> commands that drove it end-to-end (`briefing`/`refresh`/`correct`/`curate`/`reconcile`)
> were removed in the desktop pivot and will return as desktop actions.

Your files are ground truth (charter §5 原则4). AI-written state lives in `auto/`, your own notes in `notes/` — physically separate, and **the AI never silently overwrites your edits**. `omniproj recall` surfaces your `notes/` alongside any stored AI state.

All state lives in `~/.omniproj` — plain markdown, versioned by its own git repo (every write is an independent, revertable commit). Nothing is ever written into your project.

The store records its on-disk layout in `~/.omniproj/SCHEMA_VERSION`. Upgrades are forward-compat-strict: a newer OmniProj migrates an older store forward (stepwise, each migration a revertable commit), but an **older** binary refuses to touch a store written by a newer one rather than silently downgrading it. A pre-versioning `~/.omniproj` is adopted as v1 non-destructively (nothing is converted).

## Quickstart

```sh
# 1. Install (build from source; binaries on the Releases page)
cargo install --git https://github.com/SuooL/OmniProj omniproj-cli

# 2. Configure a model (any OpenAI-compatible provider works)
omniproj init                          # writes ~/.omniproj/config.toml
export DEEPSEEK_API_KEY=sk-...      # or ANTHROPIC_API_KEY, OPENAI_API_KEY, ...

# 3. Register a project and start a next-action list
omniproj add ~/code/my-project
omniproj note add "wire the new sampler into render::integrate"
omniproj recall                        # print stored context + your notes (no LLM)
```

> Automatic background refresh (the daemon) and the one-shot `briefing` distill were removed
> in the desktop pivot; keeping projects fresh moves into the Tauri app. The CLI today is for
> registration, your `notes/` next-actions, `clarify`, `search`, `recall`, and `digest`.

### Privacy boundary

Persistent state stays local in `~/.omniproj`, but distillation sends the captured digest (git/session-derived text) to your configured LLM provider unless you select a local endpoint such as Ollama. Before sending, OmniProj scrubs the **outbound digest**: sensitive paths (`.env*`, `*.key`, `*.pem`, `id_rsa*`, `secrets/`, `credentials*`, …) are dropped and common secret shapes (`sk-…`, `AKIA…`, `Bearer …`, `KEY=value`) are masked — both on by default. Run `omniproj digest` to see the **exact** text that would leave the machine before it does. For a nothing-leaves-the-machine setup, point `default_model` at a local Ollama model. The deny-list is configurable (`[privacy] deny_globs`) and masking can be turned off per run with `--no-redact` (the deny-list still applies). Treat captured sessions as potentially sensitive.

### Agent integration (hooks)

`omniproj recall` is the no-LLM instant recall built for SessionStart/SessionEnd hooks — drop it into your agent's hook config to print the stored context + your `notes/` at session start. (The MCP server (`omniproj mcp`) was removed in the desktop pivot; the desktop app will own richer agent integration.)

## Commands

| Command | What it does |
|---|---|
| `omniproj add / list / remove` | register projects (state lives in `~/.omniproj`, never your repo) |
| `omniproj note [add\|done\|rm]` / `omniproj next` | your next-action list per project (`notes/next.md`, user ground truth) / cross-project overview |
| `omniproj clarify <id>` | one bounded round of adversarial questioning on a not-yet-clear item (标记+理由, never a recommendation) |
| `omniproj recall` | print the stored re-entry context + your `notes/` (no LLM, instant — built for hooks) |
| `omniproj search <query>` | full-text search across captured sessions (FTS5, local, CJK-aware) |
| `omniproj digest` | inspect the raw captured substrate (no LLM) |
| `omniproj stats` | per-project state-file sizes + store history |
| `omniproj providers` / `omniproj init` | provider catalog / starter config |

> **Pivoting to a desktop app.** OmniProj is moving from this CLI to a Tauri desktop
> "project advancer" (see [`docs/desktop-design.md`](docs/desktop-design.md)). The background
> daemon, the axum web dashboard, and the briefing/opinion/curate/eval distillation surface
> were removed in that teardown; the CLI now keeps project registration plus the capture-side
> and notes utilities the desktop has not yet subsumed. The higher-level pitch above still
> describes the pre-pivot product and will be rewritten as the desktop app lands.

## Architecture

Rust workspace, hexagonal core. One binary (`omniproj`) today; a Tauri desktop shell (`omniproj-desktop`) is being built out.

```
omniproj-core      domain types, ~/.omniproj layout, FactSheet (no async/net/llm)
omniproj-capture   git + Claude Code/Codex parsers → unified Session
omniproj-distill   the only crate that links an LLM (provider adapters + verify gate + clarify)
omniproj-index     disposable FTS5 index over raw normalized sessions
omniproj-cli       clap CLI (the `omniproj` binary)
omniproj-desktop   Tauri desktop shell (WIP — the pivot target)
```

## Platform support

macOS and Linux; the Tauri desktop app targets macOS first. See the [banner](#omniproj) at the top for the privacy boundary summary.

## Status

**In transition.** The shipped CLI product (capture → distill → verify, background daemon, dashboard, opinion, MCP) is being pivoted to a Tauri desktop "project advancer" (see [`docs/desktop-design.md`](docs/desktop-design.md)). The daemon, axum dashboard, and distillation/opinion/eval CLI surface have been removed; `omniproj-core` / `omniproj-capture` / `omniproj-distill` (provider + verify + clarify) and the FTS5 index are retained and being rebuilt behind the desktop app. See [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE)
