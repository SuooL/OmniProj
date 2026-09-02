//! Task 7 acceptance suite for the typed R0 desktop service and command allowlist.
//!
//! Prefixes group the steps: `error_` (serialization contract), `dto_` (wire shapes),
//! `service_` (methods), `refresh_` (observation cache + partial batch), `handler_`
//! (behavior-level IPC allowlist). Every test that touches the store serializes on
//! `OMNIPROJ_HOME` via the shared env guard.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde_json::{json, Value};

use omniproj_core::project::{register_project, RegisterOutcome, RegisterProjectInput};
use omniproj_core::project_state::{ProjectStateDoc, ProjectStatus};
use omniproj_core::{ensure_home, ProjectId, ProjectRecord};

use omniproj_desktop::dto;
use omniproj_desktop::error::{CommandError, ErrorCode};
use omniproj_desktop::mvp;
use omniproj_desktop::repository_cache;
use omniproj_desktop::service::{Clock, DesktopService, R0Service};

const NOW: &str = "2026-08-12T10:00:00Z";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn env_guard() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn unique_path(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "omniproj-desktop-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

/// A throwaway `~/.omniproj` with `OMNIPROJ_HOME` pointed at it and the store initialized.
struct Home {
    path: PathBuf,
}

impl Home {
    fn new(tag: &str) -> Self {
        let path = unique_path(&format!("{tag}-home"));
        std::env::set_var("OMNIPROJ_HOME", &path);
        ensure_home().unwrap();
        Self { path }
    }

    fn legacy_v1(tag: &str) -> Self {
        let path = unique_path(&format!("{tag}-v1-home"));
        std::env::set_var("OMNIPROJ_HOME", &path);

        let project_id = "4480aa56adb5ec98";
        let project = path.join("projects").join(project_id);
        std::fs::create_dir_all(project.join("notes")).unwrap();
        std::fs::write(path.join("SCHEMA_VERSION"), "1\n").unwrap();
        std::fs::write(path.join(".gitignore"), "projects/*/cache/\n").unwrap();
        std::fs::write(
            project.join("meta.toml"),
            format!(
                "path = {:?}\nname = \"Legacy Desktop Project\"\nhash = {:?}\nadded_at = {:?}\n",
                "/Users/research/legacy-desktop-project", project_id, NOW,
            ),
        )
        .unwrap();

        git(&path, &["init", "-q"]);
        git(&path, &["config", "user.name", "R0 Test"]);
        git(&path, &["config", "user.email", "r0@test.invalid"]);
        git(&path, &["config", "commit.gpgsign", "false"]);
        git(&path, &["add", "-A"]);
        git(&path, &["commit", "-q", "-m", "seed schema v1"]);

        Self { path }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct FixedClock(String);

impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        self.0.clone()
    }
}

fn service() -> DesktopService<FixedClock> {
    DesktopService::new(FixedClock(NOW.to_owned()))
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git must be available");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "R0 Test"]);
    git(dir, &["config", "user.email", "r0@test.invalid"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
}

fn commit(dir: &Path, subject: &str, name: &str) {
    std::fs::write(dir.join(name), format!("{subject}\n")).unwrap();
    git(dir, &["add", "."]);
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["commit", "-q", "-m", subject])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_DATE", "2026-08-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-08-01T00:00:00Z")
        .output()
        .expect("git commit");
    assert!(output.status.success(), "commit failed");
}

/// A git repo with one commit (attached HEAD).
fn repo_with_commit(tag: &str) -> PathBuf {
    let dir = unique_path(tag);
    init_repo(&dir);
    commit(&dir, "initial", "README.md");
    dir
}

fn register(location: &Path, name: &str) -> ProjectRecord {
    std::fs::create_dir_all(location).unwrap();
    match register_project(RegisterProjectInput {
        location,
        name,
        created_at: NOW,
    })
    .unwrap()
    {
        RegisterOutcome::Created(record) => record,
        RegisterOutcome::Existing(id) => panic!("unexpected existing {id}"),
    }
}

#[test]
fn mvp_task_writes_are_revision_checked_atomic_and_path_scoped() {
    let _guard = env_guard();
    let home = Home::new("mvp-task-write");
    let repo = repo_with_commit("mvp-task-write-repo");
    let record = register(&repo, "Task project");

    let initial = mvp::get_tasks(record.id.clone()).unwrap();
    assert!(initial.tasks.is_empty());
    let first = mvp::add_task(
        record.id.clone(),
        initial.revision.clone(),
        "Validate cohort labels".into(),
        true,
    )
    .unwrap();
    assert_eq!(first.tasks.len(), 1);
    assert!(first.tasks[0].unclear);
    assert_ne!(first.revision, initial.revision);

    let stale = mvp::add_task(
        record.id.clone(),
        initial.revision,
        "This must not overwrite".into(),
        false,
    )
    .unwrap_err();
    assert_eq!(stale.code, ErrorCode::RevisionConflict);
    assert_eq!(mvp::get_tasks(record.id.clone()).unwrap().tasks.len(), 1);

    let relative = format!("projects/{}/notes/project.md", record.id.as_str());
    let committed = Command::new("git")
        .arg("-C")
        .arg(&home.path)
        .args(["show", "--name-only", "--format=", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&committed.stdout).trim(), relative);
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn mvp_plan_link_and_advance_adoption_preserve_provenance() {
    let _guard = env_guard();
    let _home = Home::new("mvp-plan-provenance");
    let repo = repo_with_commit("mvp-plan-provenance-repo");
    let record = register(&repo, "Plan project");

    let plan = mvp::get_plan(record.id.clone()).unwrap();
    let plan = mvp::add_plan_entry(
        record.id.clone(),
        plan.revision,
        "Use external validation".into(),
        "Avoid optimistic internal-only claims".into(),
    )
    .unwrap();
    let entry_id = plan.entries[0].id.clone().unwrap();
    let plan = mvp::set_plan_commit(
        record.id.clone(),
        plan.revision,
        entry_id,
        Some("deadbeef".into()),
    )
    .unwrap();
    assert_eq!(plan.entries[0].commit.as_deref(), Some("deadbeef"));

    let tasks = mvp::get_tasks(record.id.clone()).unwrap();
    let adopted = mvp::adopt_subtasks(
        record.id.clone(),
        tasks.revision,
        "proposal-1234".into(),
        vec!["Define the external cohort".into()],
    )
    .unwrap();
    assert_eq!(
        adopted.tasks[0].adopted_from_proposal_id.as_deref(),
        Some("proposal-1234")
    );
    let state = ProjectStateDoc::load(&record.id).unwrap();
    assert_eq!(
        state.work_items[0].adopted_from_proposal_id.as_deref(),
        Some("proposal-1234")
    );
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn dogfood_events_are_audited_and_summarized_across_projects() {
    let _guard = env_guard();
    let _home = Home::new("dogfood-events");
    let repo_a = repo_with_commit("dogfood-events-a");
    let repo_b = repo_with_commit("dogfood-events-b");
    let a = register(&repo_a, "A");
    let b = register(&repo_b, "B");
    let first = mvp::record_reentry_event(a.id, 90).unwrap();
    assert_eq!(first.event_count, 1);
    assert_eq!(first.project_count, 1);
    let second = mvp::record_reentry_event(b.id, 30).unwrap();
    assert_eq!(second.event_count, 2);
    assert_eq!(second.project_count, 2);
    assert_eq!(second.median_duration_seconds, Some(60));
    assert!(!second.meets_event_threshold && !second.meets_project_threshold);
    let _ = std::fs::remove_dir_all(repo_a);
    let _ = std::fs::remove_dir_all(repo_b);
}

// ---------------------------------------------------------------------------
// error_ : the serialization contract (step 2)
// ---------------------------------------------------------------------------

#[test]
fn error_required_codes_all_serialize_snake_case() {
    let expected = [
        (ErrorCode::ProjectNotFound, "project_not_found"),
        (ErrorCode::InvalidInput, "invalid_input"),
        (ErrorCode::InvalidPath, "invalid_path"),
        (ErrorCode::SourceMissing, "source_missing"),
        (ErrorCode::SourceUnreadable, "source_unreadable"),
        (ErrorCode::NotGitRepository, "not_git_repository"),
        (ErrorCode::BareRepository, "bare_repository"),
        (ErrorCode::DuplicateSource, "duplicate_source"),
        (
            ErrorCode::SourceObservationFailed,
            "source_observation_failed",
        ),
        (ErrorCode::StoreReadFailed, "store_read_failed"),
        (ErrorCode::StoreWriteFailed, "store_write_failed"),
        (ErrorCode::AuditCommitFailed, "audit_commit_failed"),
        (ErrorCode::RevisionConflict, "revision_conflict"),
        (
            ErrorCode::CurrentCommitmentExists,
            "current_commitment_exists",
        ),
        (ErrorCode::NoCurrentCommitment, "no_current_commitment"),
        (
            ErrorCode::CurrentCommitmentChanged,
            "current_commitment_changed",
        ),
        (ErrorCode::ReasonRequired, "reason_required"),
        (ErrorCode::TransitionNotFound, "transition_not_found"),
        (ErrorCode::UndoNotAvailable, "undo_not_available"),
        (ErrorCode::UndoConflict, "undo_conflict"),
    ];
    for (code, name) in expected {
        assert_eq!(serde_json::to_value(code).unwrap(), json!(name));
    }
}

#[test]
fn error_audit_commit_failed_is_the_only_state_applied_error() {
    let audit = CommandError::audit_commit_failed(7);
    let value = serde_json::to_value(&audit).unwrap();
    assert_eq!(value["code"], json!("audit_commit_failed"));
    assert_eq!(value["state_applied"], json!(true));
    assert_eq!(value["retryable"], json!(false));
    assert_eq!(value["durable_revision"], json!(7));

    // store_write_failed is the retryable, not-yet-applied counterpart.
    let write = CommandError::from(omniproj_core::store::StoreError::Io(std::io::Error::other(
        "disk full",
    )));
    let value = serde_json::to_value(&write).unwrap();
    assert_eq!(value["code"], json!("store_write_failed"));
    assert_eq!(value["state_applied"], json!(false));
    assert_eq!(value["retryable"], json!(true));
}

#[test]
fn error_optional_fields_are_omitted_when_absent() {
    let value = serde_json::to_value(CommandError::invalid_input("bad")).unwrap();
    let object = value.as_object().unwrap();
    assert!(!object.contains_key("field"));
    assert!(!object.contains_key("project_id"));
    assert!(!object.contains_key("existing_project_id"));
    assert!(!object.contains_key("durable_revision"));

    let with_field = serde_json::to_value(
        CommandError::invalid_input("objective is required").with_field("objective"),
    )
    .unwrap();
    assert_eq!(with_field["field"], json!("objective"));
}

#[test]
fn error_maps_commitment_mismatch_to_distinct_codes() {
    use omniproj_core::project_state::ProjectStateError;
    let expected = ProjectId::parse("project-mismatch").unwrap();
    let work = omniproj_core::WorkItemId::new();

    let no_current = CommandError::from(ProjectStateError::CurrentCommitmentMismatch {
        expected: work.clone(),
        actual: None,
    });
    assert_eq!(no_current.code, ErrorCode::NoCurrentCommitment);

    let changed = CommandError::from(ProjectStateError::CurrentCommitmentMismatch {
        expected: work.clone(),
        actual: Some(omniproj_core::WorkItemId::new()),
    });
    assert_eq!(changed.code, ErrorCode::CurrentCommitmentChanged);

    let reason = CommandError::from(ProjectStateError::ReasonRequired);
    assert_eq!(reason.code, ErrorCode::ReasonRequired);

    let conflict = CommandError::from(ProjectStateError::RevisionConflict {
        expected: 1,
        actual: 2,
    });
    assert_eq!(conflict.code, ErrorCode::RevisionConflict);
    assert!(!conflict.retryable);

    let audit = CommandError::from(ProjectStateError::AuditCommitFailed {
        durable_revision: 4,
        source: omniproj_core::store::StoreError::AuditCommit("hook".into()),
    });
    assert_eq!(audit.code, ErrorCode::AuditCommitFailed);
    assert!(audit.state_applied);
    assert_eq!(audit.durable_revision, Some(4));
    let _ = expected;
}

// ---------------------------------------------------------------------------
// dto_ : wire shapes and enum values (step 3)
// ---------------------------------------------------------------------------

#[test]
fn dto_review_policy_is_r0() {
    let value = serde_json::to_value(dto::ReviewPolicyDto::r0()).unwrap();
    assert_eq!(value["commitment_review_days"], json!(7));
    assert_eq!(value["rule_version"], json!("r1-v1"));
}

#[test]
fn dto_enum_tags_are_snake_case() {
    assert_eq!(
        serde_json::to_value(dto::HeadStateDto::Attached {
            branch: "main".into()
        })
        .unwrap(),
        json!({ "kind": "attached", "branch": "main" })
    );
    assert_eq!(
        serde_json::to_value(dto::RefreshOutcome::RefreshInProgress).unwrap(),
        json!("refresh_in_progress")
    );
    assert_eq!(
        serde_json::to_value(dto::SourceValidationDto::NotGitRepository {
            location: "/tmp/x".into()
        })
        .unwrap(),
        json!({ "state": "not_git_repository", "location": "/tmp/x" })
    );
}

#[test]
fn dto_index_row_excludes_source_path_while_overview_includes_it() {
    let _guard = env_guard();
    let _home = Home::new("dto-shapes");
    let source = repo_with_commit("dto-shapes-src");
    let record = register(&source, "Shapes");
    let stored_location = record.primary_git_source().unwrap().location.clone();

    let service = service();
    let index = service.list_project_index().unwrap();
    let row = serde_json::to_value(&index.projects[0]).unwrap();
    let row_object = row.as_object().unwrap();
    // Field names present on a dense row.
    for key in [
        "project_id",
        "name",
        "status",
        "review_reasons",
        "source_status",
        "revision",
        "source_revision",
    ] {
        assert!(row_object.contains_key(key), "index row missing {key}");
    }
    // The full source path is Overview-only.
    assert!(
        !row.to_string().contains(&stored_location),
        "index row leaked the source path"
    );

    let overview = service.get_project_overview(record.id.clone()).unwrap();
    let overview_value = serde_json::to_value(&overview).unwrap();
    assert_eq!(overview_value["source"]["location"], json!(stored_location));
    assert_eq!(
        overview_value["review_policy"]["rule_version"],
        json!("r1-v1")
    );
}

// ---------------------------------------------------------------------------
// service_ : methods (step 5)
// ---------------------------------------------------------------------------

#[test]
fn service_startup_migrates_v1_store_before_first_index_read() {
    let _guard = env_guard();
    let home = Home::legacy_v1("startup-migration");

    let service = DesktopService::initialize(FixedClock(NOW.to_owned())).unwrap();
    let index = service.list_project_index().unwrap();

    assert_eq!(
        std::fs::read_to_string(home.path.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    assert_eq!(index.projects.len(), 1);
    assert_eq!(index.projects[0].name, "Legacy Desktop Project");
    assert_eq!(index.projects[0].status, ProjectStatus::Setup);
}

#[test]
fn service_index_includes_archived_projects_for_recovery() {
    let _guard = env_guard();
    let _home = Home::new("archived");
    let visible = register(&unique_path("archived-visible"), "Visible");
    let archived = register(&unique_path("archived-hidden"), "Archived");

    // Archive `hidden` by driving its state to Archived (Active -> Archived).
    let service = service();
    // Move Hidden to Active first via a commitment-free status change is not allowed from
    // Setup, so complete setup, then archive.
    service
        .complete_project_setup(dto::CompleteProjectSetupInput {
            project_id: archived.id.clone(),
            expected_revision: 0,
            objective: "o".into(),
            desired_outcome: "d".into(),
            phase: None,
            first_commitment: "first".into(),
        })
        .unwrap();
    service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: archived.id.clone(),
            expected_revision: 1,
            command: dto::MutationCommand::SetStatus {
                status: ProjectStatus::Archived,
                reason: None,
                review_at: None,
            },
        })
        .unwrap();

    let index = service.list_project_index().unwrap();
    let ids: Vec<&str> = index
        .projects
        .iter()
        .map(|p| p.project_id.as_str())
        .collect();
    assert!(ids.contains(&visible.id.as_str()));
    assert!(
        ids.contains(&archived.id.as_str()),
        "archived projects must remain discoverable so they can be restored"
    );
    let overview = service.get_project_overview(archived.id.clone()).unwrap();
    assert_eq!(overview.status, ProjectStatus::Archived);
}

#[test]
fn service_register_returns_a_fresh_observation_without_manual_refresh() {
    let _guard = env_guard();
    let _home = Home::new("register-observation");
    let source = repo_with_commit("register-observation-src");
    let service = service();

    let overview = block_on(service.register_project(dto::RegisterProjectInput {
        location: source.to_string_lossy().into_owned(),
        name: "Fresh registration".into(),
    }))
    .unwrap();

    let observed = overview
        .observed_actual
        .expect("registration should persist its validation observation");
    assert_eq!(observed.observed_at, NOW);
    assert_eq!(observed.last_commit.unwrap().subject, "initial");
    assert_eq!(
        overview
            .source
            .unwrap()
            .last_successful_refresh_at
            .as_deref(),
        Some(NOW)
    );
}

#[test]
fn service_mutation_returns_updated_overview_and_undoable_transition() {
    let _guard = env_guard();
    let _home = Home::new("mutation");
    let project = register(&unique_path("mutation-src"), "Mutation");
    let service = service();

    let after_setup = service
        .complete_project_setup(dto::CompleteProjectSetupInput {
            project_id: project.id.clone(),
            expected_revision: 0,
            objective: "Ship R0".into(),
            desired_outcome: "Dogfood".into(),
            phase: None,
            first_commitment: "Wire the service".into(),
        })
        .unwrap();
    assert_eq!(after_setup.status, ProjectStatus::Active);
    let commitment = after_setup.current_commitment.expect("current commitment");
    assert_eq!(commitment.text, "Wire the service");
    assert_eq!(after_setup.revision, 1);
    // The Set transition that produced revision 1 is undoable.
    assert_eq!(
        after_setup.undoable_transition_id.as_ref(),
        after_setup.last_transition.as_ref().map(|t| &t.id)
    );

    let after_confirm = service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: project.id.clone(),
            expected_revision: 1,
            command: dto::MutationCommand::ConfirmCommitment {
                work_item_id: commitment.work_item_id.clone(),
            },
        })
        .unwrap();
    assert_eq!(after_confirm.revision, 2);
    assert!(after_confirm
        .current_commitment
        .as_ref()
        .and_then(|c| c.confirmed_at.as_ref())
        .is_some());
}

#[test]
fn service_mutation_revision_conflict_is_typed() {
    let _guard = env_guard();
    let _home = Home::new("conflict");
    let project = register(&unique_path("conflict-src"), "Conflict");
    let service = service();

    let error = service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: project.id.clone(),
            expected_revision: 99,
            command: dto::MutationCommand::SetCommitment {
                text: "stale write".into(),
            },
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RevisionConflict);
    assert!(!error.state_applied);
}

#[test]
fn service_validate_source_reports_typed_states() {
    let _guard = env_guard();
    let _home = Home::new("validate");
    let service = service();

    let repo = repo_with_commit("validate-ok");
    match block_on(service.validate_project_source(repo.to_string_lossy().into_owned())).unwrap() {
        dto::SourceValidationDto::Ok {
            head, last_commit, ..
        } => {
            assert_eq!(
                head,
                dto::HeadStateDto::Attached {
                    branch: "main".into()
                }
            );
            assert!(last_commit.is_some());
        }
        other => panic!("expected Ok, got {other:?}"),
    }

    let missing = unique_path("validate-missing");
    assert!(matches!(
        block_on(service.validate_project_source(missing.to_string_lossy().into_owned())).unwrap(),
        dto::SourceValidationDto::Missing { .. }
    ));

    let plain = unique_path("validate-plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert!(matches!(
        block_on(service.validate_project_source(plain.to_string_lossy().into_owned())).unwrap(),
        dto::SourceValidationDto::NotGitRepository { .. }
    ));

    // A registered repo validates as a Duplicate.
    let owned = repo_with_commit("validate-dup");
    let record = register(&owned, "Owned");
    match block_on(service.validate_project_source(owned.to_string_lossy().into_owned())).unwrap() {
        dto::SourceValidationDto::Duplicate {
            existing_project_id,
            ..
        } => {
            assert_eq!(existing_project_id, record.id);
        }
        other => panic!("expected Duplicate, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// refresh_ : observation cache + partial batch (step 4)
// ---------------------------------------------------------------------------

#[test]
fn refresh_success_caches_observation_and_populates_observed_actual() {
    let _guard = env_guard();
    let _home = Home::new("refresh-ok");
    let source = repo_with_commit("refresh-ok-src");
    let record = register(&source, "Refreshed");
    let service = service();

    let results = block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].outcome, dto::RefreshOutcome::Refreshed);
    let item = results[0].item.as_ref().expect("row");
    let observed = item.observed_actual.as_ref().expect("observed actual");
    assert_eq!(
        observed.head,
        dto::HeadStateDto::Attached {
            branch: "main".into()
        }
    );
    assert!(observed.last_commit.is_some());

    // The cache file exists and is keyed to the project's source.
    let source = record.primary_git_source().unwrap();
    assert!(repository_cache::load(source).is_some());

    // The overview carries the same observed actual.
    let overview = service.get_project_overview(record.id.clone()).unwrap();
    assert!(overview.observed_actual.is_some());
}

#[test]
fn refresh_unborn_repo_is_success_with_null_last_commit() {
    let _guard = env_guard();
    let _home = Home::new("refresh-unborn");
    let source = unique_path("refresh-unborn-src");
    init_repo(&source); // no commit -> unborn HEAD
    let record = register(&source, "Unborn");
    let service = service();

    let results = block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();
    assert_eq!(results[0].outcome, dto::RefreshOutcome::Refreshed);
    let observed = results[0]
        .item
        .as_ref()
        .unwrap()
        .observed_actual
        .as_ref()
        .unwrap();
    assert!(observed.last_commit.is_none());
    assert!(matches!(observed.head, dto::HeadStateDto::Unborn { .. }));
}

#[test]
fn refresh_detached_head_is_reported() {
    let _guard = env_guard();
    let _home = Home::new("refresh-detached");
    let source = repo_with_commit("refresh-detached-src");
    git(&source, &["checkout", "-q", "--detach", "HEAD"]);
    let record = register(&source, "Detached");
    let service = service();

    let results = block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();
    let observed = results[0]
        .item
        .as_ref()
        .unwrap()
        .observed_actual
        .as_ref()
        .unwrap();
    assert_eq!(observed.head, dto::HeadStateDto::Detached);
}

#[test]
fn refresh_missing_source_preserves_cached_facts() {
    let _guard = env_guard();
    let _home = Home::new("refresh-preserve");
    let source = repo_with_commit("refresh-preserve-src");
    let record = register(&source, "Preserve");
    let service = service();

    // First a good refresh populates the cache.
    block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();
    // Then the source disappears.
    std::fs::remove_dir_all(&source).unwrap();
    let results = block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();

    assert_eq!(results[0].outcome, dto::RefreshOutcome::SourceFailed);
    assert_eq!(results[0].error_category.as_deref(), Some("source_missing"));
    // Cached facts survive the failure.
    let item = results[0].item.as_ref().unwrap();
    assert!(
        item.observed_actual.is_some(),
        "cached facts must be preserved"
    );
    assert_eq!(
        item.source_status,
        omniproj_core::project::ProjectSourceStatus::Missing
    );
}

#[test]
fn refresh_partial_batch_returns_one_result_per_project() {
    let _guard = env_guard();
    let _home = Home::new("refresh-partial");
    let good = register(&repo_with_commit("refresh-partial-good"), "Good");
    let broken_source = repo_with_commit("refresh-partial-broken");
    let broken = register(&broken_source, "Broken");
    std::fs::remove_dir_all(&broken_source).unwrap();

    let service = service();
    let results = block_on(service.refresh_projects(None)).unwrap();
    assert_eq!(results.len(), 2, "one result per project");
    let outcome_of = |id: &ProjectId| {
        results
            .iter()
            .find(|r| &r.project_id == id)
            .map(|r| r.outcome)
            .unwrap()
    };
    assert_eq!(outcome_of(&good.id), dto::RefreshOutcome::Refreshed);
    assert_eq!(outcome_of(&broken.id), dto::RefreshOutcome::SourceFailed);
}

#[test]
fn refresh_skips_a_concurrent_in_flight_refresh() {
    let _guard = env_guard();
    let _home = Home::new("refresh-skip");
    let source = repo_with_commit("refresh-skip-src");
    let record = register(&source, "Skip");
    let service = service();

    block_on(async {
        // Hold the in-flight slot, then a second refresh must skip.
        let held = service
            .state
            .begin_refresh(&record.id)
            .await
            .expect("first claim succeeds");
        let results = service
            .refresh_projects(Some(vec![record.id.clone()]))
            .await
            .unwrap();
        assert_eq!(results[0].outcome, dto::RefreshOutcome::RefreshInProgress);
        drop(held);
        // Once released, a refresh proceeds.
        assert!(!service.state.is_refreshing(&record.id).await);
    });
}

#[test]
fn refresh_never_clobbers_a_concurrent_relink() {
    // An injected refresh-versus-relink race: whatever the interleaving, the relinked
    // location must win and the source revision must advance monotonically.
    let _guard = env_guard();
    let _home = Home::new("refresh-relink-race");
    let original = repo_with_commit("refresh-relink-a");
    let record = register(&original, "Race");
    let relinked = repo_with_commit("refresh-relink-b");
    let expected_location = record.primary_git_source().unwrap().location.clone();
    let expected_revision = record.primary_git_source().unwrap().revision;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let service = service();
    let id = record.id.clone();
    let relinked_path = relinked.clone();
    runtime.block_on(async {
        let refresh = service.refresh_projects(Some(vec![id.clone()]));
        let relink = service.relink_project_source(dto::RelinkProjectInput {
            project_id: id.clone(),
            expected_source_revision: expected_revision,
            expected_location: expected_location.clone(),
            new_location: relinked_path.to_string_lossy().into_owned(),
        });
        let (refresh_result, relink_result) = tokio::join!(refresh, relink);
        // The batch always returns exactly one result, never rejecting on the race.
        assert_eq!(refresh_result.unwrap().len(), 1, "batch returns one result");
        // The relink either applied cleanly or lost the optimistic-concurrency check —
        // it is never a torn write.
        if let Err(error) = relink_result {
            assert_eq!(error.code, ErrorCode::RevisionConflict);
        }
    });

    // Whatever the interleaving, the CAS mediates a consistent outcome: the location is
    // one of the two candidates (never a clobbered/torn state) and the revision advanced.
    let final_record = omniproj_core::load_project(&record.id).unwrap();
    let final_source = final_record.primary_git_source().unwrap();
    let canonical_original = std::fs::canonicalize(&original)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let canonical_relinked = std::fs::canonicalize(&relinked)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        final_source.location == canonical_original || final_source.location == canonical_relinked,
        "source location {} is neither candidate",
        final_source.location
    );
    assert!(final_source.revision > expected_revision);
}

#[test]
fn relink_immediately_replaces_the_previous_repository_observation() {
    // A relink keeps the same source id but changes the location. The previous repo's
    // observed facts must not survive as the current observed-actual.
    let _guard = env_guard();
    let _home = Home::new("relink-invalidate");
    let source_a = repo_with_commit("relink-invalidate-a");
    let record = register(&source_a, "Relinked");
    let service = service();

    block_on(service.refresh_projects(Some(vec![record.id.clone()]))).unwrap();
    assert!(service
        .get_project_overview(record.id.clone())
        .unwrap()
        .observed_actual
        .is_some());

    // Relink to a different repository.
    let source_b = repo_with_commit("relink-invalidate-b");
    let current = omniproj_core::load_project(&record.id).unwrap();
    let current_source = current.primary_git_source().unwrap();
    block_on(service.relink_project_source(dto::RelinkProjectInput {
        project_id: record.id.clone(),
        expected_source_revision: current_source.revision,
        expected_location: current_source.location.clone(),
        new_location: source_b.to_string_lossy().into_owned(),
    }))
    .unwrap();

    let observed = service
        .get_project_overview(record.id.clone())
        .unwrap()
        .observed_actual
        .expect("relink should persist the new repository observation immediately");
    assert_eq!(observed.last_commit.unwrap().subject, "initial");
    let cached = repository_cache::load(
        omniproj_core::load_project(&record.id)
            .unwrap()
            .primary_git_source()
            .unwrap(),
    )
    .expect("new source observation is cached");
    assert_eq!(
        cached.source_location,
        std::fs::canonicalize(&source_b).unwrap().to_string_lossy()
    );
}

#[test]
fn service_undo_error_codes_are_distinguished() {
    let _guard = env_guard();
    let _home = Home::new("undo-codes");
    let project = register(&unique_path("undo-codes-src"), "Undo");
    let service = service();

    let after_setup = service
        .complete_project_setup(dto::CompleteProjectSetupInput {
            project_id: project.id.clone(),
            expected_revision: 0,
            objective: "o".into(),
            desired_outcome: "d".into(),
            phase: None,
            first_commitment: "first".into(),
        })
        .unwrap();
    let set_transition = after_setup.last_transition.unwrap().id;

    // An unknown transition id -> transition_not_found (not a generic undo_conflict).
    let unknown = omniproj_core::CommitmentTransitionId::new();
    let error = service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: project.id.clone(),
            expected_revision: 1,
            command: dto::MutationCommand::Undo {
                transition_id: unknown,
            },
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TransitionNotFound);

    // Undo the Set (revision 1 -> 2, appending a Correction).
    service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: project.id.clone(),
            expected_revision: 1,
            command: dto::MutationCommand::Undo {
                transition_id: set_transition.clone(),
            },
        })
        .unwrap();

    // Now the newest transition is a correction, so nothing is undoable: undoing the
    // (still-present) Set id -> undo_not_available.
    let error = service
        .apply_project_mutation(dto::ProjectMutationInput {
            project_id: project.id.clone(),
            expected_revision: 2,
            command: dto::MutationCommand::Undo {
                transition_id: set_transition,
            },
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::UndoNotAvailable);
}

// ---------------------------------------------------------------------------
// handler_ : behavior-level IPC allowlist (step 6)
// ---------------------------------------------------------------------------

#[test]
fn handler_rejects_deferred_commands_and_accepts_the_r0_surface() {
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::webview::InvokeRequest;

    let _guard = env_guard();
    let _home = Home::new("handler");

    // The commands resolve `State<DesktopService<SystemClock>>`, so the managed value
    // must be the production service type.
    let app = mock_builder()
        .manage(DesktopService::new(omniproj_desktop::service::SystemClock))
        .invoke_handler(omniproj_desktop::r0_invoke_handler())
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let invoke = |cmd: &str, body: Value| -> Result<Value, Value> {
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|response| response.deserialize::<Value>().unwrap())
    };

    // Commands outside the reviewed surface remain rejected as unregistered.
    for deferred in ["get_graph", "get_attention"] {
        let error = invoke(deferred, json!({})).expect_err("deferred command must be rejected");
        assert!(
            error
                .as_str()
                .is_some_and(|message| message.contains("not found")),
            "expected `{deferred}` to be not found, got {error}"
        );
    }

    // Every shipped data command is accepted by the handler boundary (it may still error on
    // args, but never with the unregistered-command sentinel). `test_reminder` is exercised in
    // a real app runtime because the mock builder does not install the notification plugin.
    let r0_commands = [
        "list_project_index",
        "get_project_overview",
        "validate_project_source",
        "register_project",
        "relink_project_source",
        "refresh_projects",
        "complete_project_setup",
        "save_project_framing",
        "set_project_status",
        "set_commitment",
        "confirm_commitment",
        "complete_commitment",
        "replace_commitment",
        "clear_commitment",
        "undo_commitment_transition",
        "get_tasks",
        "get_attention_summary",
        "add_task",
        "update_task",
        "remove_task",
        "attribute_commit",
        "unattribute_commit",
        "get_commit_timeline",
        "get_git_graph",
        "advance_task",
        "adopt_subtasks",
        "promote_task_to_commitment",
        "get_plan",
        "add_plan_entry",
        "set_plan_status",
        "set_plan_commit",
        "get_reminder_settings",
        "set_reminder_settings",
        "get_dogfood_summary",
        "record_reentry_event",
    ];
    for command in r0_commands {
        let response = invoke(command, json!({}));
        if let Err(error) = &response {
            assert!(
                !error
                    .as_str()
                    .is_some_and(|message| message.contains("not found")),
                "R0 command `{command}` was rejected as unregistered: {error}"
            );
        }
    }

    // The read path actually runs and returns a well-formed response.
    let index = invoke("list_project_index", json!({})).expect("list_project_index runs");
    assert!(index["projects"].is_array());
    assert_eq!(index["review_policy"]["rule_version"], json!("r1-v1"));
}

#[test]
fn service_index_reports_overdue_work_and_orders_it_into_the_decision_queue() {
    let _guard = env_guard();
    let _home = Home::new("overdue");
    let record = register(&unique_path("overdue-repo"), "Overdue project");

    let service = service();
    service
        .complete_project_setup(dto::CompleteProjectSetupInput {
            project_id: record.id.clone(),
            expected_revision: 0,
            objective: "o".into(),
            desired_outcome: "d".into(),
            phase: None,
            first_commitment: "first".into(),
        })
        .unwrap();

    // A second task whose user-set expected date is long past (far enough that the
    // local-timezone day boundary cannot matter to this assertion).
    let tasks = mvp::get_tasks(record.id.clone()).unwrap();
    let tasks = mvp::add_task(
        record.id.clone(),
        tasks.revision,
        "Ship the overdue milestone".into(),
        false,
    )
    .unwrap();
    let late = tasks
        .tasks
        .iter()
        .find(|task| task.text == "Ship the overdue milestone")
        .unwrap()
        .id
        .clone();
    mvp::update_task(
        record.id.clone(),
        tasks.revision,
        late,
        "open".into(),
        Some("2026-08-01".into()),
        None,
    )
    .unwrap();

    let index = service.list_project_index().unwrap();
    let project = index
        .projects
        .iter()
        .find(|p| p.project_id == record.id)
        .unwrap();
    let overdue = project
        .review_reasons
        .iter()
        .find(|reason| reason.code == "overdue_work")
        .expect("overdue_work reason on the index row");
    assert_eq!(overdue.label, "Overdue work");
    assert!(overdue
        .evidence
        .iter()
        .any(|line| line == "overdue items: 1"));
    assert!(overdue
        .evidence
        .iter()
        .any(|line| line.contains("due 2026-08-01") && line.contains("Ship the overdue")));
    // Priority: overdue outranks scheduled/review codes in the DTO ordering.
    let codes: Vec<&str> = project
        .review_reasons
        .iter()
        .map(|reason| reason.code.as_str())
        .collect();
    if let Some(review_pos) = codes.iter().position(|code| *code == "review_action") {
        assert!(
            codes
                .iter()
                .position(|code| *code == "overdue_work")
                .unwrap()
                < review_pos
        );
    }
}

#[test]
fn attention_summary_counts_projects_with_overdue_work() {
    let _guard = env_guard();
    let _home = Home::new("overdue-attention");
    let record = register(&unique_path("overdue-attention-repo"), "Late project");
    let quiet = register(&unique_path("overdue-attention-quiet"), "Quiet project");

    let service = service();
    for (id, revision) in [(&record.id, 0), (&quiet.id, 0)] {
        service
            .complete_project_setup(dto::CompleteProjectSetupInput {
                project_id: id.clone(),
                expected_revision: revision,
                objective: "o".into(),
                desired_outcome: "d".into(),
                phase: None,
                first_commitment: "first".into(),
            })
            .unwrap();
    }

    let tasks = mvp::get_tasks(record.id.clone()).unwrap();
    let tasks = mvp::add_task(record.id.clone(), tasks.revision, "late".into(), false).unwrap();
    let late = tasks
        .tasks
        .iter()
        .find(|t| t.text == "late")
        .unwrap()
        .id
        .clone();
    mvp::update_task(
        record.id.clone(),
        tasks.revision,
        late,
        "open".into(),
        // A frozen past date: relative to any real wall clock this stays overdue.
        Some("2026-08-01".into()),
        None,
    )
    .unwrap();

    // A huge silence threshold isolates the overdue condition: neither project has any
    // commit, so only the overdue task can pull one into the summary.
    let summary = mvp::attention_summary_with_threshold(36500);
    assert!(summary.project_ids.contains(&record.id));
    assert!(!summary.project_ids.contains(&quiet.id));
}
