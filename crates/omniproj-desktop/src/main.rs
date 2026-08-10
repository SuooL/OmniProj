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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_projects])
        .run(tauri::generate_context!())
        .expect("error while running omniproj desktop");
}
