//! The R0 desktop service: the typed application boundary between the Tauri IPC layer
//! and the `omniproj-core` domain. It owns the clock seam and the in-flight refresh
//! state, assembles DTOs, and enforces the R0 rules the UI depends on:
//!
//! - source Git inspection always runs on a blocking pool (`spawn_blocking`);
//! - a failed observation preserves the last cached facts;
//! - a partial multi-project refresh returns one result per project;
//! - `store_write_failed` and `audit_commit_failed` stay distinct.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use omniproj_capture::git::{count_commits_since, observe_repository, RepositoryObservation};
use omniproj_core::ids::ProjectId;
use omniproj_core::project::{
    canonical_source_owner, list_project_records, load_project, record_source_observation,
    register_project, relink_primary_git_source, ProjectRecord, ProjectSource, ProjectSourceStatus,
    RecordSourceObservationInput, RegisterOutcome,
    RegisterProjectInput as CoreRegisterProjectInput, RelinkSourceInput, SourceObservationOutcome,
};
use omniproj_core::project_state::{
    apply_project_command, ProjectCommand, ProjectStateDoc, ProjectStatus,
};
use omniproj_core::review::{derive_review_reasons, ReviewReason, DEFAULT_COMMITMENT_REVIEW_DAYS};

use crate::dto::{
    assemble_index_item, assemble_overview, index_sort_key, CommitDto, CompleteProjectSetupInput,
    MutationCommand, ObservedActualDto, ProjectIndexItemDto, ProjectIndexResponseDto,
    ProjectMutationInput, ProjectOverviewDto, RefreshOutcome, RefreshResultDto,
    RegisterProjectInput, RelinkProjectInput, ReviewPolicyDto, SourceValidationDto,
};
use crate::error::{CommandError, CommandResult, ErrorCode};
use crate::repository_cache::{self, head_state_dto, CachedObservation};
use crate::state::DesktopState;

/// The time seam. Production uses the system clock; tests inject a fixed instant so
/// review-due and observation timestamps are deterministic.
pub trait Clock: Send + Sync {
    fn now_rfc3339(&self) -> String;
}

/// The system clock: UTC, RFC3339, second precision.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }
}

/// The R0 application service.
#[derive(Debug, Default)]
pub struct DesktopService<C: Clock> {
    pub clock: C,
    pub state: DesktopState,
}

/// The typed R0 service surface. Reads are synchronous; anything that inspects a source
/// repository is async so the Git work can run on a blocking pool.
#[allow(async_fn_in_trait)]
pub trait R0Service {
    fn list_project_index(&self) -> CommandResult<ProjectIndexResponseDto>;
    fn get_project_overview(&self, project_id: ProjectId) -> CommandResult<ProjectOverviewDto>;
    async fn validate_project_source(&self, location: String)
        -> CommandResult<SourceValidationDto>;
    async fn register_project(
        &self,
        input: RegisterProjectInput,
    ) -> CommandResult<ProjectOverviewDto>;
    async fn relink_project_source(
        &self,
        input: RelinkProjectInput,
    ) -> CommandResult<ProjectOverviewDto>;
    async fn refresh_projects(
        &self,
        project_ids: Option<Vec<ProjectId>>,
    ) -> CommandResult<Vec<RefreshResultDto>>;
    fn apply_project_mutation(
        &self,
        input: ProjectMutationInput,
    ) -> CommandResult<ProjectOverviewDto>;
}

impl<C: Clock> DesktopService<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            state: DesktopState::new(),
        }
    }

    /// The current instant as a validated `DateTime<Utc>`.
    fn now(&self) -> CommandResult<DateTime<Utc>> {
        let raw = self.clock.now_rfc3339();
        DateTime::parse_from_rfc3339(&raw)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| {
                CommandError::new(
                    ErrorCode::StoreReadFailed,
                    format!("clock produced non-RFC3339 time {raw:?}"),
                )
            })
    }

    /// The neutral view parts shared by Index rows and Overview: primary source, review
    /// reasons, and the observed-actual rebuilt from the last-successful cache.
    fn view_parts<'a>(
        &self,
        record: &'a ProjectRecord,
        state: &ProjectStateDoc,
        now: DateTime<Utc>,
    ) -> (
        Option<&'a ProjectSource>,
        Vec<ReviewReason>,
        Option<ObservedActualDto>,
    ) {
        let source = record.primary_git_source();
        let reasons = match source {
            Some(source) => {
                derive_review_reasons(state, source, now, DEFAULT_COMMITMENT_REVIEW_DAYS)
            }
            None => Vec::new(),
        };
        let observed_actual = source
            .and_then(|source| repository_cache::load(&record.id, &source.id))
            .map(|cache| cache.to_observed_actual(state.current_next_action_id.as_ref()));
        (source, reasons, observed_actual)
    }

    /// Assemble one Index row for a loaded record.
    fn index_item(
        &self,
        record: &ProjectRecord,
        state: &ProjectStateDoc,
        now: DateTime<Utc>,
    ) -> ProjectIndexItemDto {
        let (source, reasons, observed_actual) = self.view_parts(record, state, now);
        assemble_index_item(record, state, source, &reasons, observed_actual)
    }

    /// Assemble the full Overview for a loaded record.
    fn overview(
        &self,
        record: &ProjectRecord,
        state: &ProjectStateDoc,
        now: DateTime<Utc>,
    ) -> ProjectOverviewDto {
        let (source, reasons, observed_actual) = self.view_parts(record, state, now);
        assemble_overview(record, state, source, &reasons, observed_actual)
    }

    /// Load record + state and build the Overview at the current time.
    fn load_overview(&self, project_id: &ProjectId) -> CommandResult<ProjectOverviewDto> {
        let record = load_project(project_id)?;
        let state = ProjectStateDoc::load(project_id)?;
        let now = self.now()?;
        Ok(self.overview(&record, &state, now))
    }

    /// Apply one core project command and return the resulting Overview directly.
    fn apply_core(
        &self,
        project_id: &ProjectId,
        expected_revision: u64,
        command: ProjectCommand,
    ) -> CommandResult<ProjectOverviewDto> {
        let occurred_at = self.clock.now_rfc3339();
        let mutation = apply_project_command(project_id, expected_revision, command, &occurred_at)?;
        let record = load_project(project_id)?;
        let now = self.now()?;
        Ok(self.overview(&record, &mutation.state, now))
    }

    /// The atomic setup-completion command (framing + first commitment + activation).
    pub fn complete_project_setup(
        &self,
        input: CompleteProjectSetupInput,
    ) -> CommandResult<ProjectOverviewDto> {
        self.apply_core(
            &input.project_id,
            input.expected_revision,
            ProjectCommand::CompleteSetup {
                objective: input.objective,
                desired_outcome: input.desired_outcome,
                phase: input.phase,
                first_commitment: input.first_commitment,
            },
        )
    }

    /// Observe a source path on the blocking pool. Wraps the join into a typed error.
    async fn observe(
        &self,
        location: PathBuf,
        observed_at: String,
    ) -> CommandResult<RepositoryObservation> {
        tokio::task::spawn_blocking(move || observe_repository(&location, &observed_at))
            .await
            .map_err(|error| {
                CommandError::new(
                    ErrorCode::SourceObservationFailed,
                    format!("observation task failed: {error}"),
                )
                .retryable()
            })?
            .map_err(CommandError::from)
    }

    /// Count commits since an instant on the blocking pool, or `None` on any failure
    /// (the count is an enrichment, never a hard error).
    async fn count_since(&self, location: PathBuf, since: String) -> Option<u32> {
        tokio::task::spawn_blocking(move || count_commits_since(&location, &since))
            .await
            .ok()?
            .ok()
    }

    /// One project's refresh. Never rejects the batch: every path returns a result.
    async fn refresh_one(&self, project_id: ProjectId, now: String) -> RefreshResultDto {
        let Some(_guard) = self.state.begin_refresh(&project_id).await else {
            // A refresh is already in flight: return the current cached row unchanged.
            return self.refresh_result(&project_id, RefreshOutcome::RefreshInProgress, None);
        };

        // Load canonical state; a load failure is reported per-row, not as a batch reject.
        let (record, state) = match (
            load_project(&project_id),
            ProjectStateDoc::load(&project_id),
        ) {
            (Ok(record), Ok(state)) => (record, state),
            (Err(error), _) => {
                return self.refresh_result(
                    &project_id,
                    RefreshOutcome::SourceFailed,
                    Some(error_category(&CommandError::from(error))),
                );
            }
            (_, Err(error)) => {
                return self.refresh_result(
                    &project_id,
                    RefreshOutcome::SourceFailed,
                    Some(error_category(&CommandError::from(error))),
                );
            }
        };
        let Some(source) = record.primary_git_source() else {
            return self.refresh_result(
                &project_id,
                RefreshOutcome::SourceFailed,
                Some("source_missing".into()),
            );
        };
        let source_id = source.id.clone();
        let source_revision = source.revision;
        let location = PathBuf::from(&source.location);
        let expected_location = source.location.clone();
        let current_commitment = state.current_next_action_id.clone();
        let commitment_set_at = current_commitment
            .as_ref()
            .and_then(|id| state.work_items.iter().find(|item| &item.id == id))
            .map(|item| item.created_at.clone());

        match self.observe(location.clone(), now.clone()).await {
            Ok(observation) => {
                // Enrich with the commitment-relative count when a commitment exists.
                let commits_since = match (&current_commitment, &commitment_set_at) {
                    (Some(_), Some(set_at)) => {
                        self.count_since(location.clone(), set_at.clone()).await
                    }
                    _ => None,
                };
                // Record the successful observation first (CAS on revision + location).
                let outcome = record_source_observation(RecordSourceObservationInput {
                    project_id: &project_id,
                    source_id: &source_id,
                    expected_source_revision: source_revision,
                    expected_location: &expected_location,
                    attempted_at: &now,
                    outcome: SourceObservationOutcome::Success {
                        successful_refresh_at: &now,
                    },
                });
                match outcome {
                    Ok(_) => {
                        // Only now persist the cache, keyed to this source id.
                        let cached = CachedObservation::from_observation(
                            project_id.clone(),
                            source_id.clone(),
                            &observation,
                            current_commitment.clone(),
                            commits_since,
                        );
                        if let Err(error) = repository_cache::store(&cached) {
                            return self.refresh_result(
                                &project_id,
                                RefreshOutcome::SourceFailed,
                                Some(error_category(&CommandError::from(error))),
                            );
                        }
                        self.refresh_result(&project_id, RefreshOutcome::Refreshed, None)
                    }
                    // A relink won the race: discard this stale result, do not overwrite.
                    Err(error) if is_stale_race(&error) => {
                        self.refresh_result(&project_id, RefreshOutcome::Stale, None)
                    }
                    Err(error) => self.refresh_result(
                        &project_id,
                        RefreshOutcome::SourceFailed,
                        Some(error_category(&CommandError::from(error))),
                    ),
                }
            }
            Err(read_error) => {
                // Preserve cached facts. Record the failure (status + category) but leave
                // the cache bytes untouched.
                let (status, category) = failure_status_and_category(&read_error);
                let outcome = record_source_observation(RecordSourceObservationInput {
                    project_id: &project_id,
                    source_id: &source_id,
                    expected_source_revision: source_revision,
                    expected_location: &expected_location,
                    attempted_at: &now,
                    outcome: SourceObservationOutcome::Failure {
                        status,
                        error_category: &category,
                    },
                });
                match outcome {
                    Err(error) if is_stale_race(&error) => {
                        self.refresh_result(&project_id, RefreshOutcome::Stale, None)
                    }
                    _ => self.refresh_result(
                        &project_id,
                        RefreshOutcome::SourceFailed,
                        Some(category),
                    ),
                }
            }
        }
    }

    /// Build a refresh result, reloading the current row so the UI always gets the latest
    /// facts (fresh on success, preserved on failure). A row that can no longer be built
    /// is returned as `item: None`.
    fn refresh_result(
        &self,
        project_id: &ProjectId,
        outcome: RefreshOutcome,
        error_category: Option<String>,
    ) -> RefreshResultDto {
        let item = self.current_row(project_id);
        RefreshResultDto {
            project_id: project_id.clone(),
            outcome,
            item,
            error_category,
        }
    }

    /// Build the current Index row for a project, or `None` if it can no longer be loaded.
    fn current_row(&self, project_id: &ProjectId) -> Option<ProjectIndexItemDto> {
        let record = load_project(project_id).ok()?;
        let state = ProjectStateDoc::load(project_id).ok()?;
        let now = self.now().ok()?;
        Some(self.index_item(&record, &state, now))
    }
}

impl<C: Clock> R0Service for DesktopService<C> {
    fn list_project_index(&self) -> CommandResult<ProjectIndexResponseDto> {
        let now = self.now()?;
        let records = list_project_records()?;
        let mut projects = Vec::with_capacity(records.len());
        for record in &records {
            let state = ProjectStateDoc::load(&record.id)?;
            // Archived projects are absent from the default Index (still addressable).
            if state.status == ProjectStatus::Archived {
                continue;
            }
            projects.push(self.index_item(record, &state, now));
        }
        projects.sort_by_key(index_sort_key);
        Ok(ProjectIndexResponseDto {
            projects,
            review_policy: ReviewPolicyDto::r0(),
        })
    }

    fn get_project_overview(&self, project_id: ProjectId) -> CommandResult<ProjectOverviewDto> {
        self.load_overview(&project_id)
    }

    async fn validate_project_source(
        &self,
        location: String,
    ) -> CommandResult<SourceValidationDto> {
        let now = self.clock.now_rfc3339();
        let display_location = canonical_display(&location);
        match self.observe(PathBuf::from(&location), now).await {
            Ok(observation) => {
                // A valid repo that is already owned is a duplicate.
                if let Some(existing_project_id) = canonical_source_owner(Path::new(&location))? {
                    let existing_name = load_project(&existing_project_id)
                        .map(|record| record.name)
                        .unwrap_or_default();
                    return Ok(SourceValidationDto::Duplicate {
                        location: display_location,
                        existing_project_id,
                        existing_name,
                    });
                }
                Ok(SourceValidationDto::Ok {
                    location: display_location,
                    head: head_state_dto(&observation.head_state),
                    last_commit: observation.last_commit.as_ref().map(|commit| CommitDto {
                        sha: commit.sha.clone(),
                        short_sha: commit.short_sha.clone(),
                        subject: commit.subject.clone(),
                        committed_at: commit.committed_at.clone(),
                    }),
                })
            }
            Err(error) => Ok(validation_from_error(display_location, error)),
        }
    }

    async fn register_project(
        &self,
        input: RegisterProjectInput,
    ) -> CommandResult<ProjectOverviewDto> {
        if input.name.trim().is_empty() {
            return Err(CommandError::invalid_input("project name is required").with_field("name"));
        }
        let now = self.clock.now_rfc3339();
        // Validate the source before creating any identity.
        self.observe(PathBuf::from(&input.location), now.clone())
            .await?;

        let created_at = now;
        let outcome = register_project(CoreRegisterProjectInput {
            location: Path::new(&input.location),
            name: input.name.trim(),
            created_at: &created_at,
        })?;
        let project_id = match outcome {
            RegisterOutcome::Created(record) => record.id,
            RegisterOutcome::Existing(existing_project_id) => {
                let mut error = CommandError::new(
                    ErrorCode::DuplicateSource,
                    "that location is already registered as a project",
                );
                error.existing_project_id = Some(existing_project_id.as_str().to_owned());
                return Err(error);
            }
        };
        self.load_overview(&project_id)
    }

    async fn relink_project_source(
        &self,
        input: RelinkProjectInput,
    ) -> CommandResult<ProjectOverviewDto> {
        let now = self.clock.now_rfc3339();
        // Validate the new source before mutating the envelope.
        self.observe(PathBuf::from(&input.new_location), now)
            .await?;

        relink_primary_git_source(RelinkSourceInput {
            project_id: &input.project_id,
            expected_source_revision: input.expected_source_revision,
            expected_location: &input.expected_location,
            new_location: Path::new(&input.new_location),
        })?;
        self.load_overview(&input.project_id)
    }

    async fn refresh_projects(
        &self,
        project_ids: Option<Vec<ProjectId>>,
    ) -> CommandResult<Vec<RefreshResultDto>> {
        let now = self.clock.now_rfc3339();
        let targets = match project_ids {
            Some(ids) => ids,
            None => list_project_records()?
                .into_iter()
                .map(|record| record.id)
                .collect(),
        };
        let mut results = Vec::with_capacity(targets.len());
        for project_id in targets {
            results.push(self.refresh_one(project_id, now.clone()).await);
        }
        Ok(results)
    }

    fn apply_project_mutation(
        &self,
        input: ProjectMutationInput,
    ) -> CommandResult<ProjectOverviewDto> {
        let command = match input.command {
            MutationCommand::SaveFraming {
                objective,
                desired_outcome,
                phase,
            } => ProjectCommand::SaveFraming {
                objective,
                desired_outcome,
                phase,
            },
            MutationCommand::SetStatus {
                status,
                reason,
                review_at,
            } => ProjectCommand::SetStatus {
                status,
                reason,
                review_at,
            },
            MutationCommand::SetCommitment { text } => ProjectCommand::SetCommitment { text },
            MutationCommand::ConfirmCommitment { work_item_id } => {
                ProjectCommand::ConfirmCommitment { work_item_id }
            }
            MutationCommand::CompleteCommitment { work_item_id } => {
                ProjectCommand::CompleteCommitment { work_item_id }
            }
            MutationCommand::ReplaceCommitment {
                previous_work_item_id,
                text,
                reason,
            } => ProjectCommand::ReplaceCommitment {
                previous_work_item_id,
                text,
                reason,
            },
            MutationCommand::ClearCommitment {
                work_item_id,
                reason,
            } => ProjectCommand::ClearCommitment {
                work_item_id,
                reason,
            },
            MutationCommand::Undo { transition_id } => ProjectCommand::Undo { transition_id },
        };
        self.apply_core(&input.project_id, input.expected_revision, command)
    }
}

/// Whether a record_source_observation error means a relink moved the source under us.
fn is_stale_race(error: &omniproj_core::project::ProjectStoreError) -> bool {
    use omniproj_core::project::ProjectStoreError;
    matches!(
        error,
        ProjectStoreError::LocationConflict { .. } | ProjectStoreError::RevisionConflict { .. }
    )
}

/// Map an observation read error to the source status and stable error category recorded
/// in the source envelope.
fn failure_status_and_category(
    error: &crate::error::CommandError,
) -> (ProjectSourceStatus, String) {
    let category = error_category(error);
    let status = match error.code {
        ErrorCode::SourceMissing => ProjectSourceStatus::Missing,
        _ => ProjectSourceStatus::Unreadable,
    };
    (status, category)
}

/// The stable snake_case category string for an error code.
fn error_category(error: &crate::error::CommandError) -> String {
    serde_json::to_value(error.code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "source_observation_failed".into())
}

/// Map a source read error to a typed validation state.
fn validation_from_error(location: String, error: CommandError) -> SourceValidationDto {
    match error.code {
        ErrorCode::SourceMissing => SourceValidationDto::Missing { location },
        ErrorCode::SourceUnreadable => SourceValidationDto::Unreadable { location },
        ErrorCode::NotGitRepository => SourceValidationDto::NotGitRepository { location },
        ErrorCode::BareRepository => SourceValidationDto::BareRepository { location },
        _ => SourceValidationDto::ObservationFailed {
            location,
            message: error.message,
        },
    }
}

/// Best-effort canonical display for a candidate location (falls back to the raw string).
fn canonical_display(location: &str) -> String {
    std::fs::canonicalize(location)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| location.to_owned())
}
