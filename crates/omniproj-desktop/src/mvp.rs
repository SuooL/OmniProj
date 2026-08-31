//! MVP Record / Advance helpers. These operate only on OmniProj's local store and read
//! repository Git history; the source repository is never modified.
#![allow(deprecated)]

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use omniproj_capture::git::commit_log;
use omniproj_core::ids::ProjectId;
use omniproj_core::{load_project, NextDoc, TaskStatus};

use crate::error::{CommandError, CommandResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDto {
    pub id: String,
    pub text: String,
    pub status: String,
    pub unclear: bool,
    pub due: Option<String>,
    pub note: Option<String>,
    pub commits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCommitDto {
    pub sha: String,
    pub short_sha: String,
    pub committed_at: String,
    pub author: String,
    pub subject: String,
    pub attributed_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCommitDto {
    pub sha: String,
    pub short_sha: String,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub committed_at: String,
    pub author: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionSummaryDto {
    pub count: usize,
    pub project_ids: Vec<ProjectId>,
}

pub fn attention_count_with_threshold(days: u32) -> usize {
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    omniproj_core::list_project_records()
        .ok()
        .map(|records| {
            records
                .into_iter()
                .filter(|record| {
                    let Some(source) = record.primary_git_source() else {
                        return false;
                    };
                    let latest = commit_log(Path::new(&source.location), 1)
                        .first()
                        .and_then(|commit| {
                            chrono::DateTime::parse_from_rfc3339(&commit.committed_at).ok()
                        })
                        .map(|dt| dt.with_timezone(&Utc));
                    latest.map(|at| at < cutoff).unwrap_or(true)
                })
                .count()
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanEntryDto {
    pub id: Option<String>,
    pub date: String,
    pub title: String,
    pub status: String,
    pub commit: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReminderSettingsDto {
    pub enabled: bool,
    pub cadence: String,
    pub silent_days_threshold: u32,
}

impl Default for ReminderSettingsDto {
    fn default() -> Self {
        Self {
            enabled: true,
            cadence: "daily".into(),
            silent_days_threshold: 7,
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    omniproj_core::omniproj_home().join("desktop.toml")
}

pub fn load_reminder_settings() -> ReminderSettingsDto {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|raw| toml::from_str::<ReminderSettingsDto>(&raw).ok())
        .map(|mut s| {
            if s.cadence != "daily" && s.cadence != "off" {
                s.cadence = "daily".into();
            }
            s
        })
        .unwrap_or_default()
}

pub fn save_reminder_settings(
    mut settings: ReminderSettingsDto,
) -> CommandResult<ReminderSettingsDto> {
    if settings.cadence != "daily" && settings.cadence != "off" {
        return Err(CommandError::invalid_input("cadence must be daily or off"));
    }
    settings.silent_days_threshold = settings.silent_days_threshold.min(3650);
    omniproj_core::ensure_home().map_err(CommandError::from)?;
    let body = toml::to_string_pretty(&settings).map_err(|e| {
        CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
    })?;
    std::fs::write(settings_path(), body).map_err(|e| {
        CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
    })?;
    Ok(settings)
}

pub fn get_plan(project_id: ProjectId) -> CommandResult<Vec<PlanEntryDto>> {
    Ok(omniproj_core::PlanDoc::load(project_id.as_str())
        .entries()
        .iter()
        .map(|e| PlanEntryDto {
            id: e.id.clone(),
            date: e.date.clone(),
            title: e.title.clone(),
            status: e.status.as_str().into(),
            commit: e.commit.clone(),
            body: e.body.clone(),
        })
        .collect())
}

pub fn add_plan_entry(
    project_id: ProjectId,
    title: String,
    body: String,
) -> CommandResult<Vec<PlanEntryDto>> {
    if title.trim().is_empty() {
        return Err(CommandError::invalid_input("decision title is required"));
    }
    let hash = project_id.as_str().to_owned();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        let mut doc = omniproj_core::PlanDoc::load(&hash);
        doc.add(&Utc::now().format("%Y-%m-%d").to_string(), &title, &body);
        doc.save(&hash).map_err(|e| {
            CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
        })?;
        omniproj_core::commit_all("decision add");
        Ok(())
    })?;
    get_plan(project_id)
}

pub fn set_plan_status(
    project_id: ProjectId,
    id: String,
    status: String,
) -> CommandResult<Vec<PlanEntryDto>> {
    let parsed = omniproj_core::PlanStatus::parse(&status)
        .ok_or_else(|| CommandError::invalid_input("invalid decision status"))?;
    let hash = project_id.as_str().to_owned();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        let mut doc = omniproj_core::PlanDoc::load(&hash);
        if !doc.set_status(&id, parsed) {
            return Err(CommandError::invalid_input("decision not found"));
        }
        doc.save(&hash).map_err(|e| {
            CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
        })?;
        omniproj_core::commit_all("decision status");
        Ok(())
    })?;
    get_plan(project_id)
}

fn tasks(project_id: &ProjectId) -> Vec<TaskDto> {
    NextDoc::load(project_id.as_str())
        .items()
        .filter_map(|item| {
            item.id.as_ref().map(|id| TaskDto {
                id: id.clone(),
                text: item.text.clone(),
                status: item.status.as_str().to_owned(),
                unclear: item.unclear,
                due: item.due.clone(),
                note: item.note.clone(),
                commits: item.commits.clone(),
            })
        })
        .collect()
}

pub fn get_tasks(project_id: ProjectId) -> CommandResult<Vec<TaskDto>> {
    Ok(tasks(&project_id))
}

fn mutate(
    project_id: &ProjectId,
    message: &str,
    f: impl FnOnce(&mut NextDoc) -> Result<(), String>,
) -> CommandResult<Vec<TaskDto>> {
    let id = project_id.clone();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        let mut doc = NextDoc::load(id.as_str());
        f(&mut doc).map_err(CommandError::invalid_input)?;
        doc.save(id.as_str()).map_err(|e| {
            CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
        })?;
        omniproj_core::commit_all(message);
        Ok(())
    })?;
    Ok(tasks(project_id))
}

pub fn add_task(project_id: ProjectId, text: String, unclear: bool) -> CommandResult<Vec<TaskDto>> {
    if text.trim().is_empty() {
        return Err(CommandError::invalid_input("task text is required"));
    }
    mutate(&project_id, "task add", |doc| {
        doc.add(&text, unclear);
        Ok(())
    })
}

pub fn update_task(
    project_id: ProjectId,
    id: String,
    status: String,
    due: Option<String>,
    note: Option<String>,
) -> CommandResult<Vec<TaskDto>> {
    let parsed = TaskStatus::parse(&status)
        .ok_or_else(|| CommandError::invalid_input("invalid task status"))?;
    mutate(&project_id, "task update", move |doc| {
        if !doc.set_status(&id, parsed) {
            return Err("task not found".into());
        }
        if !doc.set_due(&id, due) {
            return Err("invalid due date or task not found".into());
        }
        if !doc.set_note(&id, note) {
            return Err("task not found".into());
        }
        Ok(())
    })
}

pub fn remove_task(project_id: ProjectId, id: String) -> CommandResult<Vec<TaskDto>> {
    mutate(&project_id, "task remove", move |doc| {
        if doc.remove(&id) {
            Ok(())
        } else {
            Err("task not found".into())
        }
    })
}

pub fn attribute_commit(
    project_id: ProjectId,
    id: String,
    sha: String,
) -> CommandResult<Vec<TaskDto>> {
    mutate(&project_id, "task attribute commit", move |doc| {
        if doc.attribute_commit(&id, &sha) {
            Ok(())
        } else {
            Err("invalid task or commit SHA".into())
        }
    })
}

pub fn unattribute_commit(
    project_id: ProjectId,
    id: String,
    sha: String,
) -> CommandResult<Vec<TaskDto>> {
    mutate(&project_id, "task unattribute commit", move |doc| {
        if doc.unattribute_commit(&id, &sha) {
            Ok(())
        } else {
            Err("commit attribution not found".into())
        }
    })
}

pub fn get_timeline(project_id: ProjectId, limit: usize) -> CommandResult<Vec<TimelineCommitDto>> {
    let record = load_project(&project_id)?;
    let source = record.primary_git_source().ok_or_else(|| {
        CommandError::new(crate::error::ErrorCode::SourceMissing, "no Git source")
    })?;
    let current_tasks = tasks(&project_id);
    Ok(commit_log(Path::new(&source.location), limit.min(200))
        .into_iter()
        .map(|commit| TimelineCommitDto {
            attributed_task_ids: current_tasks
                .iter()
                .filter(|t| {
                    t.commits.iter().any(|s| {
                        s.eq_ignore_ascii_case(&commit.hash)
                            || s.eq_ignore_ascii_case(&commit.short)
                    })
                })
                .map(|t| t.id.clone())
                .collect(),
            sha: commit.hash,
            short_sha: commit.short,
            committed_at: commit.committed_at,
            author: commit.author,
            subject: commit.subject,
        })
        .collect())
}

pub fn get_graph(project_id: ProjectId, limit: usize) -> CommandResult<Vec<GraphCommitDto>> {
    let record = load_project(&project_id)?;
    let source = record.primary_git_source().ok_or_else(|| {
        CommandError::new(crate::error::ErrorCode::SourceMissing, "no Git source")
    })?;
    Ok(
        omniproj_capture::git::commit_graph(Path::new(&source.location), limit.min(200))
            .into_iter()
            .map(|c| GraphCommitDto {
                sha: c.hash,
                short_sha: c.short,
                parents: c.parents,
                refs: c.refs,
                committed_at: c.date,
                author: c.author,
                subject: c.subject,
            })
            .collect(),
    )
}

pub async fn advance_task(project_id: ProjectId, id: String) -> CommandResult<Vec<String>> {
    let item = NextDoc::load(project_id.as_str())
        .items()
        .find(|t| t.id.as_deref() == Some(id.as_str()))
        .cloned()
        .ok_or_else(|| CommandError::invalid_input("task not found"))?;
    let resolved = omniproj_distill::resolve(None).map_err(|e| {
        CommandError::new(
            crate::error::ErrorCode::InvalidInput,
            format!("no LLM provider configured: {e}"),
        )
    })?;
    let steps = omniproj_distill::breakdown(&item.text, item.note.as_deref(), &resolved.provider)
        .await
        .map_err(|e| {
            CommandError::new(
                crate::error::ErrorCode::SourceObservationFailed,
                e.to_string(),
            )
            .retryable()
        })?;
    let body = format!(
        "# Advance proposal — #{id}: {}\n\n{}\n",
        item.text,
        steps
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let hash = project_id.as_str().to_owned();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        let dir = omniproj_core::auto_dir(&hash).join("advance");
        std::fs::create_dir_all(&dir).map_err(|e| {
            CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
        })?;
        std::fs::write(dir.join(format!("{id}.md")), body).map_err(|e| {
            CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
        })?;
        omniproj_core::commit_all(&format!("advance proposal #{id}"));
        Ok(())
    })?;
    Ok(steps)
}

pub fn adopt_subtasks(project_id: ProjectId, texts: Vec<String>) -> CommandResult<Vec<TaskDto>> {
    mutate(&project_id, "adopt advance subtasks", move |doc| {
        for text in texts.iter().filter(|t| !t.trim().is_empty()) {
            doc.add(text, false);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_settings_default_is_daily_and_bounded() {
        let settings = ReminderSettingsDto::default();
        assert!(settings.enabled);
        assert_eq!(settings.cadence, "daily");
        assert_eq!(settings.silent_days_threshold, 7);
    }

    #[test]
    fn reminder_settings_reject_unknown_cadence() {
        let settings = ReminderSettingsDto {
            enabled: true,
            cadence: "weekly".into(),
            silent_days_threshold: 7,
        };
        let error = save_reminder_settings(settings).expect_err("unknown cadence must be rejected");
        assert_eq!(error.code, crate::error::ErrorCode::InvalidInput);
    }
}
