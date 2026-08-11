#![allow(deprecated)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use omniproj_core::{
    canonical_source_owner, ensure_home, find_project_by_cwd, list_project_records, list_projects,
    load_meta, load_project, record_source_observation, register_project,
    relink_primary_git_source, Cadence, CaptureCursor, ProjectId, ProjectRecord, ProjectSource,
    ProjectSourceId, ProjectSourceKind, ProjectSourceStatus, ProjectStoreError,
    RecordSourceObservationInput, RegisterOutcome, RegisterProjectInput, RelinkSourceInput,
    SourceObservationOutcome, StoreError,
};

const CREATED_AT: &str = "2026-08-10T12:00:00Z";

fn unique_path(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "omniproj-registry-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

fn env_guard() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn setup(tag: &str) -> (PathBuf, PathBuf) {
    let home = unique_path(&format!("{tag}-home"));
    let source = unique_path(&format!("{tag}-source"));
    std::fs::create_dir_all(&source).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);
    ensure_home().unwrap();
    (home, source)
}

fn register(source: &Path, name: &str) -> ProjectRecord {
    match register_project(RegisterProjectInput {
        location: source,
        name,
        created_at: CREATED_AT,
    })
    .unwrap()
    {
        RegisterOutcome::Created(project) => project,
        RegisterOutcome::Existing(id) => panic!("expected creation, got existing {id}"),
    }
}

fn cleanup(home: PathBuf, sources: impl IntoIterator<Item = PathBuf>) {
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
    for source in sources {
        std::fs::remove_dir_all(source).unwrap();
    }
}

#[cfg(unix)]
fn install_failing_commit_hook(home: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let hook = home.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho forced audit failure >&2\nexit 1\n").unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();
    hook
}

fn rewrite_pending_audit_as_round2(home: &Path) {
    let path = home.join(".git/omniproj-pending-audit.toml");
    let mut document: toml::Value = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    for target in document.get_mut("targets").unwrap().as_array_mut().unwrap() {
        let target = target.as_table_mut().unwrap();
        let prior = target.remove("prior").unwrap();
        let prior = prior.as_table().unwrap();
        match prior.get("kind").and_then(toml::Value::as_str) {
            Some("missing") => {}
            Some("regular_file") => {
                target.insert("prior_sha256".into(), prior.get("sha256").unwrap().clone());
            }
            other => panic!("unexpected generated prior identity {other:?}"),
        }
        let expected = target.remove("expected").unwrap();
        let expected = expected.as_table().unwrap();
        assert_eq!(
            expected.get("kind").and_then(toml::Value::as_str),
            Some("regular_file")
        );
        target.insert(
            "expected_sha256".into(),
            expected.get("sha256").unwrap().clone(),
        );
    }
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

#[test]
fn v2_registry_envelope_serializes_snake_case_and_exposes_primary_source() {
    let project_id = ProjectId::parse("project-2026").unwrap();
    let source_id = ProjectSourceId::parse("source-2026").unwrap();
    let record = ProjectRecord {
        id: project_id.clone(),
        name: "Cancer Imaging".into(),
        created_at: "2026-08-10T12:00:00Z".into(),
        sources: vec![ProjectSource {
            id: source_id,
            project_id,
            kind: ProjectSourceKind::GitRepo,
            location: "/research/cancer-imaging".into(),
            is_primary: true,
            status: ProjectSourceStatus::Available,
            created_at: "2026-08-10T12:00:00Z".into(),
            last_observed_at: None,
            last_successful_refresh_at: None,
            last_error_category: None,
            revision: 0,
        }],
        capture_cursor: CaptureCursor {
            last_distilled: Some("2026-08-10T13:00:00Z".into()),
            last_head: Some("abc123".into()),
            last_status_digest: Some("clean".into()),
            last_session_mtime: Some(42.5),
        },
        cadence: Some(Cadence {
            refresh_floor_secs: Some(3600),
            depth: Some("deep".into()),
        }),
    };

    let text = toml::to_string_pretty(&record).unwrap();

    assert!(text.contains("kind = \"git_repo\""));
    assert!(text.contains("status = \"available\""));
    assert_eq!(record.storage_id(), &record.id);
    assert_eq!(
        record.primary_git_source().unwrap().location,
        "/research/cancer-imaging"
    );
    assert_eq!(toml::from_str::<ProjectRecord>(&text).unwrap(), record);
}

#[test]
fn registration_is_atomic_and_duplicate_canonical_source_returns_existing_id() {
    let _guard = env_guard();
    let (home, source) = setup("register");

    let created = register(&source, "First name");
    let duplicate = register_project(RegisterProjectInput {
        location: &source.join("."),
        name: "Ignored replacement name",
        created_at: "2026-08-11T12:00:00Z",
    })
    .unwrap();

    assert!(matches!(
        duplicate,
        RegisterOutcome::Existing(ref id) if id == &created.id
    ));
    assert_eq!(list_project_records().unwrap(), vec![created.clone()]);
    let legacy = list_projects();
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].hash, created.id.as_str());
    assert_eq!(
        legacy[0].path,
        created.primary_git_source().unwrap().location
    );
    assert_eq!(load_meta(created.id.as_str()), Some(legacy[0].clone()));
    assert!(home
        .join("projects")
        .join(created.id.as_str())
        .join("notes/project.md")
        .exists());
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![
            format!("projects/{}/meta.toml", created.id),
            format!("projects/{}/notes/project.md", created.id),
        ]
    );

    cleanup(home, [source]);
}

#[test]
fn canonical_owner_lookup_is_read_only() {
    let _guard = env_guard();
    let (home, source) = setup("owner");
    let created = register(&source, "Owner");
    let head_before = git_output(&home, &["rev-parse", "HEAD"]);
    let status_before = git_output(&home, &["status", "--short"]);

    let owner = canonical_source_owner(&source).unwrap();

    assert_eq!(owner, Some(created.id));
    assert_eq!(git_output(&home, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git_output(&home, &["status", "--short"]), status_before);
    cleanup(home, [source]);
}

#[test]
fn loading_v2_metadata_rejects_duplicate_sources_bad_timestamps_and_incoherent_fields() {
    let _guard = env_guard();
    let (home, source_path) = setup("strict-v2-record");
    let valid = register(&source_path, "Strict record");
    let meta_path = home
        .join("projects")
        .join(valid.id.as_str())
        .join("meta.toml");

    let mut invalid_records = Vec::new();

    let mut duplicate_source = valid.clone();
    let mut duplicate = duplicate_source.sources[0].clone();
    duplicate.is_primary = false;
    duplicate_source.sources.push(duplicate);
    invalid_records.push(("duplicate source id", duplicate_source));

    let mut bad_project_timestamp = valid.clone();
    bad_project_timestamp.created_at = "not-rfc3339".into();
    invalid_records.push(("project created_at", bad_project_timestamp));

    let mut bad_source_timestamp = valid.clone();
    bad_source_timestamp.sources[0].last_observed_at = Some("not-rfc3339".into());
    invalid_records.push(("source last_observed_at", bad_source_timestamp));

    let mut empty_name = valid.clone();
    empty_name.name.clear();
    invalid_records.push(("empty project name", empty_name));

    let mut empty_location = valid.clone();
    empty_location.sources[0].location.clear();
    invalid_records.push(("empty source location", empty_location));

    let mut available_with_error = valid.clone();
    available_with_error.sources[0].last_error_category = Some("stale_error".into());
    invalid_records.push(("available source with error", available_with_error));

    let mut missing_without_error = valid.clone();
    missing_without_error.sources[0].status = ProjectSourceStatus::Missing;
    invalid_records.push(("missing source without error", missing_without_error));

    for (case, record) in invalid_records {
        std::fs::write(&meta_path, toml::to_string_pretty(&record).unwrap()).unwrap();
        assert!(
            matches!(
                load_project(&valid.id),
                Err(ProjectStoreError::InvalidRecord { .. })
            ),
            "{case} was accepted"
        );
    }

    cleanup(home, [source_path]);
}

#[test]
fn registration_failpoints_never_expose_a_partial_project_and_retry_succeeds() {
    let _guard = env_guard();
    for failpoint in [
        "registration_after_project_state_write",
        "registration_after_metadata_write",
        "registration_before_directory_rename",
    ] {
        let (home, source) = setup(failpoint);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);

        assert!(register_project(RegisterProjectInput {
            location: &source,
            name: "Interrupted",
            created_at: CREATED_AT,
        })
        .is_err());
        assert!(list_project_records().unwrap().is_empty());

        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        let created = register(&source, "Retried");
        assert_eq!(list_project_records().unwrap(), vec![created]);
        cleanup(home, [source]);
    }
}

#[test]
fn registration_rename_failure_clears_prepared_audit_and_retry_converges() {
    let _guard = env_guard();
    let (home, source) = setup("registration-rename-failure");
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "registration_directory_rename_failure",
    );

    assert!(register_project(RegisterProjectInput {
        location: &source,
        name: "Rename failure",
        created_at: CREATED_AT,
    })
    .is_err());
    assert!(list_project_records().unwrap().is_empty());
    assert!(home.join(".git/omniproj-pending-audit.toml").exists());

    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    ensure_home().unwrap();
    assert!(!home.join(".git/omniproj-pending-audit.toml").exists());
    assert!(list_project_records().unwrap().is_empty());
    let created = register(&source, "Rename retry");
    let root = home.join("projects").join(created.id.as_str());
    for required in ["auto", "notes", "cache"] {
        assert!(root.join(required).is_dir());
    }
    cleanup(home, [source]);
}

#[test]
fn registration_parent_fsync_failure_recognizes_applied_rename_and_audits_without_replay() {
    let _guard = env_guard();
    let (home, source) = setup("registration-parent-fsync-failure");
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "registration_parent_fsync_failure",
    );

    assert!(register_project(RegisterProjectInput {
        location: &source,
        name: "Parent fsync failure",
        created_at: CREATED_AT,
    })
    .is_err());
    let durable = list_project_records().unwrap();
    assert_eq!(durable.len(), 1, "rename must already be visible");
    assert!(home.join(".git/omniproj-pending-audit.toml").exists());

    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    ensure_home().unwrap();
    assert!(!home.join(".git/omniproj-pending-audit.toml").exists());
    assert_eq!(list_project_records().unwrap(), durable);
    let retry = register_project(RegisterProjectInput {
        location: &source,
        name: "must stay existing",
        created_at: CREATED_AT,
    })
    .unwrap();
    assert!(matches!(retry, RegisterOutcome::Existing(ref id) if id == &durable[0].id));
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![
            format!("projects/{}/meta.toml", durable[0].id),
            format!("projects/{}/notes/project.md", durable[0].id),
        ]
    );
    cleanup(home, [source]);
}

#[cfg(unix)]
#[test]
fn round2_pending_audits_resume_from_prepared_and_applied() {
    let _guard = env_guard();

    let (prepared_home, prepared_source) = setup("round2-pending-prepared");
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "registration_directory_rename_failure",
    );
    assert!(register_project(RegisterProjectInput {
        location: &prepared_source,
        name: "Round-2 prepared",
        created_at: CREATED_AT,
    })
    .is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    rewrite_pending_audit_as_round2(&prepared_home);

    ensure_home().unwrap();

    assert!(!prepared_home
        .join(".git/omniproj-pending-audit.toml")
        .exists());
    assert!(list_project_records().unwrap().is_empty());
    cleanup(prepared_home, [prepared_source]);

    let (applied_home, applied_source) = setup("round2-pending-applied");
    let hook = install_failing_commit_hook(&applied_home);
    assert!(register_project(RegisterProjectInput {
        location: &applied_source,
        name: "Round-2 applied",
        created_at: CREATED_AT,
    })
    .is_err());
    std::fs::remove_file(hook).unwrap();
    rewrite_pending_audit_as_round2(&applied_home);

    ensure_home().unwrap();

    assert!(!applied_home
        .join(".git/omniproj-pending-audit.toml")
        .exists());
    assert_eq!(list_project_records().unwrap().len(), 1);
    cleanup(applied_home, [applied_source]);
}

#[cfg(unix)]
#[test]
fn registration_audit_failure_is_recovered_without_replaying_or_staging_human_edits() {
    let _guard = env_guard();
    let (home, source) = setup("register-audit-recovery");
    std::fs::write(home.join("human-draft.md"), b"Human draft bytes\n").unwrap();
    let hook = install_failing_commit_hook(&home);

    let error = register_project(RegisterProjectInput {
        location: &source,
        name: "Audit recovery",
        created_at: CREATED_AT,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        ProjectStoreError::Store(StoreError::AuditCommit(_))
    ));
    let durable = list_project_records().unwrap();
    assert_eq!(durable.len(), 1, "the renamed project must remain durable");
    assert!(home.join(".git/omniproj-pending-audit.toml").exists());
    std::fs::remove_file(hook).unwrap();

    ensure_home().unwrap();
    let retried = register_project(RegisterProjectInput {
        location: &source,
        name: "must not replace",
        created_at: CREATED_AT,
    })
    .unwrap();
    assert!(matches!(retried, RegisterOutcome::Existing(ref id) if id == &durable[0].id));
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![
            format!("projects/{}/meta.toml", durable[0].id),
            format!("projects/{}/notes/project.md", durable[0].id),
        ]
    );
    assert_eq!(
        std::fs::read(home.join("human-draft.md")).unwrap(),
        b"Human draft bytes\n"
    );
    assert!(git_output(&home, &["status", "--short", "--", "human-draft.md"]).starts_with("??"));
    assert!(!home.join(".git/omniproj-pending-audit.toml").exists());
    cleanup(home, [source]);
}

#[cfg(unix)]
#[test]
fn registration_audit_recovery_rejects_a_same_path_human_edit_before_git_add() {
    let _guard = env_guard();
    let (home, source) = setup("register-same-path-audit-conflict");
    std::fs::write(home.join("human-draft.md"), b"Unrelated Human draft\n").unwrap();
    let hook = install_failing_commit_hook(&home);

    assert!(register_project(RegisterProjectInput {
        location: &source,
        name: "Snapshot recovery",
        created_at: CREATED_AT,
    })
    .is_err());
    let durable = list_project_records().unwrap().pop().unwrap();
    let relative_state = format!("projects/{}/notes/project.md", durable.id);
    let state = home.join(&relative_state);
    let human_edit = b"Human replaced the pending audit target\n";
    std::fs::write(&state, human_edit).unwrap();
    std::fs::remove_file(hook).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(std::fs::read(&state).unwrap(), human_edit);
    assert_ne!(
        git_output(&home, &["show", &format!(":{relative_state}")]).as_bytes(),
        human_edit,
        "recovery must not stage the Human replacement"
    );
    assert!(git_output(&home, &["status", "--short", "--", "human-draft.md"]).starts_with("??"));
    assert!(home.join(".git/omniproj-pending-audit.toml").exists());
    cleanup(home, [source]);
}

#[cfg(unix)]
#[test]
fn relink_audit_failure_recovers_exact_metadata_and_stale_retry_stays_a_conflict() {
    let _guard = env_guard();
    let (home, old_source) = setup("relink-audit-recovery");
    let new_source = unique_path("relink-audit-recovery-new");
    std::fs::create_dir_all(&new_source).unwrap();
    let created = register(&old_source, "Relink recovery");
    let state = home
        .join("projects")
        .join(created.id.as_str())
        .join("notes/project.md");
    std::fs::write(&state, b"Human state edit\n").unwrap();
    let source = created.primary_git_source().unwrap().clone();
    let hook = install_failing_commit_hook(&home);

    let error = relink_primary_git_source(RelinkSourceInput {
        project_id: &created.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        new_location: &new_source,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ProjectStoreError::Store(StoreError::AuditCommit(_))
    ));
    assert_eq!(
        load_project(&created.id)
            .unwrap()
            .primary_git_source()
            .unwrap()
            .revision,
        1
    );
    std::fs::remove_file(hook).unwrap();

    let retry = relink_primary_git_source(RelinkSourceInput {
        project_id: &created.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        new_location: &new_source,
    })
    .unwrap_err();
    assert!(matches!(retry, ProjectStoreError::RevisionConflict { .. }));
    ensure_home().unwrap();
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![format!("projects/{}/meta.toml", created.id)]
    );
    assert_eq!(std::fs::read(&state).unwrap(), b"Human state edit\n");
    assert!(git_output(
        &home,
        &[
            "status",
            "--short",
            "--",
            &format!("projects/{}/notes/project.md", created.id)
        ]
    )
    .starts_with(" M"));
    cleanup(home, [old_source, new_source]);
}

#[cfg(unix)]
#[test]
fn observation_audit_failure_recovers_exact_metadata_without_replaying_the_cas_update() {
    let _guard = env_guard();
    let (home, source_path) = setup("observation-audit-recovery");
    let created = register(&source_path, "Observation recovery");
    let source = created.primary_git_source().unwrap().clone();
    std::fs::write(home.join("human-draft.md"), b"Human draft bytes\n").unwrap();
    let hook = install_failing_commit_hook(&home);

    let error = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        attempted_at: "2026-08-10T13:00:00Z",
        outcome: SourceObservationOutcome::Failure {
            status: ProjectSourceStatus::Missing,
            error_category: "source_missing",
        },
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ProjectStoreError::Store(StoreError::AuditCommit(_))
    ));
    assert_eq!(
        load_project(&created.id)
            .unwrap()
            .primary_git_source()
            .unwrap()
            .revision,
        1
    );
    std::fs::remove_file(hook).unwrap();

    ensure_home().unwrap();
    let retry = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        attempted_at: "2026-08-10T13:00:00Z",
        outcome: SourceObservationOutcome::Failure {
            status: ProjectSourceStatus::Missing,
            error_category: "source_missing",
        },
    })
    .unwrap_err();
    assert!(matches!(retry, ProjectStoreError::RevisionConflict { .. }));
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![format!("projects/{}/meta.toml", created.id)]
    );
    assert!(git_output(&home, &["status", "--short", "--", "human-draft.md"]).starts_with("??"));
    cleanup(home, [source_path]);
}

#[test]
fn startup_removes_only_recognized_empty_staging_directories() {
    let _guard = env_guard();
    let (home, source) = setup("staging-cleanup");
    let empty = home
        .join("projects")
        .join(".staging-019fee85-f6cd-79e3-b8da-68b08c672e25");
    let nonempty = home
        .join("projects")
        .join(".staging-019fee85-f6cd-79e3-b8da-68b08c672e26");
    let unrecognized = home.join("projects/.staging-not-an-id!");
    for subdir in ["auto", "notes", "cache"] {
        std::fs::create_dir_all(empty.join(subdir)).unwrap();
    }
    std::fs::create_dir_all(&nonempty).unwrap();
    std::fs::write(nonempty.join("meta.toml"), b"partial").unwrap();
    std::fs::create_dir_all(&unrecognized).unwrap();

    ensure_home().unwrap();

    assert!(!empty.exists());
    assert!(nonempty.exists());
    assert!(unrecognized.exists());
    cleanup(home, [source]);
}

#[test]
fn relink_preserves_identity_and_human_files_and_updates_cwd_lookup() {
    let _guard = env_guard();
    let (home, old_source) = setup("relink");
    let new_source = unique_path("relink-new");
    std::fs::create_dir_all(new_source.join("nested/work")).unwrap();
    let created = register(&old_source, "Relinked");
    let root = home.join("projects").join(created.id.as_str());
    let project_notes = root.join("notes/project.md");
    let legacy_next = root.join("notes/next.md");
    let legacy_plan = root.join("plan.md");
    std::fs::write(&project_notes, b"Human-edited project state bytes\n").unwrap();
    std::fs::write(&legacy_next, b"Legacy next bytes\n").unwrap();
    std::fs::write(&legacy_plan, b"Legacy plan bytes\n").unwrap();
    let source = created.primary_git_source().unwrap();

    let relinked = relink_primary_git_source(RelinkSourceInput {
        project_id: &created.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        new_location: &new_source,
    })
    .unwrap();

    assert_eq!(relinked.id, created.id);
    assert_eq!(relinked.primary_git_source().unwrap().revision, 1);
    assert_eq!(
        relinked.primary_git_source().unwrap().location,
        std::fs::canonicalize(&new_source)
            .unwrap()
            .to_string_lossy()
    );
    assert_eq!(
        std::fs::read(&project_notes).unwrap(),
        b"Human-edited project state bytes\n"
    );
    assert_eq!(std::fs::read(&legacy_next).unwrap(), b"Legacy next bytes\n");
    assert_eq!(std::fs::read(&legacy_plan).unwrap(), b"Legacy plan bytes\n");
    assert!(find_project_by_cwd(&new_source.join("nested/work"))
        .unwrap()
        .is_some_and(|project| project.id == created.id));
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![format!("projects/{}/meta.toml", created.id)]
    );
    cleanup(home, [old_source, new_source]);
}

#[test]
fn relink_rejects_source_collision_and_stale_revision_without_mutation() {
    let _guard = env_guard();
    let (home, first_source) = setup("collision");
    let second_source = unique_path("collision-second");
    std::fs::create_dir_all(&second_source).unwrap();
    let first = register(&first_source, "First");
    let second = register(&second_source, "Second");
    let before = std::fs::read(
        home.join("projects")
            .join(first.id.as_str())
            .join("meta.toml"),
    )
    .unwrap();
    let source = first.primary_git_source().unwrap();

    let collision = relink_primary_git_source(RelinkSourceInput {
        project_id: &first.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        new_location: &second_source,
    })
    .unwrap_err();
    assert!(matches!(
        collision,
        ProjectStoreError::DuplicateSource { existing_project_id } if existing_project_id == second.id
    ));

    let stale = relink_primary_git_source(RelinkSourceInput {
        project_id: &first.id,
        expected_source_revision: source.revision + 1,
        expected_location: &source.location,
        new_location: &first_source,
    })
    .unwrap_err();
    assert!(matches!(stale, ProjectStoreError::RevisionConflict { .. }));
    assert_eq!(
        std::fs::read(
            home.join("projects")
                .join(first.id.as_str())
                .join("meta.toml")
        )
        .unwrap(),
        before
    );
    cleanup(home, [first_source, second_source]);
}

#[test]
fn source_observation_uses_location_and_revision_cas_and_commits_only_metadata() {
    let _guard = env_guard();
    let (home, source_path) = setup("observation");
    let created = register(&source_path, "Observed");
    let source = created.primary_git_source().unwrap().clone();

    let observed = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        attempted_at: "2026-08-10T13:00:00Z",
        outcome: SourceObservationOutcome::Success {
            successful_refresh_at: "2026-08-10T13:00:01Z",
        },
    })
    .unwrap();
    let updated = observed.primary_git_source().unwrap();
    assert_eq!(updated.revision, 1);
    assert_eq!(updated.status, ProjectSourceStatus::Available);
    assert_eq!(
        updated.last_observed_at.as_deref(),
        Some("2026-08-10T13:00:00Z")
    );
    assert_eq!(
        updated.last_successful_refresh_at.as_deref(),
        Some("2026-08-10T13:00:01Z")
    );

    let bytes = std::fs::read(
        home.join("projects")
            .join(created.id.as_str())
            .join("meta.toml"),
    )
    .unwrap();
    let stale = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: 0,
        expected_location: &source.location,
        attempted_at: "2026-08-10T14:00:00Z",
        outcome: SourceObservationOutcome::Failure {
            status: ProjectSourceStatus::Missing,
            error_category: "source_missing",
        },
    })
    .unwrap_err();
    assert!(matches!(stale, ProjectStoreError::RevisionConflict { .. }));
    assert_eq!(
        std::fs::read(
            home.join("projects")
                .join(created.id.as_str())
                .join("meta.toml")
        )
        .unwrap(),
        bytes
    );

    let moved = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: 1,
        expected_location: "/stale/caller/location",
        attempted_at: "2026-08-10T14:00:00Z",
        outcome: SourceObservationOutcome::Failure {
            status: ProjectSourceStatus::Moved,
            error_category: "source_moved",
        },
    })
    .unwrap_err();
    assert!(matches!(moved, ProjectStoreError::LocationConflict { .. }));
    assert_eq!(
        std::fs::read(
            home.join("projects")
                .join(created.id.as_str())
                .join("meta.toml")
        )
        .unwrap(),
        bytes
    );

    let failed = record_source_observation(RecordSourceObservationInput {
        project_id: &created.id,
        source_id: &source.id,
        expected_source_revision: 1,
        expected_location: &source.location,
        attempted_at: "2026-08-10T14:00:00Z",
        outcome: SourceObservationOutcome::Failure {
            status: ProjectSourceStatus::Missing,
            error_category: "source_missing",
        },
    })
    .unwrap();
    let failed_source = failed.primary_git_source().unwrap();
    assert_eq!(failed_source.revision, 2);
    assert_eq!(failed_source.status, ProjectSourceStatus::Missing);
    assert_eq!(
        failed_source.last_successful_refresh_at.as_deref(),
        Some("2026-08-10T13:00:01Z")
    );
    assert_eq!(
        failed_source.last_error_category.as_deref(),
        Some("source_missing")
    );
    assert_eq!(
        git_names(&home, "HEAD"),
        vec![format!("projects/{}/meta.toml", created.id)]
    );
    cleanup(home, [source_path]);
}

fn git_output(home: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(home)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn git_names(home: &Path, revision: &str) -> Vec<String> {
    let mut names: Vec<_> = git_output(home, &["show", "--format=", "--name-only", revision])
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}
