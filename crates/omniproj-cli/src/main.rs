//! omniproj — CLI front-end (spec §6.1). v1 thin loop.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod mcp;
mod service;

#[derive(Parser)]
#[command(
    name = "omniproj",
    version,
    about = "Cognitive scaffolding for LLM-era knowledge work"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the captured substrate digest for a project (no LLM — for inspecting capture).
    /// This is the EXACT text that would be sent to the LLM provider: deny-listed paths
    /// are dropped and secret shapes masked (spec §5, W1-1). Use --no-redact for raw.
    Digest {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Show the raw digest without secret masking (deny-list still applies).
        #[arg(long)]
        no_redact: bool,
    },
    /// Distill a project into briefing/decisions/open, write them, and print the briefing.
    /// Always re-distills (explicit request); use `refresh` for the change-gated path.
    Briefing {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Model as `provider/model` (e.g. openrouter/anthropic/claude-...). Overrides config.
        #[arg(long)]
        model: Option<String>,
        /// Reasoning depth: shallow (1 LLM pass, default) or deep (map-reduce +
        /// extraction + critic, several passes). Overrides config default_depth.
        #[arg(long)]
        depth: Option<String>,
        /// Disable outbound secret masking for this run (deny-list still applies).
        #[arg(long)]
        no_redact: bool,
    },
    /// Re-distill a project ONLY if its substrate changed since the last distill
    /// (the staleness floor, spec §5). Silent when up to date. The daemon runs this.
    Refresh {
        /// Project directory (default: current directory). Ignored with --all.
        path: Option<PathBuf>,
        /// Refresh every registered project.
        #[arg(long)]
        all: bool,
        /// Distill even if nothing changed (same as `briefing`, but quiet).
        #[arg(long)]
        force: bool,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
        /// Reasoning depth: shallow (default) or deep. Overrides config.
        #[arg(long)]
        depth: Option<String>,
        /// Disable outbound secret masking for this run (deny-list still applies).
        #[arg(long)]
        no_redact: bool,
    },
    /// Teach OmniProj from a correction: distills your feedback (and/or your in-place
    /// edit to the briefing) into per-project heuristics for future distillation.
    Correct {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Correction text. If omitted, uses your uncommitted edit to auto/briefing.md.
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
    },
    /// Curator GC: consolidate the append-only decisions.md (offline, no capture).
    Curate {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
    },
    /// Register a project to track (writes to ~/.omniproj only, never your repo).
    Add {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
    },
    /// List registered projects.
    List,
    /// Unregister a project (keeps your notes/; removes AI-written auto/ + cache/).
    Remove {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
    },
    /// Show the background daemon's live status (lazy-starts it if not running).
    Status {
        /// Don't auto-start the daemon; just report whether it's running.
        #[arg(long)]
        no_start: bool,
    },
    /// Run the background daemon: watch registered projects + a floor timer and
    /// auto-refresh (change-gated). Foreground; Ctrl-C / SIGTERM to stop.
    Daemon {
        /// Floor probe interval in seconds (default 86400 = 24h).
        #[arg(long)]
        interval: Option<u64>,
        /// fs-event debounce window in milliseconds (default 1500).
        #[arg(long)]
        debounce: Option<u64>,
        /// Session-transcript quiet window in seconds (default 600): a conversation
        /// triggers a refresh only after this much silence.
        #[arg(long)]
        session_quiet: Option<u64>,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
        /// Reasoning depth: shallow (default) or deep. Overrides config.
        #[arg(long)]
        depth: Option<String>,
    },
    /// Second opinion: a deliberately counter-convergent view that challenges the
    /// current briefing (标记+理由, never recommendations). Ignores chosen user-model
    /// dimensions to stay "unlike you" (spec §4.5).
    Opinion {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Comma-separated user-model dimensions to ignore (default: all of them).
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
    },
    /// Serve the local read-only web dashboard (127.0.0.1). Ctrl-C to stop.
    Dashboard {
        /// Port to listen on.
        #[arg(long, default_value_t = 7700)]
        port: u16,
    },
    /// Show your user model (~/.omniproj/user/model.md) — the profile distillation uses
    /// as a presentation lens. Edit the file directly; disable a dimension by adding
    /// `(disabled)` to its heading.
    Model {
        /// Write the starter template if no model exists yet.
        #[arg(long)]
        init: bool,
    },
    /// Full-text search across a project's captured sessions (FTS5, local, no LLM).
    Search {
        /// What to look for (matched literally; CJK works).
        query: String,
        /// Project directory (default: current directory).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Max hits.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Print the stored re-entry context (briefing + open + decisions) for a project.
    /// Read-only, no LLM — built for hooks and quick terminal recall.
    Recall {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
    },
    /// Resolve pending reconciles (charter §5 原则4): when you hand-edit an `auto/`
    /// file, the next distill parks its version in `<file>.md.incoming` instead of
    /// silently overwriting yours. With no flag this SHOWS the pending diffs (read-only,
    /// never auto-resolves). auto/ is AI territory — durable notes belong in notes/.
    Reconcile {
        /// Project directory (default: current directory).
        path: Option<PathBuf>,
        /// Discard the AI `.incoming` and keep your edits (deletes the `.incoming`).
        #[arg(long, conflicts_with = "take_ai")]
        keep_mine: bool,
        /// Replace your file with the AI `.incoming` version.
        #[arg(long)]
        take_ai: bool,
    },
    /// Your next-action list for a project (`notes/next.md`, user ground truth — AI
    /// never writes it). With no subcommand, prints the current list. This is the
    /// "把纸质笔记本搬进来" entry point (cockpit).
    Note {
        #[command(subcommand)]
        action: Option<NoteAction>,
        /// Project directory (default: current directory).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Cross-project next-action overview: every project's open items, sorted by most
    /// recent activity (a neutral fact, not a priority ranking — charter §5 原则3).
    /// No LLM.
    Next,
    /// Discuss a not-yet-clear next-action item to help you think it through (cockpit;
    /// charter §6 例外). One bounded round per invocation: the model returns 标记+理由
    /// (unstated premises, contradictions, missing criteria) — never a recommendation.
    /// The discussion is AI-written to auto/clarify/<id>.md; the CONCLUSION is yours to
    /// transcribe into your note. Uses the `[clarify]` model/effort (`omniproj init`).
    Clarify {
        /// The item's short id (the `#id` from `omniproj note`).
        id: String,
        /// A thought to add to this round (e.g. answering a question raised last time).
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Project directory (default: current directory).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Serve project memory over MCP (stdio). Register with:
    /// `claude mcp add omniproj -- omniproj mcp`
    Mcp,
    /// Health stats per project: state-file sizes + store history (健康信号 =
    /// note 数量稳定或下降, spec §10). No LLM.
    Stats,
    /// Score a stored/candidate re-entry document against a human gold handoff.
    /// Observational only: writes cache/eval-report.json, never edits state.
    Eval {
        /// Project directory (default: current directory). Used when --candidate is omitted.
        path: Option<PathBuf>,
        /// Human-written gold handoff / expected re-entry document.
        #[arg(long)]
        gold: PathBuf,
        /// Candidate document to score. If omitted, uses stored briefing/open/decisions.
        #[arg(long)]
        candidate: Option<PathBuf>,
        /// Model as `provider/model`. Overrides config.
        #[arg(long)]
        model: Option<String>,
    },
    /// List configured LLM providers (predefined + custom) and whether their key is set.
    Providers,
    /// Diagnose your setup: store health, config, the default model's provider + key,
    /// and best-effort provider connectivity. Read-only, safe to run anytime.
    Doctor,
    /// Write a starter `~/.omniproj/config.toml` (providers + default model).
    Init,
    /// Install `omniproj daemon` as a persistent OS service so it starts at login and
    /// restarts on crash (macOS launchd LaunchAgent / Linux systemd user unit).
    InstallService,
    /// Stop and remove the persistent daemon service installed by `install-service`.
    UninstallService,
}

#[derive(Subcommand)]
enum NoteAction {
    /// Append a next-action item.
    Add {
        /// The item text.
        text: String,
        /// Mark it 未成形 (thought not yet clear) — a `?` flag + hook for `omniproj clarify`.
        #[arg(long)]
        unclear: bool,
    },
    /// Mark an item done by its short id (the `#id` shown in the list).
    Done {
        /// Short id (e.g. `a3f1`).
        id: String,
    },
    /// Delete an item by its short id.
    Rm {
        /// Short id (e.g. `a3f1`).
        id: String,
    },
}

fn resolve_dir(path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(p) => Ok(p),
        None => std::env::current_dir().context("could not resolve current directory"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Digest { path, no_redact } => cmd_digest(path, no_redact),
        Cmd::Briefing {
            path,
            model,
            depth,
            no_redact,
        } => cmd_briefing(path, model, depth, no_redact).await,
        Cmd::Refresh {
            path,
            all,
            force,
            model,
            depth,
            no_redact,
        } => cmd_refresh(path, all, force, model, depth, no_redact).await,
        Cmd::Correct {
            path,
            message,
            model,
        } => cmd_correct(path, message, model).await,
        Cmd::Curate { path, model } => cmd_curate(path, model).await,
        Cmd::Status { no_start } => cmd_status(no_start).await,
        Cmd::Daemon {
            interval,
            debounce,
            session_quiet,
            model,
            depth,
        } => cmd_daemon(interval, debounce, session_quiet, model, depth).await,
        Cmd::Add { path } => cmd_add(path).await,
        Cmd::List => cmd_list(),
        Cmd::Remove { path } => cmd_remove(path).await,
        Cmd::Opinion {
            path,
            ignore,
            model,
        } => cmd_opinion(path, ignore, model).await,
        Cmd::Dashboard { port } => omniproj_api::serve(port).await,
        Cmd::Model { init } => cmd_model(init),
        Cmd::Search { query, path, limit } => cmd_search(query, path, limit),
        Cmd::Recall { path } => cmd_recall(path),
        Cmd::Reconcile {
            path,
            keep_mine,
            take_ai,
        } => cmd_reconcile(path, keep_mine, take_ai),
        Cmd::Note { action, path } => cmd_note(action, path),
        Cmd::Next => cmd_next(),
        Cmd::Clarify { id, message, path } => cmd_clarify(id, message, path).await,
        Cmd::Mcp => mcp::serve().await,
        Cmd::Stats => cmd_stats(),
        Cmd::Eval {
            path,
            gold,
            candidate,
            model,
        } => cmd_eval(path, gold, candidate, model).await,
        Cmd::Providers => cmd_providers(),
        Cmd::Doctor => cmd_doctor().await,
        Cmd::Init => cmd_init(),
        Cmd::InstallService => service::install_service(),
        Cmd::UninstallService => service::uninstall_service(),
    }
}

/// Canonicalize a project dir into (absolute path, display name).
fn resolve_repo(path: Option<PathBuf>) -> Result<(String, String)> {
    let dir = resolve_dir(path)?;
    let abs = std::fs::canonicalize(&dir)
        .unwrap_or(dir)
        .to_string_lossy()
        .to_string();
    let name = Path::new(&abs)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    Ok((abs, name))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

async fn notify_daemon_reload() {
    match omniproj_ipc::client::request(&omniproj_ipc::Request::Reload).await {
        Ok(omniproj_ipc::Response::Ack) | Err(_) => {}
        Ok(omniproj_ipc::Response::Error(e)) => {
            eprintln!("[omniproj] daemon reload failed: {e}");
        }
        Ok(_) => {}
    }
}

async fn cmd_add(path: Option<PathBuf>) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    omniproj_core::ensure_home()?;
    let meta = omniproj_core::register(&abs, &name, &now_rfc3339())?;
    omniproj_core::commit_all(&format!("register {name} ({})", meta.hash));
    notify_daemon_reload().await;
    eprintln!(
        "[omniproj] registered {} [{}] -> {}",
        meta.name, meta.hash, abs
    );
    Ok(())
}

fn cmd_list() -> Result<()> {
    let projects = omniproj_core::list_projects();
    if projects.is_empty() {
        eprintln!("[omniproj] no registered projects. add one with `omniproj add <repo>`");
        return Ok(());
    }
    println!(
        "{:<22} {:<16} {:<20} PATH",
        "NAME", "HASH", "LAST DISTILLED"
    );
    for p in projects {
        println!(
            "{:<22} {:<16} {:<20} {}",
            p.name,
            p.hash,
            p.last_distilled.as_deref().unwrap_or("—"),
            p.path
        );
    }
    Ok(())
}

async fn cmd_remove(path: Option<PathBuf>) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    let hash = omniproj_core::project_hash(&abs);
    if omniproj_core::load_meta(&hash).is_none() {
        eprintln!("[omniproj] {name} [{hash}] is not registered");
        return Ok(());
    }
    let kept_notes = omniproj_core::remove_project(&hash);
    omniproj_core::commit_all(&format!("unregister {name} ({hash})"));
    notify_daemon_reload().await;
    if kept_notes {
        eprintln!("[omniproj] unregistered {name} [{hash}] — kept your notes/ at ~/.omniproj/projects/{hash}/notes/");
    } else {
        eprintln!("[omniproj] unregistered {name} [{hash}]");
    }
    Ok(())
}

fn cmd_digest(path: Option<PathBuf>, no_redact: bool) -> Result<()> {
    let dir = resolve_dir(path)?;
    let sub = omniproj_capture::capture(&dir)?;
    // Show exactly what WOULD be sent to the provider: config deny-list + redaction,
    // with an optional --no-redact escape (spec §5, W1-1 "see it before it leaves").
    let opts = omniproj_capture::DigestOpts {
        privacy: omniproj_distill::resolve_privacy(no_redact),
        ..Default::default()
    };
    println!("{}", omniproj_capture::render_digest(&sub, &opts));
    Ok(())
}

/// One-time-per-invocation privacy notice before a distill sends the digest to a
/// remote provider (spec §5, W1-1). Silent for local providers (Ollama) and once
/// the user sets `privacy.send_consent = true`. Informational — never blocks.
fn notify_send_consent(model: Option<&str>) {
    let cfg = omniproj_distill::config::load();
    if cfg.privacy.consented() {
        return;
    }
    // Resolve which provider we'd actually use, to skip the notice for local ones.
    let effective = model
        .map(str::to_string)
        .or_else(|| std::env::var("OMNIPROJ_MODEL").ok())
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| "anthropic/claude-sonnet-4-6".to_string());
    if omniproj_distill::is_local_provider(&effective) {
        return;
    }
    eprintln!(
        "[omniproj] privacy: the distill digest (git + session text) is sent to your LLM \
provider ({}).\n\
\x20        Sensitive paths are dropped and secret shapes masked; run `omniproj digest` \
to preview the exact outbound text.\n\
\x20        For a fully local path, use Ollama (e.g. --model ollama/llama3.1). \
Silence this with `send_consent = true` under [privacy] in ~/.omniproj/config.toml.",
        effective
    );
}

/// Progress logger shared by `briefing`/`refresh` — prefixes each line like the
/// rest of the CLI. The orchestration layer (`omniproj-daemon`) stays UI-agnostic.
fn log_line(m: &str) {
    eprintln!("[omniproj] {m}");
}

async fn cmd_briefing(
    path: Option<PathBuf>,
    model: Option<String>,
    depth: Option<String>,
    no_redact: bool,
) -> Result<()> {
    use omniproj_daemon::{refresh_project, RefreshOpts, RefreshOutcome};
    let dir = resolve_dir(path)?;
    notify_send_consent(model.as_deref());
    // `briefing` is the explicit request: always distill, then print the briefing.
    let outcome = refresh_project(
        &dir,
        RefreshOpts {
            force: true,
            model: model.as_deref(),
            depth: depth.as_deref(),
            no_redact,
        },
        log_line,
    )
    .await?;
    match outcome {
        RefreshOutcome::Distilled(d) => {
            println!("{}", d.briefing);
            Ok(())
        }
        RefreshOutcome::NoSubstrate { name } => anyhow::bail!(
            "no substrate for {name} (no git repo, no matching Claude/Codex sessions)"
        ),
        // force=true can't yield UpToDate/Unregistered.
        _ => unreachable!("forced briefing distills or reports no substrate"),
    }
}

async fn cmd_refresh(
    path: Option<PathBuf>,
    all: bool,
    force: bool,
    model: Option<String>,
    depth: Option<String>,
    no_redact: bool,
) -> Result<()> {
    use omniproj_daemon::{refresh_project, RefreshOpts, RefreshOutcome};

    let dirs: Vec<PathBuf> = if all {
        let projects = omniproj_core::list_projects();
        if projects.is_empty() {
            eprintln!("[omniproj] no registered projects. add one with `omniproj add <repo>`");
            return Ok(());
        }
        projects
            .into_iter()
            .map(|p| PathBuf::from(p.path))
            .collect()
    } else {
        vec![resolve_dir(path)?]
    };

    notify_send_consent(model.as_deref());
    let mut distilled = 0usize;
    let mut failed = 0usize;
    for dir in &dirs {
        // Per-project isolation: one project's failure (bad provider key, IO error)
        // must not abort a batch `--all` sweep.
        let outcome = refresh_project(
            dir,
            RefreshOpts {
                force,
                model: model.as_deref(),
                depth: depth.as_deref(),
                no_redact,
            },
            log_line,
        )
        .await;
        match outcome {
            Ok(RefreshOutcome::Distilled(d)) => {
                distilled += 1;
                let verdict = if d.verify.is_clean() {
                    "verify clean".to_string()
                } else {
                    format!("{} flagged", d.verify.flagged.len())
                };
                eprintln!(
                    "[omniproj] ✓ distilled {} ({}, {})",
                    d.name, d.provider_label, verdict
                );
            }
            Ok(RefreshOutcome::UpToDate { name, .. }) => {
                eprintln!("[omniproj] · {name}: up to date");
            }
            Ok(RefreshOutcome::NoSubstrate { name }) => {
                eprintln!("[omniproj] · {name}: no substrate");
            }
            Ok(RefreshOutcome::Unregistered { name, .. }) => {
                eprintln!(
                    "[omniproj] · {name}: not registered — run `omniproj add` first (or `--force`)"
                );
            }
            Err(e) => {
                failed += 1;
                eprintln!("[omniproj] ✗ {}: {e:#}", dir.display());
                if !all {
                    return Err(e); // single-project run still surfaces the error
                }
            }
        }
    }
    if all {
        eprintln!(
            "[omniproj] refreshed {} project(s): {distilled} distilled, {} unchanged, {failed} failed",
            dirs.len(),
            dirs.len() - distilled - failed,
        );
    }
    Ok(())
}

async fn cmd_correct(
    path: Option<PathBuf>,
    message: Option<String>,
    model: Option<String>,
) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    let hash = omniproj_core::project_hash(&abs);

    // Correction signal = explicit -m text and/or the user's in-place edit to
    // auto/briefing.md (diff vs the last committed version). Either or both.
    let diff = omniproj_core::worktree_diff(&format!("projects/{hash}/auto/briefing.md"));
    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        parts.push(format!("用户反馈:\n{m}"));
    }
    if let Some(d) = diff.as_deref() {
        parts.push(format!("用户对 briefing 的就地修改(git diff):\n{d}"));
    }
    if parts.is_empty() {
        anyhow::bail!(
            "no correction signal for {name}. Pass -m \"<feedback>\", or edit \
             ~/.omniproj/projects/{hash}/auto/briefing.md and re-run."
        );
    }
    let signal = parts.join("\n\n");

    let resolved = omniproj_distill::resolve(model.as_deref())
        .context("could not resolve an LLM provider (see `omniproj providers`)")?;
    eprintln!(
        "[omniproj] learning from correction with {}/{} …",
        resolved.provider_name, resolved.model
    );

    let learned_path = omniproj_core::learned_path(&hash);
    let existing = std::fs::read_to_string(&learned_path).unwrap_or_default();
    let updated =
        omniproj_distill::learn_from_correction(&existing, &signal, &resolved.provider).await?;

    omniproj_core::ensure_home()?;
    if let Some(parent) = learned_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    omniproj_core::store_txn(|| -> Result<()> {
        std::fs::write(&learned_path, format!("{updated}\n"))?;
        omniproj_core::commit_all(&format!("learn {name}"));
        Ok(())
    })?;
    eprintln!("[omniproj] wrote {}", learned_path.display());
    println!("{updated}");
    Ok(())
}

async fn cmd_curate(path: Option<PathBuf>, model: Option<String>) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    let hash = omniproj_core::project_hash(&abs);
    let auto = omniproj_core::auto_dir(&hash);

    let decisions = std::fs::read_to_string(auto.join("decisions.md")).unwrap_or_default();
    let open = std::fs::read_to_string(auto.join("open.md")).unwrap_or_default();
    let learned_path = omniproj_core::learned_path(&hash);
    let learned = std::fs::read_to_string(&learned_path).unwrap_or_default();

    let needs_llm = !decisions.trim().is_empty()
        || !open.trim().is_empty()
        || omniproj_distill::learned_over_cap(&learned);
    if !needs_llm {
        eprintln!("[omniproj] {name}: nothing to curate yet");
    } else {
        let resolved = omniproj_distill::resolve(model.as_deref())
            .context("could not resolve an LLM provider (see `omniproj providers`)")?;
        eprintln!(
            "[omniproj] curating with {}/{} …",
            resolved.provider_name, resolved.model
        );

        // Each target is curated independently; results land in ONE store commit.
        let mut new_decisions = None;
        if !decisions.trim().is_empty() {
            let curated =
                omniproj_distill::curate_decisions(&decisions, &resolved.provider).await?;
            eprintln!(
                "[omniproj] decisions.md: {} -> {} lines",
                decisions.lines().count(),
                curated.lines().count()
            );
            new_decisions = Some(curated);
        }
        let mut new_open = None;
        if !open.trim().is_empty() {
            let curated = omniproj_distill::curate_open(&open, &resolved.provider).await?;
            eprintln!(
                "[omniproj] open.md: {} -> {} lines",
                open.lines().count(),
                curated.lines().count()
            );
            new_open = Some(curated);
        }
        let mut new_learned = None;
        if omniproj_distill::learned_over_cap(&learned) {
            let curated =
                omniproj_distill::consolidate_learned(&learned, &resolved.provider).await?;
            eprintln!(
                "[omniproj] learned.md over {} chars: {} -> {} chars",
                omniproj_distill::LEARNED_CAP_CHARS,
                learned.chars().count(),
                curated.chars().count()
            );
            new_learned = Some(curated);
        }

        omniproj_core::ensure_home()?;
        omniproj_core::store_txn(|| -> Result<()> {
            if let Some(d) = new_decisions {
                std::fs::write(auto.join("decisions.md"), format!("{d}\n"))?;
            }
            if let Some(o) = new_open {
                std::fs::write(auto.join("open.md"), format!("{o}\n"))?;
            }
            if let Some(l) = new_learned {
                std::fs::write(&learned_path, format!("{l}\n"))?;
            }
            omniproj_core::commit_all(&format!("curate {name}"));
            Ok(())
        })?;
    }

    // User model is USER-owned (charter §5 原则4): warn over budget, never rewrite.
    let over = omniproj_distill::user_model_over_cap(&omniproj_core::UserModel::load());
    for (dim, n) in over {
        eprintln!(
            "[omniproj] ⚠ user model dimension '{dim}' is {n} chars (budget {}) — consider trimming {}",
            omniproj_distill::USER_MODEL_DIM_CAP_CHARS,
            omniproj_core::user_model_path().display()
        );
    }
    Ok(())
}

/// Spawn `omniproj daemon` detached, logging to `~/.omniproj/daemon.log`. The child outlives
/// this CLI process (v1 backgrounding; a launchd/systemd unit is the eventual upgrade).
fn spawn_daemon() -> Result<()> {
    use std::process::{Command, Stdio};
    omniproj_core::ensure_home()?;
    let exe = std::env::current_exe().context("locate omniproj executable")?;
    let log_path = omniproj_core::omniproj_home().join("daemon.log");
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    let err = out.try_clone()?;
    let mut cmd = Command::new(exe);
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    // Detach into its own process group so the launching shell's job-control signals
    // (Ctrl-C / SIGHUP on terminal close) don't kill the daemon.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().context("spawn daemon")?;
    Ok(())
}

/// Poll the socket until the daemon answers, up to `timeout`.
async fn wait_until_up(timeout: std::time::Duration) -> bool {
    use std::time::Instant;
    let start = Instant::now();
    while start.elapsed() < timeout {
        if omniproj_ipc::client::ping().await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    false
}

async fn cmd_status(no_start: bool) -> Result<()> {
    use omniproj_ipc::client;
    if !client::ping().await {
        if no_start {
            println!("daemon: not running");
            return Ok(());
        }
        eprintln!("[omniproj] daemon not running — starting it…");
        spawn_daemon()?;
        if !wait_until_up(std::time::Duration::from_secs(5)).await {
            anyhow::bail!(
                "daemon did not come up within 5s — see {}",
                omniproj_core::omniproj_home().join("daemon.log").display()
            );
        }
    }

    match client::request(&omniproj_ipc::Request::Status).await {
        Ok(omniproj_ipc::Response::Status(s)) => {
            print_status(&s);
            Ok(())
        }
        Ok(omniproj_ipc::Response::Error(e)) => anyhow::bail!("daemon error: {e}"),
        Ok(_) => anyhow::bail!("unexpected response from daemon"),
        Err(e) => anyhow::bail!("could not query daemon: {e}"),
    }
}

fn print_status(s: &omniproj_ipc::StatusResponse) {
    println!("daemon: running (pid {})", s.pid);
    println!("started: {}", s.started_at);
    println!(
        "in-flight: {}",
        s.in_flight.as_deref().unwrap_or("— (idle)")
    );
    if s.projects.is_empty() {
        println!("\nno registered projects.");
        return;
    }
    println!();
    println!("{:<22} {:<7} LAST DISTILL", "PROJECT", "WATCH");
    for p in &s.projects {
        println!(
            "{:<22} {:<7} {}",
            p.name,
            if p.watched { "yes" } else { "no" },
            p.last_activity
        );
    }
}

async fn cmd_daemon(
    interval: Option<u64>,
    debounce: Option<u64>,
    session_quiet: Option<u64>,
    model: Option<String>,
    depth: Option<String>,
) -> Result<()> {
    use std::time::Duration;
    let mut opts = omniproj_daemon::DaemonOpts {
        model,
        depth,
        ..Default::default()
    };
    if let Some(s) = interval {
        opts.interval = Duration::from_secs(s);
    }
    if let Some(ms) = debounce {
        opts.debounce = Duration::from_millis(ms);
    }
    if let Some(s) = session_quiet {
        opts.session_quiet = Duration::from_secs(s);
    }
    omniproj_daemon::run(opts).await
}

async fn cmd_opinion(
    path: Option<PathBuf>,
    ignore: Vec<String>,
    model: Option<String>,
) -> Result<()> {
    let dir = resolve_dir(path)?;
    let out = omniproj_daemon::generate_opinion(
        &dir,
        omniproj_daemon::OpinionOpts {
            model: model.as_deref(),
            ignore,
        },
        log_line,
    )
    .await?;
    if !out.verify.flagged.is_empty() {
        eprintln!(
            "[omniproj] verify: flagged {} unverified hash(es): {}",
            out.verify.flagged.len(),
            out.verify.flagged.join(", ")
        );
    }
    if !out.verify.flagged_paths.is_empty() {
        eprintln!(
            "[omniproj] verify: flagged {} unverified path(s): {}",
            out.verify.flagged_paths.len(),
            out.verify.flagged_paths.join(", ")
        );
    }
    eprintln!("[omniproj] wrote {}", out.path.display());
    println!("{}", out.text);
    Ok(())
}

fn cmd_model(init: bool) -> Result<()> {
    let path = omniproj_core::user_model_path();
    if init {
        if path.exists() {
            eprintln!("[omniproj] user model already exists: {}", path.display());
        } else {
            omniproj_core::ensure_home()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, omniproj_core::USER_MODEL_TEMPLATE)?;
            omniproj_core::commit_all("init user model");
            eprintln!(
                "[omniproj] wrote {} — edit it to fill in your profile",
                path.display()
            );
        }
        return Ok(());
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let m = omniproj_core::UserModel::parse(&text);
            let active = m.enabled().count();
            eprintln!(
                "[omniproj] {} — {} dimension(s) active of {}",
                path.display(),
                active,
                m.dimensions.len()
            );
            println!("{text}");
        }
        Err(_) => {
            eprintln!(
                "[omniproj] no user model yet. Run `omniproj model --init` to create {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn cmd_search(query: String, path: Option<PathBuf>, limit: usize) -> Result<()> {
    let dir = resolve_dir(path)?;
    let hits = omniproj_index::search_project(&dir, &query, limit)?;
    if hits.is_empty() {
        eprintln!("[omniproj] no matches for \"{query}\"");
        return Ok(());
    }
    for h in &hits {
        let when = chrono::DateTime::<chrono::Utc>::from_timestamp(h.mtime as i64, 0)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".into());
        let snippet = h.snippet.replace('\n', " ");
        println!("[{when} {} {}] {snippet}", h.source, h.role);
    }
    eprintln!(
        "\n[omniproj] {} hit(s) — index is derived from raw sessions (spec §4.6)",
        hits.len()
    );
    Ok(())
}

fn cmd_recall(path: Option<PathBuf>) -> Result<()> {
    let dir = resolve_dir(path)?;
    let canon = std::fs::canonicalize(&dir).unwrap_or(dir);
    let Some(meta) = omniproj_core::find_by_cwd(&canon) else {
        anyhow::bail!(
            "no registered project for {} — run `omniproj add` + `omniproj briefing` first",
            canon.display()
        );
    };
    let auto = omniproj_core::auto_dir(&meta.hash);
    let mut printed = false;
    println!(
        "# OmniProj recall — {} (last distilled: {})",
        meta.name,
        meta.last_distilled.as_deref().unwrap_or("never")
    );
    for kind in ["briefing", "open", "decisions"] {
        if let Ok(text) = std::fs::read_to_string(auto.join(format!("{kind}.md"))) {
            if !text.trim().is_empty() {
                println!("\n## {kind}\n{}", text.trim());
                printed = true;
            }
        }
    }
    // Surface the user's own notes/ (charter §5 原则4): read-only, clearly labeled as
    // user-authored, appended after the AI state so re-entry shows both.
    let notes = read_notes(&meta.hash);
    if !notes.is_empty() {
        println!("\n## Your notes (notes/, user-authored — AI never edits these)");
        for (name, body) in &notes {
            println!("\n### {name}\n{}", body.trim());
        }
        printed = true;
    }
    // Nudge if a hand-edit to auto/ is awaiting reconcile (charter §5 原则4).
    if !pending_reconciles(&meta.hash).is_empty() {
        eprintln!(
            "[omniproj] ⚠ pending reconcile in auto/ — run `omniproj reconcile {}`",
            meta.name
        );
    }
    if !printed {
        eprintln!("[omniproj] no distilled state yet — run `omniproj briefing`");
    }
    Ok(())
}

/// Resolve the registered project for the given dir (or cwd), erroring with a nudge.
fn resolve_project(path: Option<PathBuf>) -> Result<omniproj_core::ProjectMeta> {
    let dir = resolve_dir(path)?;
    let canon = std::fs::canonicalize(&dir).unwrap_or(dir);
    omniproj_core::find_by_cwd(&canon).ok_or_else(|| {
        anyhow::anyhow!(
            "no registered project for {} — run `omniproj add` first",
            canon.display()
        )
    })
}

/// Print a project's next-action list (or run a mutating subcommand). The list is
/// user ground truth in `notes/next.md`; the AI never touches it (charter §5 原则4).
fn cmd_note(action: Option<NoteAction>, path: Option<PathBuf>) -> Result<()> {
    let meta = resolve_project(path)?;
    let mut doc = omniproj_core::NextDoc::load(&meta.hash);
    match action {
        Some(NoteAction::Add { text, unclear }) => {
            let id = doc.add(&text, unclear);
            doc.save(&meta.hash)?;
            let tag = if unclear { " (unclear)" } else { "" };
            eprintln!("[omniproj] added #{id}{tag}: {}", text.trim());
        }
        Some(NoteAction::Done { id }) => {
            if doc.set_done(&id, true) {
                doc.save(&meta.hash)?;
                eprintln!("[omniproj] #{id} done");
            } else {
                anyhow::bail!("no item with id #{id} (run `omniproj note` to list)");
            }
        }
        Some(NoteAction::Rm { id }) => {
            if doc.remove(&id) {
                doc.save(&meta.hash)?;
                eprintln!("[omniproj] removed #{id}");
            } else {
                anyhow::bail!("no item with id #{id} (run `omniproj note` to list)");
            }
        }
        None => print_note_list(&meta.name, &doc),
    }
    Ok(())
}

/// Render one project's list to stdout. `?` = unclear, `#id` = the handle for
/// `note done`/`note rm`/`clarify`.
fn print_note_list(project: &str, doc: &omniproj_core::NextDoc) {
    let items: Vec<_> = doc.items().collect();
    if items.is_empty() {
        eprintln!("[omniproj] {project}: no next actions — `omniproj note add \"...\"`");
        return;
    }
    let (open, unclear) = doc.counts();
    println!("# Next — {project}  ({open} open, {unclear} unclear)");
    for it in items {
        let check = if it.done { "x" } else { " " };
        let q = if it.unclear { "? " } else { "" };
        let id = it.id.as_deref().unwrap_or("----");
        println!("  [{check}] #{id}  {q}{}", it.text);
    }
}

/// Cross-project next-action overview. Sorts by the mtime of each `next.md` — i.e.
/// which lists you've most recently touched. That is a neutral fact (charter §5
/// 原则3: 标记 ≠ 建议); it is NOT an importance ranking and makes no recommendation.
fn cmd_next() -> Result<()> {
    struct Row {
        name: String,
        doc: omniproj_core::NextDoc,
        touched: Option<std::time::SystemTime>,
    }
    let mut rows: Vec<Row> = omniproj_core::list_projects()
        .into_iter()
        .map(|m| {
            let touched = std::fs::metadata(omniproj_core::next_path(&m.hash))
                .and_then(|md| md.modified())
                .ok();
            Row {
                name: m.name,
                doc: omniproj_core::NextDoc::load(&m.hash),
                touched,
            }
        })
        .filter(|r| r.doc.items().next().is_some())
        .collect();
    // Most-recently-touched list first; untouched (None) last; tie-break by name.
    rows.sort_by(|a, b| b.touched.cmp(&a.touched).then_with(|| a.name.cmp(&b.name)));
    if rows.is_empty() {
        eprintln!("[omniproj] no next actions anywhere — `omniproj note add \"...\"` in a project");
        return Ok(());
    }
    println!(
        "# Next across projects (by most-recently-edited list — a neutral fact, not a ranking)"
    );
    for r in rows {
        let (open, unclear) = r.doc.counts();
        if open == 0 {
            continue;
        }
        let flag = if unclear > 0 {
            format!(" · {unclear} unclear")
        } else {
            String::new()
        };
        println!("\n## {} ({open} open{flag})", r.name);
        for it in r.doc.items().filter(|t| !t.done) {
            let q = if it.unclear { "? " } else { "" };
            let id = it.id.as_deref().unwrap_or("----");
            println!("  #{id}  {q}{}", it.text);
        }
    }
    Ok(())
}

/// `~/.omniproj/projects/<hash>/auto/clarify/<id>.md` — the AI-written discussion for one
/// note item. Under auto/ (derivative, revertable), NOT notes/ (charter §6 guardrail).
fn clarify_file(hash: &str, id: &str) -> PathBuf {
    omniproj_core::auto_dir(hash)
        .join("clarify")
        .join(format!("{id}.md"))
}

/// Run one clarify round on a next-action item: send the item (+ prior discussion +
/// this round's note) to the `[clarify]` model, append 标记+理由 to the item's
/// discussion file, and commit it (revertable). Never writes `notes/` — the user
/// transcribes any conclusion (charter §6 例外 guardrail).
async fn cmd_clarify(id: String, message: Option<String>, path: Option<PathBuf>) -> Result<()> {
    let meta = resolve_project(path)?;
    // The item must exist and be the user's own text (charter §5 原则4). We read it
    // read-only; clarify never edits the note.
    let doc = omniproj_core::NextDoc::load(&meta.hash);
    let item = doc
        .items()
        .find(|t| t.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| {
            anyhow::anyhow!("no note item with id #{id} — run `omniproj note` to list")
        })?;
    let item_text = item.text.clone();

    let resolved = omniproj_distill::resolve_clarify()?;
    // Privacy: the item text (user's words) goes to the provider. Explicit per-call
    // action, but say so — same discipline as briefing (spec §5, W1-1).
    if !omniproj_distill::is_local_provider(&resolved.provider_name) {
        eprintln!(
            "[omniproj] clarify sends this item's text to {} — a local Ollama model keeps it on-machine.",
            resolved.provider_name
        );
    }

    let file = clarify_file(&meta.hash, &id);
    let prior = std::fs::read_to_string(&file).unwrap_or_default();

    eprintln!(
        "[omniproj] clarifying #{id} with {} …",
        resolved.provider_name
    );
    let round =
        omniproj_distill::clarify_round(&item_text, &prior, message.as_deref(), &resolved.provider)
            .await?;

    let now = now_rfc3339();
    let rendered = omniproj_distill::render_round(&now, message.as_deref(), &round);
    let updated = format!("{prior}{rendered}");

    omniproj_core::ensure_home()?;
    omniproj_core::store_txn(|| -> Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file, &updated)?;
        omniproj_core::commit_all(&format!("clarify {} #{id}", meta.name));
        Ok(())
    })?;

    println!("{}", round.trim());
    // Self-monitoring counter (charter §10): how many rounds this week, across all items.
    let week = weekly_clarify_rounds(&meta.hash, &now);
    eprintln!(
        "\n[omniproj] round recorded → {} · {week} clarify round(s) this week in {}",
        file.display(),
        meta.name
    );
    eprintln!(
        "[omniproj] this is discussion, not a decision — transcribe any conclusion into your note yourself (`omniproj note add`)."
    );
    Ok(())
}

/// Count clarify rounds in the last 7 days across all of a project's discussion files
/// (charter §10 张力监控:「本周 N 轮」). Parses the RFC3339 markers via chrono.
fn weekly_clarify_rounds(hash: &str, now_rfc: &str) -> usize {
    let Some(now) = parse_rfc3339_epoch(now_rfc) else {
        return 0;
    };
    let dir = omniproj_core::auto_dir(hash).join("clarify");
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                total += omniproj_distill::count_rounds_within(
                    &text,
                    now,
                    7 * 86_400,
                    parse_rfc3339_epoch,
                );
            }
        }
    }
    total
}

fn parse_rfc3339_epoch(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|dt| dt.timestamp())
}

/// Non-empty `notes/*.md` files for a project, as (filename, body) sorted by name.
/// User-authored ground truth (charter §5 原则4); surfaced read-only by `recall`.
fn read_notes(hash: &str) -> Vec<(String, String)> {
    let dir = omniproj_core::notes_dir(hash);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&p) {
                if !text.trim().is_empty() {
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("note.md")
                        .to_string();
                    out.push((name, text));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// `auto/` files with a pending `.incoming` sibling (an unresolved reconcile from a
/// user hand-edit that a distill declined to clobber — spec §8, charter §5 原则4).
/// Returns basenames like `"briefing.md"`, sorted.
fn pending_reconciles(hash: &str) -> Vec<String> {
    let dir = omniproj_core::auto_dir(hash);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name();
            if let Some(name) = name.to_str() {
                if let Some(base) = name.strip_suffix(".incoming") {
                    out.push(base.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// `omniproj reconcile` (spec §8, charter §5 原则4): surface / resolve conflicts where a
/// user hand-edit to `auto/` was preserved and the AI version parked in `.incoming`.
/// No flag = show diffs (read-only). `--keep-mine` / `--take-ai` resolve every pending
/// file, then commit.
fn cmd_reconcile(path: Option<PathBuf>, keep_mine: bool, take_ai: bool) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    let hash = omniproj_core::project_hash(&abs);
    let auto = omniproj_core::auto_dir(&hash);
    let pending = pending_reconciles(&hash);
    if pending.is_empty() {
        eprintln!("[omniproj] {name}: no pending reconciles");
        return Ok(());
    }

    if !keep_mine && !take_ai {
        // Read-only default: show what diverged, never auto-resolve.
        println!(
            "# Reconcile — {name}: {} file(s) with pending AI updates you haven't merged",
            pending.len()
        );
        for base in &pending {
            let mine = auto.join(base);
            let incoming = auto.join(format!("{base}.incoming"));
            println!("\n## {base}  (yours vs AI .incoming)");
            match diff_files(&mine, &incoming) {
                Some(d) if !d.trim().is_empty() => println!("{}", d.trim_end()),
                _ => println!("(no textual difference)"),
            }
        }
        eprintln!(
            "\n[omniproj] resolve with `omniproj reconcile {name} --keep-mine` (discard AI) or \
             `--take-ai` (replace yours). auto/ is AI territory; keep durable notes in notes/."
        );
        return Ok(());
    }

    omniproj_core::ensure_home()?;
    omniproj_core::store_txn(|| -> Result<()> {
        resolve_pending(&auto, &pending, take_ai, |msg| eprintln!("{msg}"))?;
        let how = if take_ai { "take-ai" } else { "keep-mine" };
        omniproj_core::commit_all(&format!("reconcile {name} ({how})"));
        Ok(())
    })?;
    Ok(())
}

/// Resolve every pending reconcile in `auto` (pure filesystem ops, no git — the caller
/// wraps this in a store transaction + commit). `take_ai` adopts the `.incoming`
/// version; otherwise the user's file is kept and the `.incoming` discarded.
fn resolve_pending(
    auto: &Path,
    pending: &[String],
    take_ai: bool,
    log: impl Fn(&str),
) -> Result<()> {
    for base in pending {
        let mine = auto.join(base);
        let incoming = auto.join(format!("{base}.incoming"));
        if take_ai {
            std::fs::rename(&incoming, &mine)?;
            log(&format!("[omniproj] {base}: took AI version"));
        } else {
            std::fs::remove_file(&incoming)?;
            log(&format!("[omniproj] {base}: kept your version"));
        }
    }
    Ok(())
}

/// Readable unified diff between two files. Shells to `git diff --no-index` (the store
/// already depends on git); returns the diff body or `None` if git is unavailable.
/// `git diff --no-index` exits 1 when files differ, so a non-zero status is expected.
fn diff_files(mine: &Path, incoming: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["diff", "--no-index", "--"])
        .arg(mine)
        .arg(incoming)
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn cmd_stats() -> Result<()> {
    let projects = omniproj_core::list_projects();
    if projects.is_empty() {
        eprintln!("[omniproj] no registered projects");
        return Ok(());
    }
    let home = omniproj_core::omniproj_home();
    let lines_of = |rel: &str| -> Option<usize> {
        std::fs::read_to_string(home.join(rel))
            .ok()
            .map(|t| t.lines().count())
    };
    let revs_of = |rel: &str| -> usize {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&home)
            .args(["rev-list", "--count", "HEAD", "--", rel])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    };
    println!(
        "{:<20} {:>9} {:>10} {:>6} {:>8} {:>6}",
        "PROJECT", "briefing", "decisions", "open", "learned", "revs"
    );
    for p in &projects {
        let base = format!("projects/{}", p.hash);
        let fmt = |v: Option<usize>| v.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
        println!(
            "{:<20} {:>9} {:>10} {:>6} {:>8} {:>6}",
            p.name,
            fmt(lines_of(&format!("{base}/auto/briefing.md"))),
            fmt(lines_of(&format!("{base}/auto/decisions.md"))),
            fmt(lines_of(&format!("{base}/auto/open.md"))),
            fmt(lines_of(&format!("{base}/learned.md"))),
            revs_of(&base),
        );
    }
    eprintln!("\n[omniproj] 健康信号 = decisions/open 行数稳定或下降(spec §10);revs = store 中该项目的演化次数");
    Ok(())
}

async fn cmd_eval(
    path: Option<PathBuf>,
    gold: PathBuf,
    candidate: Option<PathBuf>,
    model: Option<String>,
) -> Result<()> {
    let dir = resolve_dir(path)?;
    let canon = std::fs::canonicalize(&dir).unwrap_or(dir);
    let abs = canon.to_string_lossy().to_string();
    let name = canon
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let hash = omniproj_core::project_hash(&abs);

    let gold_text =
        std::fs::read_to_string(&gold).with_context(|| format!("read gold {}", gold.display()))?;
    let (candidate_text, candidate_label) = match candidate {
        Some(p) => (
            std::fs::read_to_string(&p)
                .with_context(|| format!("read candidate {}", p.display()))?,
            p.display().to_string(),
        ),
        None => (
            stored_candidate(&hash).with_context(|| {
                format!(
                    "build candidate from stored state for {name}; run `omniproj briefing` first or pass --candidate"
                )
            })?,
            format!("~/.omniproj/projects/{hash}/auto/{{briefing,open,decisions}}.md"),
        ),
    };

    let resolved = omniproj_distill::resolve(model.as_deref())
        .context("could not resolve an LLM provider (see `omniproj providers`)")?;
    let provider_label = format!("{}/{}", resolved.provider_name, resolved.model);
    eprintln!("[omniproj] evaluating with {provider_label} …");
    let scores = omniproj_distill::judge(&gold_text, &candidate_text, &resolved.provider).await?;

    println!("{:<10} SCORE", "DIMENSION");
    println!("{:<10} {}", "factual", scores.factual);
    println!("{:<10} {}", "coverage", scores.coverage);
    println!("{:<10} {}", "concision", scores.concision);
    println!("\n{}", scores.rationale);

    let report = serde_json::json!({
        "at": now_rfc3339(),
        "project": {"name": name, "path": abs, "hash": hash},
        "gold": gold.display().to_string(),
        "candidate": candidate_label,
        "provider": provider_label,
        "scores": {
            "factual": scores.factual,
            "coverage": scores.coverage,
            "concision": scores.concision,
            "rationale": scores.rationale,
        }
    });
    if omniproj_core::load_meta(&hash).is_some() {
        let cache = omniproj_core::cache_dir(&hash);
        std::fs::create_dir_all(&cache)?;
        let path = cache.join("eval-report.json");
        std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
        eprintln!("[omniproj] wrote {}", path.display());
    } else {
        eprintln!("[omniproj] project is not registered; eval report not persisted");
    }
    Ok(())
}

fn stored_candidate(hash: &str) -> Result<String> {
    let auto = omniproj_core::auto_dir(hash);
    let mut out = String::new();
    for kind in ["briefing", "open", "decisions"] {
        let text = std::fs::read_to_string(auto.join(format!("{kind}.md"))).unwrap_or_default();
        if !text.trim().is_empty() {
            out.push_str(&format!("## {kind}\n{}\n\n", text.trim()));
        }
    }
    if out.trim().is_empty() {
        anyhow::bail!("no stored state files");
    }
    Ok(out)
}

fn cmd_providers() -> Result<()> {
    println!(
        "{:<12} {:<9} {:<3} {:<20} BASE URL",
        "PROVIDER", "KIND", "KEY", "KEY-ENV"
    );
    for p in omniproj_distill::list() {
        let mark = if p.key_present { "✓" } else { "✗" };
        let env = p.api_key_env.as_deref().unwrap_or("(none)");
        println!(
            "{:<12} {:<9} {:<3} {:<20} {}",
            p.name,
            p.kind.as_str(),
            mark,
            env,
            p.base_url
        );
    }
    eprintln!(
        "\nSelect with `--model provider/model` or `default_model` in {}",
        omniproj_distill::config_path().display()
    );
    Ok(())
}

async fn cmd_doctor() -> Result<()> {
    eprintln!("[omniproj] running diagnostics (read-only)…\n");
    let checks = omniproj_distill::doctor::run().await;
    for c in &checks {
        println!("{}", c.format_line());
    }
    let fails = checks
        .iter()
        .filter(|c| c.status == omniproj_distill::doctor::CheckStatus::Fail)
        .count();
    let warns = checks
        .iter()
        .filter(|c| c.status == omniproj_distill::doctor::CheckStatus::Warn)
        .count();
    eprintln!();
    if fails > 0 {
        eprintln!("[omniproj] {fails} failing, {warns} warning — fix the FAIL items above.");
        // Exit non-zero so `omniproj doctor` is usable as a setup gate in CI / scripts.
        // `process::exit` skips destructors, so flush the report we just printed.
        use std::io::Write;
        std::io::stdout().flush().ok();
        std::io::stderr().flush().ok();
        std::process::exit(1);
    } else if warns > 0 {
        eprintln!("[omniproj] all core checks pass, {warns} warning(s) to be aware of.");
    } else {
        eprintln!("[omniproj] all checks pass — you're ready to distill.");
    }
    Ok(())
}

fn cmd_init() -> Result<()> {
    let path = omniproj_distill::config_path();
    if path.exists() {
        eprintln!("[omniproj] config already exists: {}", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, omniproj_distill::CONFIG_TEMPLATE)?;
    eprintln!("[omniproj] wrote {}", path.display());
    eprintln!(
        "\n[omniproj] privacy: distillation sends a digest of your git activity and \
Claude/Codex\n\
\x20        session text to the LLM provider you configure. OmniProj drops sensitive \
paths\n\
\x20        (.env, *.key, secrets/, …) and masks secret shapes by default, and \
`omniproj digest`\n\
\x20        shows the exact outbound text before you send it.\n\
\x20        For a fully local, nothing-leaves-the-machine setup, point default_model \
at Ollama\n\
\x20        (e.g. default_model = \"ollama/llama3.1\"). Set `send_consent = true` \
under [privacy]\n\
\x20        once you've reviewed this to silence the per-run reminder."
    );
    Ok(())
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn tmp_auto(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "omniproj-reconcile-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `--keep-mine` discards the AI `.incoming` and preserves the user's file.
    #[test]
    fn resolve_pending_keep_mine() {
        let auto = tmp_auto("keep");
        std::fs::write(auto.join("briefing.md"), "USER EDIT").unwrap();
        std::fs::write(auto.join("briefing.md.incoming"), "AI VERSION").unwrap();

        resolve_pending(&auto, &["briefing.md".to_string()], false, |_| {}).unwrap();

        assert_eq!(
            std::fs::read_to_string(auto.join("briefing.md")).unwrap(),
            "USER EDIT"
        );
        assert!(!auto.join("briefing.md.incoming").exists());
        std::fs::remove_dir_all(&auto).ok();
    }

    /// `--take-ai` replaces the user's file with the AI `.incoming` and clears it.
    #[test]
    fn resolve_pending_take_ai() {
        let auto = tmp_auto("take");
        std::fs::write(auto.join("open.md"), "USER EDIT").unwrap();
        std::fs::write(auto.join("open.md.incoming"), "AI VERSION").unwrap();

        resolve_pending(&auto, &["open.md".to_string()], true, |_| {}).unwrap();

        assert_eq!(
            std::fs::read_to_string(auto.join("open.md")).unwrap(),
            "AI VERSION"
        );
        assert!(!auto.join("open.md.incoming").exists());
        std::fs::remove_dir_all(&auto).ok();
    }
}
