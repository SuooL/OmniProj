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
    /// One-line problem note (问题备注), or null.
    note: Option<String>,
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
            note: t.note.clone(),
        })
        .collect()
}

/// IPC command: set (Some) or clear (None/empty) a task's one-line problem note.
#[tauri::command]
fn set_task_note(hash: String, id: String, note: Option<String>) -> Result<(), String> {
    mutate(&hash, "task note", |doc| {
        if doc.set_note(&id, note.clone()) {
            Ok(())
        } else {
            Err(format!("unknown id #{id}"))
        }
    })
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

/// Desktop reminder settings — the *controlled push* knob (charter §4d / §5 原则5:
/// cadence and threshold are user-visible, adjustable, and switchable off). Persisted
/// at `~/.omniproj/desktop.toml`.
#[derive(Serialize, serde::Deserialize, Clone)]
struct Settings {
    /// Master switch for the daily reminder.
    reminders_enabled: bool,
    /// A project with no commit within this many days "needs attention".
    silence_days: u32,
    /// How often the reminder check runs (hours).
    interval_hours: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            reminders_enabled: true,
            silence_days: 7,
            interval_hours: 24,
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    omniproj_core::omniproj_home().join("desktop.toml")
}

fn load_settings() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn get_settings() -> Settings {
    load_settings()
}

#[tauri::command]
fn set_settings(settings: Settings) -> Result<(), String> {
    omniproj_core::ensure_home().map_err(|e| e.to_string())?;
    let body = toml::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(), body).map_err(|e| e.to_string())
}

/// Registered projects that need attention: no commit within `silence_days` (or no git
/// history). A neutral staleness fact — no scoring or ranking (charter §8 护栏).
fn attention_projects(silence_days: u32) -> Vec<String> {
    let now = chrono::Utc::now().timestamp();
    let cutoff = now - (silence_days as i64) * 86_400;
    omniproj_core::list_projects()
        .into_iter()
        .filter(|m| {
            let path = Path::new(&m.path);
            match omniproj_capture::git::commit_log(path, 1).first() {
                Some(c) => chrono::NaiveDate::parse_from_str(&c.date, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc().timestamp() < cutoff)
                    .unwrap_or(false),
                None => true, // no commits → stale
            }
        })
        .map(|m| m.name)
        .collect()
}

/// IPC command: the names of projects currently needing attention (for the in-app badge).
#[tauri::command]
fn get_attention() -> Vec<String> {
    attention_projects(load_settings().silence_days)
}

/// IPC command: fire a native notification immediately so the user can confirm the push
/// path works (and grant OS permission) without waiting for the daily cadence.
#[tauri::command]
fn test_reminder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    let names = attention_projects(load_settings().silence_days);
    let body = if names.is_empty() {
        "No projects need attention right now.".to_string()
    } else {
        format!("{} need attention: {}", names.len(), names.join(", "))
    };
    app.notification()
        .builder()
        .title("OmniProj reminder (test)")
        .body(body)
        .show()
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::TrayIconBuilder;
            use tauri::Manager;

            // Menu-bar presence (charter §3 Attend): a tray icon with a minimal menu.
            let show = MenuItemBuilder::with_id("show", "Open OmniProj").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;
            let count = attention_projects(load_settings().silence_days).len();
            let _tray = TrayIconBuilder::with_id("main")
                .icon(
                    app.default_window_icon()
                        .expect("app icon set in tauri.conf.json")
                        .clone(),
                )
                .tooltip(format!("OmniProj — {count} project(s) need attention"))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(_tray); // keep the tray alive for the app's lifetime

            // Controlled daily push (charter §4d): recompute the attention list on a
            // cadence and send ONE native summary notification when non-empty. Never
            // interrupts visually beyond the OS notification; off when disabled.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use std::time::Duration;
                use tauri_plugin_notification::NotificationExt;
                // Brief startup delay so the first check doesn't race window creation.
                tokio::time::sleep(Duration::from_secs(3)).await;
                loop {
                    let s = load_settings();
                    if s.reminders_enabled {
                        let names = attention_projects(s.silence_days);
                        if !names.is_empty() {
                            let _ = handle
                                .notification()
                                .builder()
                                .title(format!("{} project(s) need attention", names.len()))
                                .body(names.join(", "))
                                .show();
                        }
                    }
                    let hrs = load_settings().interval_hours.max(1);
                    tokio::time::sleep(Duration::from_secs(hrs * 3600)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_projects,
            get_tasks,
            add_task,
            set_task_status,
            set_task_due,
            set_task_note,
            remove_task,
            get_commits,
            attribute_commit,
            unattribute_commit,
            get_settings,
            set_settings,
            get_attention,
            test_reminder
        ])
        .run(tauri::generate_context!())
        .expect("error while running omniproj desktop");
}
