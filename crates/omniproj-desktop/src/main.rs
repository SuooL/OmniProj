// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::Path;

/// One project card for the Attend-layer overview. Neutral git-derived facts
/// only — no scores, no ranking (charter §8). This is the M0 shape; task
/// counts / staleness thresholds land in later milestones.
#[derive(Serialize)]
struct ProjectCard {
    name: String,
    hash: String,
    path: String,
    last_distilled: Option<String>,
    branch: Option<String>,
    /// Uncommitted lines (`git status --porcelain` count) — a fact, not a judgement.
    dirty: usize,
    /// 16-week commit histogram, oldest → newest (the sparkline).
    commit_weeks: Vec<u32>,
}

/// IPC command: the registered projects with their git-derived facts.
/// Replaces the old `GET /api/portfolio` HTTP handler — reused straight from
/// `omniproj-core` / `omniproj-capture`, no axum layer.
#[tauri::command]
fn get_projects() -> Vec<ProjectCard> {
    let now = chrono::Utc::now().timestamp();
    omniproj_core::list_projects()
        .into_iter()
        .map(|m| {
            let path = Path::new(&m.path);
            let git = omniproj_capture::git::collect(path);
            ProjectCard {
                name: m.name,
                hash: m.hash,
                branch: git.as_ref().map(|g| g.branch.clone()),
                dirty: git
                    .as_ref()
                    .map(|g| g.status_porcelain.lines().filter(|l| !l.is_empty()).count())
                    .unwrap_or(0),
                commit_weeks: omniproj_capture::git::commit_weeks(path, 16, now),
                path: m.path,
                last_distilled: m.last_distilled,
            }
        })
        .collect()
}

/// One next-action item for the Record layer (M2). User ground truth from
/// `notes/next.md`; the app only mutates it on explicit user action (charter §5 原则4).
#[derive(Serialize)]
struct TaskDto {
    id: Option<String>,
    text: String,
    /// "open" | "doing" | "done".
    status: String,
    unclear: bool,
    /// Expected-completion date `YYYY-MM-DD`, or null.
    due: Option<String>,
    /// Attributed commit SHAs (abbreviated) — the *actual* side of FR-R2.
    commits: Vec<String>,
}

/// IPC command: a project's next-action list (read-only, no LLM).
#[tauri::command]
fn get_tasks(hash: String) -> Vec<TaskDto> {
    omniproj_core::NextDoc::load(&hash)
        .items()
        .map(|t| TaskDto {
            id: t.id.clone(),
            text: t.text.clone(),
            status: t.status.as_str().to_string(),
            unclear: t.unclear,
            due: t.due.clone(),
            commits: t.commits.clone(),
        })
        .collect()
}

/// One commit on the Record-layer timeline (the *actual* line, FR-R2).
#[derive(Serialize)]
struct CommitDto {
    hash: String,
    short: String,
    date: String,
    author: String,
    subject: String,
}

/// IPC command: a project's recent git commits (newest first), for the timeline the
/// user attributes tasks against. Read-only.
#[tauri::command]
fn get_commits(hash: String, limit: usize) -> Vec<CommitDto> {
    let Some(meta) = omniproj_core::load_meta(&hash) else {
        return Vec::new();
    };
    omniproj_capture::git::commit_log(Path::new(&meta.path), limit)
        .into_iter()
        .map(|c| CommitDto {
            hash: c.hash,
            short: c.short,
            date: c.date,
            author: c.author,
            subject: c.subject,
        })
        .collect()
}

/// IPC command: attribute a commit (abbreviated SHA) to a task (FR-R2, many-to-one).
#[tauri::command]
fn attribute_commit(hash: String, id: String, sha: String) -> Result<(), String> {
    mutate(&hash, "task attribute", |doc| {
        if doc.attribute_commit(&id, &sha) {
            Ok(())
        } else {
            Err("unknown task id, or invalid commit sha".into())
        }
    })
}

/// IPC command: remove a commit attribution from a task.
#[tauri::command]
fn unattribute_commit(hash: String, id: String, sha: String) -> Result<(), String> {
    mutate(&hash, "task unattribute", |doc| {
        if doc.unattribute_commit(&id, &sha) {
            Ok(())
        } else {
            Err("commit was not attributed to that task".into())
        }
    })
}

/// Load → mutate → save + version the store in one revertable commit (charter §5:
/// every write to `~/.omniproj` is an independent commit). The closure runs the user's
/// intended edit; a returned `Err` aborts before any write.
fn mutate<T>(
    hash: &str,
    msg: &str,
    f: impl FnOnce(&mut omniproj_core::NextDoc) -> Result<T, String>,
) -> Result<T, String> {
    let mut doc = omniproj_core::NextDoc::load(hash);
    let out = f(&mut doc)?;
    omniproj_core::ensure_home().map_err(|e| e.to_string())?;
    let hash = hash.to_string();
    let msg = msg.to_string();
    omniproj_core::store_txn(move || -> std::io::Result<()> {
        doc.save(&hash)?;
        omniproj_core::commit_all(&format!("{msg} {hash}"));
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(out)
}

/// IPC command: append a next-action, returning its new id.
#[tauri::command]
fn add_task(hash: String, text: String, unclear: bool) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("task text is empty".into());
    }
    mutate(&hash, "task add", |doc| Ok(doc.add(&text, unclear)))
}

/// IPC command: set an item's status ("open" | "doing" | "done").
#[tauri::command]
fn set_task_status(hash: String, id: String, status: String) -> Result<(), String> {
    let st = omniproj_core::TaskStatus::parse(&status).ok_or("invalid status")?;
    mutate(&hash, "task status", |doc| {
        if doc.set_status(&id, st) {
            Ok(())
        } else {
            Err(format!("unknown id #{id}"))
        }
    })
}

/// IPC command: set (Some `YYYY-MM-DD`) or clear (None) an item's due date.
#[tauri::command]
fn set_task_due(hash: String, id: String, date: Option<String>) -> Result<(), String> {
    mutate(&hash, "task due", |doc| {
        if doc.set_due(&id, date.clone()) {
            Ok(())
        } else {
            Err("unknown id, or date is not YYYY-MM-DD".into())
        }
    })
}

/// IPC command: delete an item by id.
#[tauri::command]
fn remove_task(hash: String, id: String) -> Result<(), String> {
    mutate(&hash, "task rm", |doc| {
        if doc.remove(&id) {
            Ok(())
        } else {
            Err(format!("unknown id #{id}"))
        }
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_projects,
            get_tasks,
            add_task,
            set_task_status,
            set_task_due,
            remove_task,
            get_commits,
            attribute_commit,
            unattribute_commit
        ])
        .run(tauri::generate_context!())
        .expect("error while running omniproj desktop");
}
