//! MVP Record / Advance helpers. These operate only on OmniProj's local store and read
//! repository Git history; the source repository is never modified.
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use omniproj_capture::git::commit_log;
use omniproj_core::ids::ProjectId;
use omniproj_core::project_state::{ProjectStateDoc, WorkItemStatus};
use omniproj_core::{content_hash, load_project, NextDoc, TaskStatus};

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
    pub adopted_from_proposal_id: Option<String>,
    pub linked_work_item_id: Option<String>,
    pub is_current_commitment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListDto {
    pub revision: String,
    pub tasks: Vec<TaskDto>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvanceProposalDto {
    pub proposal_id: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReentryEventDto {
    pub project_id: ProjectId,
    pub occurred_at: String,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DogfoodSummaryDto {
    pub event_count: usize,
    pub project_count: usize,
    pub median_duration_seconds: Option<u64>,
    pub meets_event_threshold: bool,
    pub meets_project_threshold: bool,
}

fn dogfood_events_path() -> PathBuf {
    omniproj_core::omniproj_home().join("dogfood/reentry-events.jsonl")
}

fn load_reentry_events() -> CommandResult<Vec<ReentryEventDto>> {
    let raw = read_optional(&dogfood_events_path())?;
    String::from_utf8_lossy(&raw)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| {
                CommandError::new(
                    crate::error::ErrorCode::StoreReadFailed,
                    format!("invalid dogfood event: {error}"),
                )
            })
        })
        .collect()
}

pub fn dogfood_summary() -> CommandResult<DogfoodSummaryDto> {
    let events = load_reentry_events()?;
    let project_count = events
        .iter()
        .map(|event| event.project_id.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let mut durations = events
        .iter()
        .map(|event| event.duration_seconds)
        .collect::<Vec<_>>();
    durations.sort_unstable();
    let median_duration_seconds = match durations.len() {
        0 => None,
        len if len % 2 == 1 => durations.get(len / 2).copied(),
        len => Some((durations[len / 2 - 1] + durations[len / 2]) / 2),
    };
    Ok(DogfoodSummaryDto {
        event_count: events.len(),
        project_count,
        median_duration_seconds,
        meets_event_threshold: events.len() >= 20,
        meets_project_threshold: project_count >= 5,
    })
}

pub fn record_reentry_event(
    project_id: ProjectId,
    duration_seconds: u64,
) -> CommandResult<DogfoodSummaryDto> {
    load_project(&project_id)?;
    if duration_seconds == 0 || duration_seconds > 86_400 {
        return Err(CommandError::invalid_input(
            "re-entry duration must be between 1 second and 24 hours",
        ));
    }
    let path = dogfood_events_path();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        let current = read_optional(&path)?;
        let event = ReentryEventDto {
            project_id,
            occurred_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            duration_seconds,
        };
        let mut next = current;
        next.extend_from_slice(
            serde_json::to_string(&event)
                .map_err(|error| {
                    CommandError::new(crate::error::ErrorCode::StoreWriteFailed, error.to_string())
                })?
                .as_bytes(),
        );
        next.push(b'\n');
        omniproj_core::atomic_write(&path, &next)?;
        omniproj_core::commit_paths_checked(
            "dogfood: record re-entry event",
            &[PathBuf::from("dogfood/reentry-events.jsonl")],
        )?;
        Ok(())
    })?;
    dogfood_summary()
}

fn read_optional(path: &Path) -> CommandResult<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(CommandError::new(
            crate::error::ErrorCode::StoreReadFailed,
            error.to_string(),
        )
        .retryable()),
    }
}

fn revision_for(bytes: &[u8]) -> String {
    content_hash(&String::from_utf8_lossy(bytes))
}

fn revision_conflict(expected: &str, actual: &str) -> CommandError {
    CommandError::new(
        crate::error::ErrorCode::RevisionConflict,
        format!("document changed: expected revision {expected}, found {actual}"),
    )
}

fn write_checked_document(
    path: &Path,
    relative_path: PathBuf,
    expected_revision: &str,
    contents: &[u8],
    message: &str,
) -> CommandResult<String> {
    let path = path.to_owned();
    let expected_revision = expected_revision.to_owned();
    let contents = contents.to_vec();
    omniproj_core::ensure_home_then_write(|| -> CommandResult<String> {
        let current = read_optional(&path)?;
        let actual_revision = revision_for(&current);
        if actual_revision != expected_revision {
            return Err(revision_conflict(&expected_revision, &actual_revision));
        }
        omniproj_core::atomic_write(&path, &contents)?;
        omniproj_core::commit_paths_checked(message, &[relative_path])?;
        Ok(revision_for(&contents))
    })
}

pub fn attention_summary_with_threshold(days: u32) -> AttentionSummaryDto {
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    let project_ids = omniproj_core::list_project_records()
        .ok()
        .map(|records| {
            records
                .into_iter()
                .filter_map(|record| {
                    let state = ProjectStateDoc::load(&record.id).ok()?;
                    if state.status != omniproj_core::project_state::ProjectStatus::Active {
                        return None;
                    }
                    let source = record.primary_git_source()?;
                    if source.status != omniproj_core::project::ProjectSourceStatus::Available {
                        return None;
                    }
                    let blocked = state
                        .current_next_action_id
                        .as_ref()
                        .and_then(|id| state.work_items.iter().find(|item| &item.id == id))
                        .is_some_and(|item| item.status == WorkItemStatus::Blocked);
                    let latest = commit_log(Path::new(&source.location), 1)
                        .first()
                        .and_then(|commit| {
                            chrono::DateTime::parse_from_rfc3339(&commit.committed_at).ok()
                        })
                        .map(|dt| dt.with_timezone(&Utc));
                    (blocked || latest.is_some_and(|at| at < cutoff)).then_some(record.id)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    AttentionSummaryDto {
        count: project_ids.len(),
        project_ids,
    }
}

pub fn attention_count_with_threshold(days: u32) -> usize {
    attention_summary_with_threshold(days).count
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
pub struct PlanListDto {
    pub revision: String,
    pub entries: Vec<PlanEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReminderSettingsDto {
    pub enabled: bool,
    pub cadence: String,
    pub silent_days_threshold: u32,
    #[serde(default)]
    pub revision: String,
}

impl Default for ReminderSettingsDto {
    fn default() -> Self {
        Self {
            enabled: true,
            cadence: "daily".into(),
            silent_days_threshold: 7,
            revision: revision_for(&[]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedReminderSettings {
    enabled: bool,
    cadence: String,
    silent_days_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReminderDeliveryState {
    local_date: String,
    attention_signature: String,
}

fn settings_path() -> std::path::PathBuf {
    omniproj_core::omniproj_home().join("desktop.toml")
}

fn reminder_delivery_path() -> PathBuf {
    omniproj_core::omniproj_home().join("cache/reminder-delivery.toml")
}

/// Claim one daily delivery for the current local date and attention set. Reopening the
/// app cannot repeat the same notification; a genuinely changed set may notify again.
pub fn claim_daily_reminder(
    settings: &ReminderSettingsDto,
    summary: &AttentionSummaryDto,
) -> CommandResult<bool> {
    if !settings.enabled || settings.cadence != "daily" || summary.count == 0 {
        return Ok(false);
    }
    let local_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let signature = content_hash(
        &summary
            .project_ids
            .iter()
            .map(ProjectId::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let path = reminder_delivery_path();
    let current = read_optional(&path)?;
    let previous = toml::from_str::<ReminderDeliveryState>(&String::from_utf8_lossy(&current))
        .unwrap_or_default();
    if previous.local_date == local_date && previous.attention_signature == signature {
        return Ok(false);
    }
    let body = toml::to_string_pretty(&ReminderDeliveryState {
        local_date,
        attention_signature: signature,
    })
    .map_err(|error| {
        CommandError::new(crate::error::ErrorCode::StoreWriteFailed, error.to_string()).retryable()
    })?;
    omniproj_core::atomic_write(&path, body.as_bytes())?;
    Ok(true)
}

pub fn load_reminder_settings() -> ReminderSettingsDto {
    let raw = read_optional(&settings_path()).unwrap_or_default();
    let revision = revision_for(&raw);
    if raw.is_empty() {
        return ReminderSettingsDto {
            revision,
            ..ReminderSettingsDto::default()
        };
    }
    toml::from_str::<PersistedReminderSettings>(&String::from_utf8_lossy(&raw))
        .ok()
        .map(|mut s| {
            if s.cadence != "daily" && s.cadence != "off" {
                s.cadence = "daily".into();
            }
            ReminderSettingsDto {
                enabled: s.enabled,
                cadence: s.cadence,
                silent_days_threshold: s.silent_days_threshold,
                revision,
            }
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
    let persisted = PersistedReminderSettings {
        enabled: settings.enabled,
        cadence: settings.cadence.clone(),
        silent_days_threshold: settings.silent_days_threshold,
    };
    let body = toml::to_string_pretty(&persisted).map_err(|e| {
        CommandError::new(crate::error::ErrorCode::StoreWriteFailed, e.to_string()).retryable()
    })?;
    let expected = settings.revision.clone();
    let revision = write_checked_document(
        &settings_path(),
        PathBuf::from("desktop.toml"),
        &expected,
        body.as_bytes(),
        "settings: update reminders",
    )?;
    settings.revision = revision;
    Ok(settings)
}

fn plan_list(raw: &[u8]) -> PlanListDto {
    let doc = omniproj_core::PlanDoc::parse(&String::from_utf8_lossy(raw));
    PlanListDto {
        revision: revision_for(raw),
        entries: doc
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
            .collect(),
    }
}

pub fn get_plan(project_id: ProjectId) -> CommandResult<PlanListDto> {
    load_project(&project_id)?;
    let raw = read_optional(&omniproj_core::plan_path(project_id.as_str()))?;
    Ok(plan_list(&raw))
}

pub fn add_plan_entry(
    project_id: ProjectId,
    expected_revision: String,
    title: String,
    body: String,
) -> CommandResult<PlanListDto> {
    if title.trim().is_empty() {
        return Err(CommandError::invalid_input("decision title is required"));
    }
    mutate_plan(
        &project_id,
        &expected_revision,
        "decision: add",
        move |doc| {
            doc.add(&Utc::now().format("%Y-%m-%d").to_string(), &title, &body);
            Ok(())
        },
    )
}

pub fn set_plan_status(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
    status: String,
) -> CommandResult<PlanListDto> {
    let parsed = omniproj_core::PlanStatus::parse(&status)
        .ok_or_else(|| CommandError::invalid_input("invalid decision status"))?;
    mutate_plan(
        &project_id,
        &expected_revision,
        "decision: update status",
        move |doc| {
            if !doc.set_status(&id, parsed) {
                return Err(CommandError::invalid_input("decision not found"));
            }
            Ok(())
        },
    )
}

pub fn set_plan_commit(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
    commit: Option<String>,
) -> CommandResult<PlanListDto> {
    mutate_plan(
        &project_id,
        &expected_revision,
        "decision: link commit",
        move |doc| {
            if !doc.set_commit(&id, commit) {
                return Err(CommandError::invalid_input(
                    "invalid decision or commit SHA",
                ));
            }
            Ok(())
        },
    )
}

fn mutate_plan(
    project_id: &ProjectId,
    expected_revision: &str,
    message: &str,
    f: impl FnOnce(&mut omniproj_core::PlanDoc) -> CommandResult<()>,
) -> CommandResult<PlanListDto> {
    load_project(project_id)?;
    let path = omniproj_core::plan_path(project_id.as_str());
    let raw = read_optional(&path)?;
    let actual_revision = revision_for(&raw);
    if actual_revision != expected_revision {
        return Err(revision_conflict(expected_revision, &actual_revision));
    }
    let mut doc = omniproj_core::PlanDoc::parse(&String::from_utf8_lossy(&raw));
    f(&mut doc)?;
    let rendered = doc.render();
    let relative = PathBuf::from(format!("projects/{}/plan.md", project_id.as_str()));
    write_checked_document(
        &path,
        relative,
        expected_revision,
        rendered.as_bytes(),
        message,
    )?;
    Ok(plan_list(rendered.as_bytes()))
}

fn task_list(project_id: &ProjectId, raw: &[u8]) -> CommandResult<TaskListDto> {
    let state = ProjectStateDoc::load(project_id)?;
    let doc = NextDoc::parse(&String::from_utf8_lossy(raw));
    let tasks = doc
        .items()
        .filter_map(|item| {
            item.id.as_ref().map(|id| {
                let linked = state
                    .work_items
                    .iter()
                    .rev()
                    .find(|work_item| work_item.source_task_id.as_deref() == Some(id.as_str()));
                let is_current = linked.is_some_and(|work_item| {
                    state.current_next_action_id.as_ref() == Some(&work_item.id)
                });
                let status = linked.map_or_else(
                    || item.status.as_str(),
                    |work_item| match work_item.status {
                        WorkItemStatus::Planned => "open",
                        WorkItemStatus::Doing | WorkItemStatus::Blocked => "doing",
                        WorkItemStatus::Done | WorkItemStatus::Abandoned => "done",
                    },
                );
                TaskDto {
                    id: id.clone(),
                    text: item.text.clone(),
                    status: status.to_owned(),
                    unclear: item.unclear,
                    due: item.due.clone(),
                    note: item.note.clone(),
                    commits: item.commits.clone(),
                    adopted_from_proposal_id: item.adopted_from_proposal_id.clone(),
                    linked_work_item_id: linked.map(|work_item| work_item.id.as_str().to_owned()),
                    is_current_commitment: is_current,
                }
            })
        })
        .collect();
    Ok(TaskListDto {
        revision: revision_for(raw),
        tasks,
    })
}

pub fn get_tasks(project_id: ProjectId) -> CommandResult<TaskListDto> {
    load_project(&project_id)?;
    let raw = read_optional(&omniproj_core::next_path(project_id.as_str()))?;
    task_list(&project_id, &raw)
}

fn mutate(
    project_id: &ProjectId,
    expected_revision: &str,
    message: &str,
    f: impl FnOnce(&mut NextDoc) -> Result<(), String>,
) -> CommandResult<TaskListDto> {
    load_project(project_id)?;
    let path = omniproj_core::next_path(project_id.as_str());
    let raw = read_optional(&path)?;
    let actual_revision = revision_for(&raw);
    if actual_revision != expected_revision {
        return Err(revision_conflict(expected_revision, &actual_revision));
    }
    let mut doc = NextDoc::parse(&String::from_utf8_lossy(&raw));
    f(&mut doc).map_err(CommandError::invalid_input)?;
    let rendered = doc.render();
    let relative = PathBuf::from(format!("projects/{}/notes/next.md", project_id.as_str()));
    write_checked_document(
        &path,
        relative,
        expected_revision,
        rendered.as_bytes(),
        message,
    )?;
    task_list(project_id, rendered.as_bytes())
}

pub fn add_task(
    project_id: ProjectId,
    expected_revision: String,
    text: String,
    unclear: bool,
) -> CommandResult<TaskListDto> {
    if text.trim().is_empty() {
        return Err(CommandError::invalid_input("task text is required"));
    }
    mutate(&project_id, &expected_revision, "task: add", |doc| {
        doc.add(&text, unclear);
        Ok(())
    })
}

pub fn update_task(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
    status: String,
    due: Option<String>,
    note: Option<String>,
) -> CommandResult<TaskListDto> {
    let mut parsed = TaskStatus::parse(&status)
        .ok_or_else(|| CommandError::invalid_input("invalid task status"))?;
    let state = ProjectStateDoc::load(&project_id)?;
    if let Some(work_item) = state
        .work_items
        .iter()
        .rev()
        .find(|work_item| work_item.source_task_id.as_deref() == Some(id.as_str()))
    {
        parsed = match work_item.status {
            WorkItemStatus::Planned => TaskStatus::Open,
            WorkItemStatus::Doing | WorkItemStatus::Blocked => TaskStatus::Doing,
            WorkItemStatus::Done | WorkItemStatus::Abandoned => TaskStatus::Done,
        };
    }
    mutate(
        &project_id,
        &expected_revision,
        "task: update",
        move |doc| {
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
        },
    )
}

pub fn remove_task(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
) -> CommandResult<TaskListDto> {
    let state = ProjectStateDoc::load(&project_id)?;
    if state
        .work_items
        .iter()
        .any(|work_item| work_item.source_task_id.as_deref() == Some(id.as_str()))
    {
        return Err(CommandError::invalid_input(
            "a task linked to commitment history cannot be removed",
        ));
    }
    mutate(
        &project_id,
        &expected_revision,
        "task: remove",
        move |doc| {
            if doc.remove(&id) {
                Ok(())
            } else {
                Err("task not found".into())
            }
        },
    )
}

pub fn attribute_commit(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
    sha: String,
) -> CommandResult<TaskListDto> {
    mutate(
        &project_id,
        &expected_revision,
        "task: attribute commit",
        move |doc| {
            if doc.attribute_commit(&id, &sha) {
                Ok(())
            } else {
                Err("invalid task or commit SHA".into())
            }
        },
    )
}

pub fn unattribute_commit(
    project_id: ProjectId,
    expected_revision: String,
    id: String,
    sha: String,
) -> CommandResult<TaskListDto> {
    mutate(
        &project_id,
        &expected_revision,
        "task: unattribute commit",
        move |doc| {
            if doc.unattribute_commit(&id, &sha) {
                Ok(())
            } else {
                Err("commit attribution not found".into())
            }
        },
    )
}

pub fn get_timeline(project_id: ProjectId, limit: usize) -> CommandResult<Vec<TimelineCommitDto>> {
    let record = load_project(&project_id)?;
    let source = record.primary_git_source().ok_or_else(|| {
        CommandError::new(crate::error::ErrorCode::SourceMissing, "no Git source")
    })?;
    let current_tasks = get_tasks(project_id.clone())?.tasks;
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

pub async fn advance_task(project_id: ProjectId, id: String) -> CommandResult<AdvanceProposalDto> {
    let raw = read_optional(&omniproj_core::next_path(project_id.as_str()))?;
    let item = NextDoc::parse(&String::from_utf8_lossy(&raw))
        .items()
        .find(|t| t.id.as_deref() == Some(id.as_str()))
        .cloned()
        .ok_or_else(|| CommandError::invalid_input("task not found"))?;
    let resolved = crate::agent_settings::resolve_provider()?;
    let mut steps =
        omniproj_distill::breakdown(&item.text, item.note.as_deref(), &resolved.provider)
            .await
            .map_err(|e| {
                CommandError::new(
                    crate::error::ErrorCode::SourceObservationFailed,
                    e.to_string(),
                )
                .retryable()
            })?;
    if !valid_advance_candidates(&steps) {
        steps = omniproj_distill::breakdown(&item.text, item.note.as_deref(), &resolved.provider)
            .await
            .map_err(|e| {
                CommandError::new(
                    crate::error::ErrorCode::SourceObservationFailed,
                    e.to_string(),
                )
                .retryable()
            })?;
    }
    if !valid_advance_candidates(&steps) {
        return Err(CommandError::new(
            crate::error::ErrorCode::SourceObservationFailed,
            format!(
                "Agent returned {} usable candidates; expected 3–6. Nothing was saved.",
                steps.len()
            ),
        )
        .retryable());
    }
    let proposal_id = format!("{}-{}", id, Utc::now().timestamp_millis());
    let body = format!(
        "# Advance proposal {proposal_id} — #{id}: {}\n\n{}\n",
        item.text,
        steps
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let path = omniproj_core::auto_dir(project_id.as_str())
        .join("advance")
        .join(format!("{proposal_id}.md"));
    let relative = PathBuf::from(format!(
        "projects/{}/auto/advance/{proposal_id}.md",
        project_id.as_str()
    ));
    write_checked_document(
        &path,
        relative,
        &revision_for(&[]),
        body.as_bytes(),
        &format!("advance: propose from task #{id}"),
    )?;
    Ok(AdvanceProposalDto {
        proposal_id,
        candidates: steps,
    })
}

fn valid_advance_candidates(steps: &[String]) -> bool {
    (3..=6).contains(&steps.len()) && steps.iter().all(|step| !step.trim().is_empty())
}

pub fn adopt_subtasks(
    project_id: ProjectId,
    expected_revision: String,
    proposal_id: String,
    texts: Vec<String>,
) -> CommandResult<TaskListDto> {
    if proposal_id.trim().is_empty() {
        return Err(CommandError::invalid_input("proposal id is required"));
    }
    mutate(
        &project_id,
        &expected_revision,
        "advance: adopt selected subtasks",
        move |doc| {
            for text in texts.iter().filter(|t| !t.trim().is_empty()) {
                doc.add_adopted(text, &proposal_id);
            }
            Ok(())
        },
    )
}

pub fn task_for_commitment(
    project_id: &ProjectId,
    task_id: &str,
    expected_task_revision: &str,
) -> CommandResult<TaskDto> {
    let list = get_tasks(project_id.clone())?;
    if list.revision != expected_task_revision {
        return Err(revision_conflict(expected_task_revision, &list.revision));
    }
    let task = list
        .tasks
        .into_iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| CommandError::invalid_input("task not found"))?;
    if task.linked_work_item_id.is_some() {
        return Err(CommandError::invalid_input(
            "task is already linked to commitment history",
        ));
    }
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

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
            revision: revision_for(&[]),
        };
        let error = save_reminder_settings(settings).expect_err("unknown cadence must be rejected");
        assert_eq!(error.code, crate::error::ErrorCode::InvalidInput);
    }

    #[test]
    fn daily_reminder_is_deduplicated_for_the_same_attention_set() {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let home =
            std::env::temp_dir().join(format!("omniproj-reminder-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        let settings = ReminderSettingsDto::default();
        let summary = AttentionSummaryDto {
            count: 1,
            project_ids: vec![ProjectId::parse("project-reminder").unwrap()],
        };
        assert!(claim_daily_reminder(&settings, &summary).unwrap());
        assert!(!claim_daily_reminder(&settings, &summary).unwrap());
        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn advance_requires_three_to_six_non_empty_candidates() {
        assert!(!valid_advance_candidates(&["one".into(), "two".into()]));
        assert!(valid_advance_candidates(&[
            "one".into(),
            "two".into(),
            "three".into(),
        ]));
        assert!(!valid_advance_candidates(&[
            "one".into(),
            "two".into(),
            " ".into(),
        ]));
        assert!(!valid_advance_candidates(&vec!["step".into(); 7]));
    }
}
