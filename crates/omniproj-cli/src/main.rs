//! omniproj — CLI front-end. Post-pivot (desktop-design §6): the desktop app owns the
//! Attend / Record / Advance layers; the CLI keeps project registration plus the
//! capture-side and notes utilities the desktop has not yet subsumed. The background
//! daemon, the axum dashboard, and the distill/opinion/eval surface were removed with
//! the pivot to the Tauri desktop app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

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
    /// Health stats per project: state-file sizes + store history (健康信号 =
    /// note 数量稳定或下降, spec §10). No LLM.
    Stats,
    /// List configured LLM providers (predefined + custom) and whether their key is set.
    Providers,
    /// Write a starter `~/.omniproj/config.toml` (providers + default model).
    Init,
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
    /// Mark an item in-progress (doing) by its short id.
    Doing {
        /// Short id (e.g. `a3f1`).
        id: String,
    },
    /// Mark an item done by its short id (the `#id` shown in the list).
    Done {
        /// Short id (e.g. `a3f1`).
        id: String,
    },
    /// Set (or clear) an item's expected-completion date by its short id.
    Due {
        /// Short id (e.g. `a3f1`).
        id: String,
        /// Date as `YYYY-MM-DD`. Omit to clear the due date.
        date: Option<String>,
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
        Cmd::Add { path } => cmd_add(path),
        Cmd::List => cmd_list(),
        Cmd::Remove { path } => cmd_remove(path),
        Cmd::Search { query, path, limit } => cmd_search(query, path, limit),
        Cmd::Recall { path } => cmd_recall(path),
        Cmd::Note { action, path } => cmd_note(action, path),
        Cmd::Next => cmd_next(),
        Cmd::Clarify { id, message, path } => cmd_clarify(id, message, path).await,
        Cmd::Stats => cmd_stats(),
        Cmd::Providers => cmd_providers(),
        Cmd::Init => cmd_init(),
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

fn cmd_add(path: Option<PathBuf>) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    omniproj_core::ensure_home()?;
    let meta = omniproj_core::register(&abs, &name, &now_rfc3339())?;
    omniproj_core::commit_all(&format!("register {name} ({})", meta.hash));
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

fn cmd_remove(path: Option<PathBuf>) -> Result<()> {
    let (abs, name) = resolve_repo(path)?;
    let hash = omniproj_core::project_hash(&abs);
    if omniproj_core::load_meta(&hash).is_none() {
        eprintln!("[omniproj] {name} [{hash}] is not registered");
        return Ok(());
    }
    let kept_notes = omniproj_core::remove_project(&hash);
    omniproj_core::commit_all(&format!("unregister {name} ({hash})"));
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
            "no registered project for {} — run `omniproj add` first",
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
    if !printed {
        eprintln!("[omniproj] no stored state yet — add next-actions with `omniproj note add`");
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
        Some(NoteAction::Doing { id }) => {
            if doc.set_status(&id, omniproj_core::TaskStatus::Doing) {
                doc.save(&meta.hash)?;
                eprintln!("[omniproj] #{id} doing");
            } else {
                anyhow::bail!("no item with id #{id} (run `omniproj note` to list)");
            }
        }
        Some(NoteAction::Done { id }) => {
            if doc.set_done(&id, true) {
                doc.save(&meta.hash)?;
                eprintln!("[omniproj] #{id} done");
            } else {
                anyhow::bail!("no item with id #{id} (run `omniproj note` to list)");
            }
        }
        Some(NoteAction::Due { id, date }) => {
            if doc.set_due(&id, date.clone()) {
                doc.save(&meta.hash)?;
                match date {
                    Some(d) => eprintln!("[omniproj] #{id} due {d}"),
                    None => eprintln!("[omniproj] #{id} due date cleared"),
                }
            } else {
                anyhow::bail!(
                    "could not set due for #{id} — unknown id, or date is not YYYY-MM-DD"
                );
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
        let check = match it.status {
            omniproj_core::TaskStatus::Open => " ",
            omniproj_core::TaskStatus::Doing => "/",
            omniproj_core::TaskStatus::Done => "x",
        };
        let q = if it.unclear { "? " } else { "" };
        let id = it.id.as_deref().unwrap_or("----");
        let due = it
            .due
            .as_deref()
            .map(|d| format!("  (due {d})"))
            .unwrap_or_default();
        println!("  [{check}] #{id}  {q}{}{due}", it.text);
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
        for it in r.doc.items().filter(|t| !t.status.is_done()) {
            let q = if it.unclear { "? " } else { "" };
            let doing = if it.status == omniproj_core::TaskStatus::Doing {
                "▸ "
            } else {
                ""
            };
            let id = it.id.as_deref().unwrap_or("----");
            let due = it
                .due
                .as_deref()
                .map(|d| format!("  (due {d})"))
                .unwrap_or_default();
            println!("  #{id}  {doing}{q}{}{due}", it.text);
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
