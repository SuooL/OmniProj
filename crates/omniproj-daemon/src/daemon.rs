//! The background daemon (spec §5/§7 "Daemon/Orchestrator").
//!
//! Two change sources feed one gated [`refresh_project`](crate::refresh_project):
//! a **passive** fs watcher over every registered worktree (catches commits + edits)
//! and an **active floor timer** (the backstop for session-only work that never
//! touches the tree, spec §5 "为什么必须有 floor"). Both only *enqueue* a project;
//! the actual distill runs on a **single off-loop worker** so the LLM call — the one
//! slow step — never stalls event ingestion (rust-analyzer-style single state task).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tokio::sync::{mpsc, Mutex};

use crate::refresh::{refresh_project, RefreshOpts, RefreshOutcome};

/// Live daemon state exposed over IPC (`omniproj status`). The run loop fills `watched`
/// and the worker toggles `in_flight`; the IPC server reads it. A std Mutex is right
/// here — every critical section is a tiny, non-async field access.
struct Shared {
    pid: u32,
    started_at: String,
    watched: HashSet<String>,
    in_flight: Option<String>,
}

type SharedState = Arc<StdMutex<Shared>>;

/// Poison-tolerant lock on the shared status state (crash recovery, W2-1). If a task
/// panics while holding this lock the std Mutex is poisoned and every later
/// `.lock().unwrap()` would panic too, cascading one bad job into a dead daemon. The
/// shared state is only tiny status fields (never a torn invariant), so recovering the
/// inner guard is always safe here — we take the data and keep serving.
fn lock_shared(m: &StdMutex<Shared>) -> MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

enum Control {
    ReloadRegistry,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Build a status snapshot: the registry (source of truth for projects + last-distill)
/// merged with the daemon's live `watched` / `in_flight`.
fn build_status(shared: &SharedState) -> omniproj_ipc::StatusResponse {
    let s = lock_shared(shared);
    let projects = omniproj_core::list_projects()
        .into_iter()
        .map(|m| omniproj_ipc::ProjectStatus {
            watched: s.watched.contains(&m.path),
            last_activity: m
                .last_distilled
                .map(|t| format!("distilled {t}"))
                .unwrap_or_else(|| "never".into()),
            name: m.name,
            hash: m.hash,
            path: m.path,
        })
        .collect();
    omniproj_ipc::StatusResponse {
        pid: s.pid,
        started_at: s.started_at.clone(),
        in_flight: s.in_flight.clone(),
        projects,
    }
}

/// Accept IPC connections and answer Ping/Status. Runs until the process exits (the
/// task is dropped when `run` returns and the socket is cleaned up).
async fn serve_ipc(
    listener: tokio::net::UnixListener,
    shared: SharedState,
    control_tx: mpsc::UnboundedSender<Control>,
) {
    while let Ok((mut stream, _)) = listener.accept().await {
        let shared = shared.clone();
        let control_tx = control_tx.clone();
        tokio::spawn(async move {
            let req = match omniproj_ipc::server::read_request(&mut stream).await {
                Ok(r) => r,
                Err(_) => return,
            };
            let resp = match req {
                omniproj_ipc::Request::Ping => omniproj_ipc::Response::Pong {
                    pid: std::process::id(),
                },
                omniproj_ipc::Request::Status => {
                    omniproj_ipc::Response::Status(build_status(&shared))
                }
                omniproj_ipc::Request::Reload => match control_tx.send(Control::ReloadRegistry) {
                    Ok(()) => omniproj_ipc::Response::Ack,
                    Err(_) => omniproj_ipc::Response::Error(
                        "daemon run loop is not accepting reload requests".into(),
                    ),
                },
            };
            let _ = omniproj_ipc::server::write_response(&mut stream, &resp).await;
        });
    }
}

/// Daemon tuning. Defaults honor the spec's 24h floor; the watcher gives sub-second
/// responsiveness so the timer is purely a staleness ceiling.
pub struct DaemonOpts {
    /// Active floor probe interval — the maximum staleness before a forced re-check.
    pub interval: Duration,
    /// fs-event debounce window (one save/commit fires many raw events).
    pub debounce: Duration,
    /// Quiet window for SESSION transcript events. A live conversation appends to
    /// its jsonl continuously; refreshing per-write would mean an LLM call every few
    /// seconds. The semantic is "re-distill when the conversation pauses": session
    /// events fire only after this much silence (benchmark review P0#2).
    pub session_quiet: Duration,
    /// Model as `provider/model`; `None` falls back to config / env.
    pub model: Option<String>,
    /// Reasoning depth ("shallow"/"deep"); `None` falls back to config (spec §5.2).
    pub depth: Option<String>,
}

impl Default for DaemonOpts {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(24 * 60 * 60), // 24h floor (spec §5)
            debounce: Duration::from_millis(1500),
            session_quiet: Duration::from_secs(10 * 60), // conversation-pause window
            model: None,
            depth: None,
        }
    }
}

fn log(m: &str) {
    eprintln!("[daemon] {m}");
}

/// A unit of work for the worker: a project root to gate-refresh, keyed by hash for dedup.
struct Job {
    dir: PathBuf,
    hash: String,
}

/// Longest registered project path that is a prefix of `p` (the changed file's owner).
/// Pure, over an in-memory snapshot — no per-event IO.
fn owning_project<'a>(
    projects: &'a [omniproj_core::ProjectMeta],
    p: &Path,
) -> Option<&'a omniproj_core::ProjectMeta> {
    let s = p.to_string_lossy();
    projects
        .iter()
        .filter(|m| s == m.path.as_str() || s.starts_with(&format!("{}/", m.path)))
        .max_by_key(|m| m.path.len())
}

/// `.git/` internal churn we ignore (object writes, lock files) while still letting
/// commit/checkout signals (`HEAD`, `refs/`, `logs/`) wake the project.
///
/// Deliberately NOT interesting: `.git/index`. `refresh_project` itself shells out to
/// `git status`, which rewrites the index stat-cache — treating that as a signal would
/// make the daemon wake itself in a tight loop. Staged-but-uncommitted work is covered
/// by the worktree file events that preceded it and by the floor timer.
fn is_git_noise(p: &Path) -> bool {
    let s = p.to_string_lossy();
    if let Some(i) = s.find("/.git/") {
        let rest = &s[i + "/.git/".len()..];
        let interesting =
            rest.starts_with("HEAD") || rest.starts_with("refs/") || rest.starts_with("logs/");
        return !interesting;
    }
    // A dir-level event on `.git` itself (macOS FSEvents can coalesce to the directory):
    // we can't tell what changed inside, so treat it as noise to avoid self-wake loops.
    // Real commits also surface as file-level `.git/logs/HEAD` or `.git/refs/...` events,
    // and the floor timer is the backstop.
    s.ends_with("/.git")
}

/// Enqueue a project for refresh unless it's already queued/in-flight (dedup keeps a
/// noisy editor from piling up redundant distills).
async fn enqueue(
    job_tx: &mpsc::Sender<Job>,
    pending: &Arc<Mutex<HashSet<String>>>,
    meta: &omniproj_core::ProjectMeta,
) {
    let is_new = {
        let mut set = pending.lock().await;
        set.insert(meta.hash.clone())
    };
    if is_new {
        let _ = job_tx
            .send(Job {
                dir: PathBuf::from(&meta.path),
                hash: meta.hash.clone(),
            })
            .await;
    }
}

/// The single off-loop worker: drains jobs one at a time so distills never overlap
/// (bounds LLM/API load) and a crash in one project can't take down the daemon.
async fn worker(
    mut rx: mpsc::Receiver<Job>,
    pending: Arc<Mutex<HashSet<String>>>,
    model: Option<String>,
    depth: Option<String>,
    shared: SharedState,
) {
    while let Some(job) = rx.recv().await {
        // Clear the pending mark at the START of processing, so a change that lands
        // mid-distill re-enqueues and we re-check after (no lost updates).
        pending.lock().await.remove(&job.hash);

        // Surface what we're working on to `omniproj status` (cleared in all exit paths).
        let label = job
            .dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string();
        lock_shared(&shared).in_flight = Some(label);

        // Worker-panic isolation (crash recovery, W2-1): run the job on its own task
        // so a panic inside distillation (a bad substrate, a provider quirk) is caught
        // by the JoinHandle instead of unwinding the worker loop and killing the daemon.
        // Owned copies move into the task ('static); `log` is a Copy fn item.
        let dir = job.dir.clone();
        let model = model.clone();
        let depth = depth.clone();
        let handle = tokio::spawn(async move {
            tokio::time::timeout(
                // Per-job ceiling: deep mode is ≤ ~9 LLM calls × 120s HTTP timeout, so a
                // job exceeding this is wedged, not slow. The single worker must never
                // be lost to one project (benchmark review P1#4).
                Duration::from_secs(30 * 60),
                refresh_project(
                    &dir,
                    RefreshOpts {
                        force: false,
                        model: model.as_deref(),
                        depth: depth.as_deref(),
                        no_redact: false,
                    },
                    log,
                ),
            )
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("refresh timed out after 30m")))
        });
        let joined = handle.await;
        // Always clear in-flight, even on panic — never leave a stuck status.
        lock_shared(&shared).in_flight = None;
        let res = match joined {
            Ok(res) => res,
            Err(join_err) => {
                if join_err.is_panic() {
                    log(&format!(
                        "✗ distill worker panicked on {} — recovered, continuing",
                        job.dir.display()
                    ));
                } else {
                    log(&format!(
                        "✗ distill job cancelled for {}",
                        job.dir.display()
                    ));
                }
                continue;
            }
        };
        match res {
            Ok(RefreshOutcome::Distilled(d)) => {
                let verdict = if d.verify.is_clean() {
                    "verify clean".to_string()
                } else {
                    format!("{} flagged", d.verify.flagged.len())
                };
                log(&format!(
                    "✓ distilled {} ({}, {})",
                    d.name, d.provider_label, verdict
                ));
            }
            Ok(RefreshOutcome::UpToDate { name, .. }) => log(&format!("· {name}: up to date")),
            Ok(RefreshOutcome::NoSubstrate { name }) => log(&format!("· {name}: no substrate")),
            Ok(RefreshOutcome::Unregistered { name, .. }) => {
                log(&format!("· {name}: unregistered (skipped)"))
            }
            // Per-project isolation: log and keep serving everything else.
            Err(e) => log(&format!(
                "✗ refresh failed for {}: {e:#}",
                job.dir.display()
            )),
        }
    }
}

/// Block until SIGTERM (graceful service stop) or Ctrl-C.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Single-instance lock via an OS advisory flock on `~/.omniproj/daemon.lock`. The lock
/// is released automatically when the process exits (no stale-lock problem on crash).
/// The returned handle must be held for the daemon's lifetime.
fn acquire_lock() -> Result<std::fs::File> {
    use fs2::FileExt;
    let path = omniproj_core::omniproj_home().join("daemon.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("open lock {}", path.display()))?;
    file.try_lock_exclusive().map_err(|_| {
        anyhow::anyhow!(
            "another omniproj daemon is already running (lock held on {})",
            path.display()
        )
    })?;
    Ok(file)
}

/// Run the daemon in the foreground until a shutdown signal. Holds the single-instance
/// lock, watches all registered worktrees, sweeps on the floor interval, and re-scans
/// the registry each tick so projects added via `omniproj add` get picked up live.
pub async fn run(opts: DaemonOpts) -> Result<()> {
    omniproj_core::ensure_home()?;
    let _lock = acquire_lock()?; // held for the process lifetime

    // Live state shared with the IPC server (`omniproj status`).
    let shared: SharedState = Arc::new(StdMutex::new(Shared {
        pid: std::process::id(),
        started_at: now_rfc3339(),
        watched: HashSet::new(),
        in_flight: None,
    }));

    // IPC server over ~/.omniproj/daemon.sock (lazy-started + queried by the CLI, spec §7).
    let listener = omniproj_ipc::server::bind().context("bind daemon socket")?;
    let (control_tx, mut control_rx) = mpsc::unbounded_channel::<Control>();
    tokio::spawn(serve_ipc(listener, shared.clone(), control_tx));

    // Worker + dedup queue.
    let (job_tx, job_rx) = mpsc::channel::<Job>(256);
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    tokio::spawn(worker(
        job_rx,
        pending.clone(),
        opts.model.clone(),
        opts.depth.clone(),
        shared.clone(),
    ));

    // fs watcher → debounced batches of changed paths, bridged onto the async loop.
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<Vec<PathBuf>>();
    let mut debouncer = new_debouncer(opts.debounce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<PathBuf> = events
                .into_iter()
                .flat_map(|e| e.event.paths.clone())
                .collect();
            if !paths.is_empty() {
                let _ = ev_tx.send(paths);
            }
        }
    })
    .context("init fs watcher")?;

    let mut projects = omniproj_core::list_projects();
    let mut watched: HashSet<String> = HashSet::new();
    for m in &projects {
        match debouncer.watch(Path::new(&m.path), RecursiveMode::Recursive) {
            Ok(()) => {
                watched.insert(m.path.clone());
                log(&format!("watching {} [{}]", m.name, m.hash));
            }
            Err(e) => log(&format!("could not watch {}: {e}", m.path)),
        }
        // Initial sweep: gate-refresh everything once at startup.
        enqueue(&job_tx, &pending, m).await;
    }
    lock_shared(&shared).watched = watched.clone();
    if projects.is_empty() {
        log("no registered projects — `omniproj add <repo>` then it'll be picked up on the next tick");
    }

    // Session-transcript watcher (benchmark review P0#2): conversation-only work
    // never touches the worktree, so watch the agent session roots too. The debouncer
    // only COALESCES bursts (it fires on a max-delay from the first event, not on
    // quiet); the real "conversation paused" semantic is the rolling per-project
    // deadline in `sess_pending` below — every new write pushes the deadline out.
    let (sess_tx, mut sess_rx) = mpsc::unbounded_channel::<Vec<PathBuf>>();
    let coalesce = Duration::from_secs(5).min(opts.session_quiet);
    let mut session_debouncer = new_debouncer(coalesce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<PathBuf> = events
                .into_iter()
                .flat_map(|e| e.event.paths.clone())
                .collect();
            if !paths.is_empty() {
                let _ = sess_tx.send(paths);
            }
        }
    })
    .context("init session watcher")?;
    for root in omniproj_capture::session_roots() {
        match session_debouncer.watch(&root, RecursiveMode::Recursive) {
            Ok(()) => log(&format!("watching sessions under {}", root.display())),
            Err(e) => log(&format!("could not watch {}: {e}", root.display())),
        }
    }
    // Projects with recent session activity, waiting for the conversation to pause:
    // hash → (meta, deadline). Refreshed (pushed out) on every new transcript write.
    let mut sess_pending: std::collections::HashMap<
        String,
        (omniproj_core::ProjectMeta, tokio::time::Instant),
    > = std::collections::HashMap::new();

    let mut ticker = tokio::time::interval(opts.interval);
    ticker.tick().await; // consume the immediate first tick (startup sweep already ran)

    log(&format!(
        "up — {} project(s), floor {}s, debounce {}ms, session-quiet {}s (Ctrl-C / SIGTERM to stop)",
        projects.len(),
        opts.interval.as_secs(),
        opts.debounce.as_millis(),
        opts.session_quiet.as_secs()
    ));

    loop {
        // The earliest pending session deadline, if any — drives the quiet-wake branch.
        let next_sess_wake = sess_pending.values().map(|(_, t)| *t).min();
        tokio::select! {
            Some(paths) = ev_rx.recv() => {
                for p in paths {
                    if is_git_noise(&p) {
                        continue;
                    }
                    if let Some(m) = owning_project(&projects, &p) {
                        enqueue(&job_tx, &pending, m).await;
                    }
                }
            }
            Some(paths) = sess_rx.recv() => {
                // Transcript writes: don't refresh yet — (re)arm the project's quiet
                // deadline. A live conversation keeps pushing it out; it fires only
                // once the conversation pauses for session_quiet.
                let deadline = tokio::time::Instant::now() + opts.session_quiet;
                for p in paths {
                    if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let Some(cwd) = omniproj_capture::session_owner_cwd(&p) else {
                        continue;
                    };
                    if let Some(m) = owning_project(&projects, Path::new(&cwd)) {
                        let first = !sess_pending.contains_key(&m.hash);
                        sess_pending.insert(m.hash.clone(), (m.clone(), deadline));
                        if first {
                            log(&format!(
                                "session activity → {} [{}] (refresh {}s after the conversation pauses)",
                                m.name, m.hash, opts.session_quiet.as_secs()
                            ));
                        }
                    }
                }
            }
            _ = async { tokio::time::sleep_until(next_sess_wake.unwrap()).await },
                if next_sess_wake.is_some() =>
            {
                let now = tokio::time::Instant::now();
                let due: Vec<String> = sess_pending
                    .iter()
                    .filter(|(_, (_, t))| *t <= now)
                    .map(|(h, _)| h.clone())
                    .collect();
                for h in due {
                    if let Some((m, _)) = sess_pending.remove(&h) {
                        log(&format!("conversation paused → refreshing {} [{}]", m.name, m.hash));
                        enqueue(&job_tx, &pending, &m).await;
                    }
                }
            }
            _ = ticker.tick() => {
                // Re-scan the registry: watch any newly-added projects, then sweep all
                // (the floor — bounds worst-case staleness, spec §5).
                projects = omniproj_core::list_projects();
                for m in &projects {
                    if !watched.contains(&m.path)
                        && debouncer
                            .watch(Path::new(&m.path), RecursiveMode::Recursive)
                            .is_ok()
                    {
                        watched.insert(m.path.clone());
                        log(&format!("now watching {} [{}]", m.name, m.hash));
                    }
                    enqueue(&job_tx, &pending, m).await;
                }
                lock_shared(&shared).watched = watched.clone();
            }
            Some(Control::ReloadRegistry) = control_rx.recv() => {
                // `omniproj add/remove` sends this so a long-lived daemon picks up
                // registry changes immediately instead of waiting for the 24h floor.
                projects = omniproj_core::list_projects();
                for m in &projects {
                    if !watched.contains(&m.path) {
                        match debouncer.watch(Path::new(&m.path), RecursiveMode::Recursive) {
                            Ok(()) => {
                                watched.insert(m.path.clone());
                                log(&format!("now watching {} [{}]", m.name, m.hash));
                                enqueue(&job_tx, &pending, m).await;
                            }
                            Err(e) => log(&format!("could not watch {}: {e}", m.path)),
                        }
                    }
                }
                lock_shared(&shared).watched = watched.clone();
            }
            _ = shutdown_signal() => {
                log("shutdown signal — exiting");
                break;
            }
        }
    }
    omniproj_ipc::server::cleanup(); // unlink the socket on graceful exit
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniproj_core::ProjectMeta;

    fn meta(path: &str) -> ProjectMeta {
        ProjectMeta {
            path: path.into(),
            name: "p".into(),
            hash: omniproj_core::project_hash(path),
            added_at: "t".into(),
            last_distilled: None,
            last_head: None,
            last_status_digest: None,
            last_session_mtime: None,
            cadence: None,
        }
    }

    #[test]
    fn git_index_churn_is_noise_so_the_daemon_doesnt_wake_itself() {
        // `git status` (run by refresh itself) rewrites these — must NOT wake the project.
        assert!(is_git_noise(Path::new("/repo/.git/index")));
        assert!(is_git_noise(Path::new("/repo/.git/index.lock")));
        assert!(is_git_noise(Path::new("/repo/.git/objects/ab/cdef")));
        assert!(is_git_noise(Path::new("/repo/.git"))); // dir-level FSEvents coalesce
    }

    #[test]
    fn commit_signals_are_not_noise() {
        assert!(!is_git_noise(Path::new("/repo/.git/HEAD")));
        assert!(!is_git_noise(Path::new("/repo/.git/refs/heads/main")));
        assert!(!is_git_noise(Path::new("/repo/.git/logs/HEAD")));
        assert!(!is_git_noise(Path::new("/repo/src/main.rs"))); // worktree edits matter
    }

    #[test]
    fn owning_project_picks_longest_prefix() {
        let projects = vec![meta("/u/git"), meta("/u/git/foo")];
        let hit = owning_project(&projects, Path::new("/u/git/foo/src/x.rs")).unwrap();
        assert_eq!(hit.path, "/u/git/foo");
        // a sibling sharing a string prefix but not a path boundary must not match
        assert!(owning_project(&[meta("/u/git/foo")], Path::new("/u/git/foobar/x")).is_none());
    }
}
