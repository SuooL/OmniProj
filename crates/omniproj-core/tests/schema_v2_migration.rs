use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use omniproj_core::{ensure_home, load_project, ProjectId, StoreError};
use sha2::{Digest, Sha256};

const PROJECT_ID: &str = "b8a9e19ef3c91245";
const CREATED_AT: &str = "2026-08-10T12:00:00Z";
const LEGACY_PATH: &str = "/Users/research/legacy-project";
const LEGACY_NEXT: &str = "# Next\n\n- [ ] Preserve this Human note.\n";
const LEGACY_PLAN: &str = "# Plan\n\nA hand-authored plan.\n";
const LEGACY_BRIEFING: &str = "# Briefing\n\nAgent-authored legacy briefing.\n";
const SETUP_STATE: &str = "+++\nschema_version = 1\nrevision = 0\nstatus = \"setup\"\nstatus_changed_at = \"2026-08-10T12:00:00Z\"\ncreated_at = \"2026-08-10T12:00:00Z\"\nupdated_at = \"2026-08-10T12:00:00Z\"\nwork_items = []\ncommitment_transitions = []\n+++\n\n# Project notes\n";
const SCHEMA_V2_SHA256: &str = "53c234e5e8472b6ac51c1ae1cab3fe06fad053beb8ebfd8977b010655bfdd3c3";

fn unique_home(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "omniproj-schema-v2-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

fn env_guard() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn run_git(home: &Path, args: &[&str]) {
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
}

fn seed_v1_store(home: &Path) {
    let project = home.join("projects").join(PROJECT_ID);
    std::fs::create_dir_all(project.join("notes")).unwrap();
    std::fs::create_dir_all(project.join("auto")).unwrap();
    std::fs::create_dir_all(project.join("cache")).unwrap();
    std::fs::write(home.join("SCHEMA_VERSION"), "1\n").unwrap();
    std::fs::write(
        home.join(".gitignore"),
        "# existing store ignore\nprojects/*/cache/\n",
    )
    .unwrap();
    std::fs::write(
        project.join("meta.toml"),
        format!(
            "path = {LEGACY_PATH:?}\nname = \"Legacy Project\"\nhash = {PROJECT_ID:?}\nadded_at = {CREATED_AT:?}\nlast_distilled = \"2026-08-10T13:00:00Z\"\nlast_head = \"abc123\"\nlast_status_digest = \"clean\"\nlast_session_mtime = 42.5\n\n[cadence]\nrefresh_floor_secs = 3600\ndepth = \"deep\"\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("notes/next.md"), LEGACY_NEXT).unwrap();
    std::fs::write(project.join("plan.md"), LEGACY_PLAN).unwrap();
    std::fs::write(project.join("auto/briefing.md"), LEGACY_BRIEFING).unwrap();

    run_git(home, &["init", "-q"]);
    run_git(home, &["add", "-A"]);
    run_git(
        home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "seed schema v1",
        ],
    );
}

fn add_v1_project(home: &Path, project_id: &str, location: &str) {
    let project = home.join("projects").join(project_id);
    std::fs::create_dir_all(project.join("notes")).unwrap();
    std::fs::create_dir_all(project.join("auto")).unwrap();
    std::fs::create_dir_all(project.join("cache")).unwrap();
    std::fs::write(
        project.join("meta.toml"),
        format!(
            "path = {location:?}\nname = \"Added During Migration\"\nhash = {project_id:?}\nadded_at = {CREATED_AT:?}\n"
        ),
    )
    .unwrap();
}

fn write_legacy_migration_journal(home: &Path, project_ids: &[&str]) {
    let ids = project_ids
        .iter()
        .map(|id| format!("{id:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        home.join(".migration-v2"),
        format!("target_schema_version = 2\nproject_ids = [{ids}]\n"),
    )
    .unwrap();
}

fn write_round1_migration_journal(home: &Path, phase: &str) {
    std::fs::write(
        home.join(".migration-v2"),
        format!(
            "target_schema_version = 2\nphase = {phase:?}\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\n"
        ),
    )
    .unwrap();
}

fn rewrite_snapshot_journal_as_round2(path: &Path, targets_key: &str, phase: Option<&str>) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    document
        .as_table_mut()
        .unwrap()
        .remove("preserved_state_proofs");
    if let Some(phase) = phase {
        document
            .as_table_mut()
            .unwrap()
            .insert("phase".into(), toml::Value::String(phase.into()));
    }
    for target in document
        .get_mut(targets_key)
        .unwrap()
        .as_array_mut()
        .unwrap()
    {
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

fn corrupt_tagged_expected_sha256(path: &Path, relative_path: &str, phase: Option<&str>) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    if let Some(phase) = phase {
        document
            .as_table_mut()
            .unwrap()
            .insert("phase".into(), toml::Value::String(phase.into()));
    }
    let target = document
        .get_mut("audit_targets")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|target| {
            target.get("relative_path").and_then(toml::Value::as_str) == Some(relative_path)
        })
        .unwrap()
        .as_table_mut()
        .unwrap();
    target
        .get_mut("expected")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert("sha256".into(), toml::Value::String("0".repeat(64)));
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

fn rewrite_tagged_journal_phase_without_targets(path: &Path, phase: &str) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    let table = document.as_table_mut().unwrap();
    table.insert("phase".into(), toml::Value::String(phase.into()));
    table.insert("audit_targets".into(), toml::Value::Array(Vec::new()));
    table.remove("pending_ignore_contents");
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

fn remove_preserved_state_proofs(path: &Path) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    assert!(document
        .as_table_mut()
        .unwrap()
        .remove("preserved_state_proofs")
        .is_some());
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

fn rewrite_preserved_state_head_policy(path: &Path, head_required: bool) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    let proofs = document
        .get_mut("preserved_state_proofs")
        .unwrap()
        .as_array_mut()
        .unwrap();
    assert_eq!(proofs.len(), 1);
    let prior = proofs[0]
        .as_table_mut()
        .unwrap()
        .insert("head_required".into(), toml::Value::Boolean(head_required));
    assert_eq!(
        prior.and_then(|value| value.as_bool()),
        Some(!head_required)
    );
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

fn rewrite_journal_as_ignore_audited_with_created_state(path: &Path) {
    let mut document: toml::Value = std::fs::read_to_string(path).unwrap().parse().unwrap();
    let table = document.as_table_mut().unwrap();
    table.insert("phase".into(), toml::Value::String("ignore_audited".into()));
    table.insert(
        "created_state_ids".into(),
        toml::Value::Array(vec![toml::Value::String(PROJECT_ID.into())]),
    );
    table.insert(
        "preserved_state_proofs".into(),
        toml::Value::Array(Vec::new()),
    );
    table.insert("audit_targets".into(), toml::Value::Array(Vec::new()));
    table.remove("pending_ignore_contents");
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

fn sha256(contents: &[u8]) -> String {
    format!("{:x}", Sha256::digest(contents))
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

#[test]
fn migrates_v1_store_to_v2_without_rewriting_legacy_documents() {
    let _guard = env_guard();
    let home = unique_home("basic");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);

    ensure_home().unwrap();

    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    let project = home.join("projects").join(PROJECT_ID);
    assert!(project.exists());
    let project_record = load_project(&ProjectId::parse(PROJECT_ID).unwrap()).unwrap();
    assert_eq!(project_record.id.as_str(), PROJECT_ID);
    assert_eq!(
        project_record.primary_git_source().unwrap().location,
        LEGACY_PATH
    );
    assert_eq!(
        std::fs::read_to_string(project.join("notes/next.md")).unwrap(),
        LEGACY_NEXT
    );
    assert_eq!(
        std::fs::read_to_string(project.join("plan.md")).unwrap(),
        LEGACY_PLAN
    );
    assert_eq!(
        std::fs::read_to_string(project.join("auto/briefing.md")).unwrap(),
        LEGACY_BRIEFING
    );
    assert_eq!(
        std::fs::read_to_string(project.join("notes/project.md")).unwrap(),
        SETUP_STATE
    );

    let before = managed_bytes(&home);
    ensure_home().unwrap();
    assert_eq!(managed_bytes(&home), before, "migration must be idempotent");

    assert_eq!(
        git_names(&home, "HEAD"),
        vec!["SCHEMA_VERSION"],
        "schema audit commit must contain only the stamp"
    );
    assert_eq!(
        git_names(&home, "HEAD^"),
        vec![
            format!("projects/{PROJECT_ID}/meta.toml"),
            format!("projects/{PROJECT_ID}/notes/project.md"),
        ],
        "project migration audit must contain only tool-managed paths"
    );

    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

fn managed_bytes(home: &Path) -> Vec<(String, Vec<u8>)> {
    [
        ".gitignore".to_owned(),
        "SCHEMA_VERSION".to_owned(),
        format!("projects/{PROJECT_ID}/meta.toml"),
        format!("projects/{PROJECT_ID}/notes/project.md"),
        format!("projects/{PROJECT_ID}/notes/next.md"),
        format!("projects/{PROJECT_ID}/plan.md"),
        format!("projects/{PROJECT_ID}/auto/briefing.md"),
    ]
    .into_iter()
    .map(|relative| {
        let bytes = std::fs::read(home.join(&relative)).unwrap();
        (relative, bytes)
    })
    .collect()
}

fn git_names(home: &Path, revision: &str) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(home)
        .args(["show", "--format=", "--name-only", revision])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut names: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

#[test]
fn migration_failpoints_resume_to_the_same_v2_files() {
    let _guard = env_guard();
    for failpoint in [
        "migration_after_journal_creation",
        "migration_after_project_state_write",
        "migration_after_metadata_write",
        "migration_after_project_audit_commit",
        "migration_after_schema_stamp",
        "migration_after_schema_audit_commit",
    ] {
        let home = unique_home(failpoint);
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);

        let first = ensure_home();
        assert!(first.is_err(), "{failpoint} must interrupt migration");
        assert!(home.join(".migration-v2").exists());

        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        ensure_home().unwrap();
        let converged = managed_bytes(&home);
        ensure_home().unwrap();
        assert_eq!(managed_bytes(&home), converged, "{failpoint} retry drifted");
        assert!(!home.join(".migration-v2").exists());
        assert_eq!(
            std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
            "2\n"
        );

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn schema_stamp_write_before_phase_advance_is_recoverable() {
    let _guard = env_guard();
    let home = unique_home("schema-write-before-phase");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_schema_stamp_write_before_phase",
    );

    assert!(ensure_home().is_err());
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    assert!(home.join(".migration-v2").exists());

    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    ensure_home().unwrap();
    assert!(!home.join(".migration-v2").exists());
    assert_eq!(git_names(&home, "HEAD"), vec!["SCHEMA_VERSION"]);
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn round2_snapshot_migration_journals_resume_from_every_write_phase() {
    let _guard = env_guard();
    for phase in ["ignore_write_prepared", "ignore_written"] {
        let home = unique_home(&format!("round2-{phase}"));
        seed_v1_store(&home);
        let hook = install_failing_commit_hook(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        assert!(matches!(ensure_home(), Err(StoreError::AuditCommit(_))));
        std::fs::remove_file(hook).unwrap();
        rewrite_snapshot_journal_as_round2(
            &home.join(".migration-v2"),
            "audit_targets",
            Some(phase),
        );

        ensure_home().unwrap();

        assert_eq!(std::fs::read(home.join("SCHEMA_VERSION")).unwrap(), b"2\n");
        assert!(!home.join(".migration-v2").exists());
        assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }

    for phase in ["projects_write_prepared", "projects_written"] {
        let home = unique_home(&format!("round2-{phase}"));
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        rewrite_snapshot_journal_as_round2(
            &home.join(".migration-v2"),
            "audit_targets",
            Some(phase),
        );

        ensure_home().unwrap();

        assert_eq!(std::fs::read(home.join("SCHEMA_VERSION")).unwrap(), b"2\n");
        assert!(!home.join(".migration-v2").exists());
        assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn round2_snapshot_compatibility_does_not_treat_a_symlink_as_a_regular_prior() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("round2-symlink-prior");
    seed_v1_store(&home);
    let gitignore = home.join(".gitignore");
    let external = home.join("Human-ignore");
    let prior = std::fs::read(&gitignore).unwrap();
    let mut expected = String::from_utf8(prior.clone()).unwrap();
    expected.push_str("/.migration-v2\n");
    std::fs::write(&external, &prior).unwrap();
    std::fs::remove_file(&gitignore).unwrap();
    symlink(&external, &gitignore).unwrap();
    let journal = format!(
        "target_schema_version = 2\nphase = \"ignore_write_prepared\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\npending_ignore_contents = {expected:?}\n\n[[audit_targets]]\nrelative_path = \".gitignore\"\nprior_sha256 = {:?}\nexpected_sha256 = {:?}\n",
        sha256(&prior),
        sha256(expected.as_bytes())
    );
    std::fs::write(home.join(".migration-v2"), &journal).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(std::fs::read_link(&gitignore).unwrap(), external);
    assert_eq!(std::fs::read(&external).unwrap(), prior);
    assert_eq!(
        std::fs::read_to_string(home.join(".migration-v2")).unwrap(),
        journal
    );
    assert_eq!(
        git_output(
            &home,
            &["diff", "--cached", "--name-only", "--", ".gitignore"]
        ),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn malformed_and_ambiguous_round2_snapshot_journals_are_not_staged_or_rewritten() {
    let _guard = env_guard();
    for target in [
        "relative_path = \".gitignore\"\nexpected_sha256 = \"not-a-hash\"\n".to_owned(),
        format!(
            "relative_path = \".gitignore\"\nexpected_sha256 = {SCHEMA_V2_SHA256:?}\n\n[audit_targets.prior]\nkind = \"missing\"\n"
        ),
    ] {
        let home = unique_home("malformed-round2-snapshot");
        seed_v1_store(&home);
        let journal = format!(
            "target_schema_version = 2\nphase = \"ignore_write_prepared\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\npending_ignore_contents = \"expected\\n\"\n\n[[audit_targets]]\n{target}"
        );
        std::fs::write(home.join(".migration-v2"), &journal).unwrap();
        let before = std::fs::read(home.join(".gitignore")).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);

        let error = ensure_home().unwrap_err();

        assert!(matches!(error, StoreError::InvalidData(_)));
        assert_eq!(std::fs::read(home.join(".gitignore")).unwrap(), before);
        assert_eq!(std::fs::read_to_string(home.join(".migration-v2")).unwrap(), journal);
        assert_eq!(
            git_output(&home, &["diff", "--cached", "--name-only", "--", ".gitignore"]),
            ""
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn round2_ignore_prepared_rejects_an_expected_hash_that_disagrees_with_its_write_bytes() {
    let _guard = env_guard();
    let home = unique_home("round2-ignore-authoritative-expected");
    seed_v1_store(&home);
    let gitignore = home.join(".gitignore");
    let prior = std::fs::read(&gitignore).unwrap();
    let mut expected = String::from_utf8(prior.clone()).unwrap();
    expected.push_str("/.migration-v2\n");
    let journal = format!(
        "target_schema_version = 2\nphase = \"ignore_write_prepared\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\npending_ignore_contents = {expected:?}\n\n[[audit_targets]]\nrelative_path = \".gitignore\"\nprior_sha256 = {:?}\nexpected_sha256 = {:?}\n",
        sha256(&prior),
        "0".repeat(64)
    );
    let journal_path = home.join(".migration-v2");
    std::fs::write(&journal_path, &journal).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
    ));
    assert_eq!(std::fs::read(&gitignore).unwrap(), prior);
    assert_eq!(std::fs::read_to_string(&journal_path).unwrap(), journal);
    assert_eq!(
        git_output(
            &home,
            &["diff", "--cached", "--name-only", "--", ".gitignore"]
        ),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn round2_projects_prepared_rejects_non_authoritative_meta_and_state_hashes_before_writes() {
    let _guard = env_guard();
    for corrupted_relative in [
        format!("projects/{PROJECT_ID}/meta.toml"),
        format!("projects/{PROJECT_ID}/notes/project.md"),
    ] {
        let home = unique_home("round2-project-authoritative-expected");
        seed_v1_store(&home);
        let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
        let state = home
            .join("projects")
            .join(PROJECT_ID)
            .join("notes/project.md");
        let prior_meta = std::fs::read(&meta).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        rewrite_snapshot_journal_as_round2(
            &home.join(".migration-v2"),
            "audit_targets",
            Some("projects_write_prepared"),
        );
        std::fs::write(&meta, &prior_meta).unwrap();
        std::fs::remove_file(&state).unwrap();
        let journal_path = home.join(".migration-v2");
        let mut document: toml::Value = std::fs::read_to_string(&journal_path)
            .unwrap()
            .parse()
            .unwrap();
        let target = document
            .get_mut("audit_targets")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|target| {
                target.get("relative_path").and_then(toml::Value::as_str)
                    == Some(corrupted_relative.as_str())
            })
            .unwrap()
            .as_table_mut()
            .unwrap();
        target.insert(
            "expected_sha256".into(),
            toml::Value::String("0".repeat(64)),
        );
        let journal = toml::to_string(&document).unwrap();
        std::fs::write(&journal_path, &journal).unwrap();

        let error = ensure_home().unwrap_err();

        assert!(matches!(
            error,
            StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
        ));
        assert_eq!(std::fs::read(&meta).unwrap(), prior_meta);
        assert!(!state.exists());
        assert_eq!(std::fs::read_to_string(&journal_path).unwrap(), journal);
        assert_eq!(
            git_output(
                &home,
                &[
                    "diff",
                    "--cached",
                    "--name-only",
                    "--",
                    &format!("projects/{PROJECT_ID}")
                ]
            ),
            ""
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn tagged_ignore_prepared_rejects_a_non_authoritative_expected_hash_before_writes() {
    let _guard = env_guard();
    let home = unique_home("tagged-ignore-authoritative-expected");
    seed_v1_store(&home);
    let gitignore = home.join(".gitignore");
    let prior = std::fs::read(&gitignore).unwrap();
    let hook = install_failing_commit_hook(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    assert!(matches!(ensure_home(), Err(StoreError::AuditCommit(_))));
    std::fs::remove_file(hook).unwrap();
    std::fs::write(&gitignore, &prior).unwrap();
    run_git(&home, &["reset", "-q", "HEAD", "--", ".gitignore"]);
    let journal_path = home.join(".migration-v2");
    corrupt_tagged_expected_sha256(&journal_path, ".gitignore", Some("ignore_write_prepared"));
    let journal = std::fs::read(&journal_path).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
    ));
    assert_eq!(std::fs::read(&gitignore).unwrap(), prior);
    assert_eq!(std::fs::read(&journal_path).unwrap(), journal);
    assert_eq!(
        git_output(
            &home,
            &["diff", "--cached", "--name-only", "--", ".gitignore"]
        ),
        ""
    );
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn tagged_projects_prepared_rejects_non_authoritative_expected_hashes_before_writes() {
    let _guard = env_guard();
    for corrupted_relative in [
        format!("projects/{PROJECT_ID}/meta.toml"),
        format!("projects/{PROJECT_ID}/notes/project.md"),
    ] {
        let home = unique_home("tagged-project-authoritative-expected");
        seed_v1_store(&home);
        let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
        let state = home
            .join("projects")
            .join(PROJECT_ID)
            .join("notes/project.md");
        let prior_meta = std::fs::read(&meta).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        std::fs::write(&meta, &prior_meta).unwrap();
        std::fs::remove_file(&state).unwrap();
        let journal_path = home.join(".migration-v2");
        corrupt_tagged_expected_sha256(&journal_path, &corrupted_relative, None);
        let journal = std::fs::read(&journal_path).unwrap();

        let error = ensure_home().unwrap_err();

        assert!(matches!(
            error,
            StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
        ));
        assert_eq!(std::fs::read(&meta).unwrap(), prior_meta);
        assert!(!state.exists());
        assert_eq!(std::fs::read(&journal_path).unwrap(), journal);
        assert_eq!(
            git_output(
                &home,
                &[
                    "diff",
                    "--cached",
                    "--name-only",
                    "--",
                    &format!("projects/{PROJECT_ID}")
                ]
            ),
            ""
        );
        assert_eq!(
            std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
            "1\n"
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn tagged_and_round2_projects_audited_require_proven_project_outputs() {
    let _guard = env_guard();
    let fixtures = [
        (
            "tagged",
            format!(
                "target_schema_version = 2\nphase = \"projects_audited\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\naudit_targets = []\n"
            ),
        ),
        (
            // No-target audited phases have the same wire representation in the tagged and
            // Round-2 schemas. This differently ordered fixture comes from the Round-2 shape.
            "round2",
            format!(
                "phase = \"projects_audited\"\ntarget_schema_version = 2\ncreated_state_ids = [{PROJECT_ID:?}]\nproject_ids = [{PROJECT_ID:?}]\naudit_targets = []\n"
            ),
        ),
    ];
    let mut failures = Vec::new();
    for (format, journal) in fixtures {
        for ignore_state in ["untouched", "audited"] {
            let home = unique_home(&format!("{format}-{ignore_state}-false-projects-audited"));
            seed_v1_store(&home);
            let gitignore = home.join(".gitignore");
            if ignore_state == "audited" {
                let mut audited_ignore = std::fs::read_to_string(&gitignore).unwrap();
                audited_ignore.push_str("/.migration-v2\n");
                std::fs::write(&gitignore, audited_ignore).unwrap();
                run_git(&home, &["add", "--", ".gitignore"]);
                run_git(
                    &home,
                    &[
                        "-c",
                        "user.name=omniproj-test",
                        "-c",
                        "user.email=omniproj-test@local",
                        "commit",
                        "-q",
                        "-m",
                        "audit migration ignore only",
                        "--",
                        ".gitignore",
                    ],
                );
            }
            let journal_path = home.join(".migration-v2");
            std::fs::write(&journal_path, &journal).unwrap();
            let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
            let state = home
                .join("projects")
                .join(PROJECT_ID)
                .join("notes/project.md");
            let meta_before = std::fs::read(&meta).unwrap();
            let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
            let ignore_before = std::fs::read(&gitignore).unwrap();
            std::env::set_var("OMNIPROJ_HOME", &home);

            let rejected = ensure_home().is_err();
            let unchanged = std::fs::read(&meta).ok().as_deref() == Some(meta_before.as_slice())
                && !state.exists()
                && std::fs::read(&journal_path).ok().as_deref() == Some(journal.as_bytes())
                && std::fs::read(home.join("SCHEMA_VERSION")).ok().as_deref()
                    == Some(schema_before.as_slice())
                && std::fs::read(home.join(".gitignore")).ok().as_deref()
                    == Some(ignore_before.as_slice())
                && git_output(&home, &["diff", "--cached", "--name-only"]).is_empty();
            if !rejected || !unchanged {
                failures.push(format!(
                    "{format}/{ignore_state}: rejected={rejected}, unchanged={unchanged}"
                ));
            }

            std::env::remove_var("OMNIPROJ_HOME");
            std::fs::remove_dir_all(home).unwrap();
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn ignore_audited_requires_the_expected_ignore_bytes_to_be_committed() {
    let _guard = env_guard();
    let home = unique_home("false-ignore-audited");
    seed_v1_store(&home);
    let gitignore = home.join(".gitignore");
    let mut expected = std::fs::read_to_string(&gitignore).unwrap();
    expected.push_str("/.migration-v2\n");
    std::fs::write(&gitignore, &expected).unwrap();
    let journal_path = home.join(".migration-v2");
    let journal = format!(
        "target_schema_version = 2\nphase = \"ignore_audited\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = [{PROJECT_ID:?}]\naudit_targets = []\n"
    );
    std::fs::write(&journal_path, &journal).unwrap();
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
    ));
    assert_eq!(std::fs::read(&gitignore).unwrap(), expected.as_bytes());
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert!(!home
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md")
        .exists());
    assert_eq!(std::fs::read(&journal_path).unwrap(), journal.as_bytes());
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn tagged_schema_audited_requires_exact_schema_bytes_in_worktree_and_head() {
    let _guard = env_guard();
    let mut failures = Vec::new();
    for case in ["uncommitted", "modified", "missing"] {
        let home = unique_home(&format!("false-schema-audited-{case}"));
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        let failpoint = if case == "uncommitted" {
            "migration_after_schema_stamp"
        } else {
            "migration_after_schema_audit_commit"
        };
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        let journal_path = home.join(".migration-v2");
        rewrite_tagged_journal_phase_without_targets(&journal_path, "schema_audited");
        let schema_path = home.join("SCHEMA_VERSION");
        match case {
            "uncommitted" => {}
            "modified" => std::fs::write(&schema_path, b"2 \n").unwrap(),
            "missing" => std::fs::remove_file(&schema_path).unwrap(),
            _ => unreachable!(),
        }
        let journal_before = std::fs::read(&journal_path).unwrap();
        let schema_before = std::fs::read(&schema_path).ok();
        let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

        let rejected = ensure_home().is_err();
        let unchanged = std::fs::read(&journal_path).ok().as_deref()
            == Some(journal_before.as_slice())
            && std::fs::read(&schema_path).ok() == schema_before
            && git_output(&home, &["diff", "--cached", "--name-only"]) == index_before;
        if !rejected || !unchanged {
            failures.push(format!(
                "{case}: rejected={rejected}, unchanged={unchanged}"
            ));
        }

        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn legitimate_tagged_audited_phase_journals_still_converge() {
    let _guard = env_guard();
    for failpoint in [
        "migration_after_project_audit_commit",
        "migration_after_schema_audit_commit",
    ] {
        let home = unique_home(&format!("legitimate-audited-{failpoint}"));
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

        ensure_home().unwrap();

        assert_eq!(std::fs::read(home.join("SCHEMA_VERSION")).unwrap(), b"2\n");
        assert!(!home.join(".migration-v2").exists());
        assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn migration_rejects_a_valid_project_id_symlink_root_without_following_it() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("project-root-symlink");
    let external = unique_home("project-root-symlink-external");
    seed_v1_store(&home);
    let project = home.join("projects").join(PROJECT_ID);
    std::fs::rename(&project, &external).unwrap();
    let external_meta = std::fs::read(external.join("meta.toml")).unwrap();
    symlink(&external, &project).unwrap();
    let gitignore = std::fs::read(home.join(".gitignore")).unwrap();
    let journal_path = home.join(".migration-v2");
    let journal = b"target_schema_version = 2\nphase = \"journal_created\"\nproject_ids = []\ncreated_state_ids = []\naudit_targets = []\n";
    std::fs::write(&journal_path, journal).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::MigrationConflict { .. }
    ));
    assert_eq!(std::fs::read_link(&project).unwrap(), external);
    assert_eq!(
        std::fs::read(external.join("meta.toml")).unwrap(),
        external_meta
    );
    assert!(!external.join("notes/project.md").exists());
    assert_eq!(std::fs::read(home.join(".gitignore")).unwrap(), gitignore);
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    assert_eq!(std::fs::read(&journal_path).unwrap(), journal);
    assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
    std::fs::remove_dir_all(external).unwrap();
}

#[test]
fn migration_rescan_rejects_a_valid_project_id_regular_file_before_mutation() {
    let _guard = env_guard();
    let home = unique_home("project-root-file-rescan");
    seed_v1_store(&home);
    let gitignore = std::fs::read(home.join(".gitignore")).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_journal_creation",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    let journal_path = home.join(".migration-v2");
    let journal = std::fs::read(&journal_path).unwrap();
    let file_id = "c8a9e19ef3c91246";
    let project_file = home.join("projects").join(file_id);
    std::fs::write(&project_file, b"Human project-shaped file\n").unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::MigrationConflict { .. }
    ));
    assert_eq!(
        std::fs::read(&project_file).unwrap(),
        b"Human project-shaped file\n"
    );
    assert_eq!(std::fs::read(home.join(".gitignore")).unwrap(), gitignore);
    assert_eq!(std::fs::read(&journal_path).unwrap(), journal);
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    assert_eq!(git_output(&home, &["diff", "--cached", "--name-only"]), "");
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn migration_rejects_a_notes_ancestor_symlink_before_writing_external_state() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("notes-ancestor-symlink");
    seed_v1_store(&home);
    let notes = home.join("projects").join(PROJECT_ID).join("notes");
    std::fs::remove_dir_all(&notes).unwrap();
    let external = unique_home("notes-ancestor-external");
    std::fs::create_dir(&external).unwrap();
    let sentinel = external.join("sentinel.md");
    std::fs::write(&sentinel, b"Human sentinel bytes\n").unwrap();
    symlink(&external, &notes).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert_eq!(std::fs::read(&sentinel).unwrap(), b"Human sentinel bytes\n");
    assert!(!external.join("project.md").exists());
    assert!(matches!(
        error,
        StoreError::InvalidData(_) | StoreError::AuditConflict { .. }
    ));
    assert_eq!(std::fs::read_link(&notes).unwrap(), external);
    assert_eq!(
        git_output(
            &home,
            &[
                "diff",
                "--cached",
                "--name-only",
                "--",
                &format!("projects/{PROJECT_ID}/notes/project.md")
            ]
        ),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
    std::fs::remove_dir_all(external).unwrap();
}

#[cfg(unix)]
#[test]
fn schema_write_prepared_rejects_a_persisted_symlink_prior_without_replacing_it() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("persisted-schema-symlink-prior");
    seed_v1_store(&home);
    let schema = home.join("SCHEMA_VERSION");
    let external = home.join("Human-schema");
    std::fs::write(&external, b"1\n").unwrap();
    std::fs::remove_file(&schema).unwrap();
    symlink(&external, &schema).unwrap();
    let journal = format!(
        "target_schema_version = 2\nphase = \"schema_write_prepared\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = []\n\n[[audit_targets]]\nrelative_path = \"SCHEMA_VERSION\"\n\n[audit_targets.prior]\nkind = \"symlink\"\ntarget = {external:?}\n\n[audit_targets.expected]\nkind = \"regular_file\"\nsha256 = {SCHEMA_V2_SHA256:?}\n"
    );
    std::fs::write(home.join(".migration-v2"), &journal).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::InvalidData(_)));
    assert_eq!(std::fs::read_link(&schema).unwrap(), external);
    assert_eq!(std::fs::read(&external).unwrap(), b"1\n");
    assert_eq!(
        std::fs::read_to_string(home.join(".migration-v2")).unwrap(),
        journal
    );
    assert_eq!(
        git_output(
            &home,
            &["diff", "--cached", "--name-only", "--", "SCHEMA_VERSION"]
        ),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn project_write_prepared_rejects_a_persisted_directory_prior_during_decode() {
    let _guard = env_guard();
    let home = unique_home("persisted-project-directory-prior");
    seed_v1_store(&home);
    let relative = format!("projects/{PROJECT_ID}/meta.toml");
    let meta = home.join(&relative);
    std::fs::remove_file(&meta).unwrap();
    std::fs::create_dir(&meta).unwrap();
    std::fs::write(meta.join("Human.md"), b"Human directory bytes\n").unwrap();
    let journal = format!(
        "target_schema_version = 2\nphase = \"projects_write_prepared\"\nproject_ids = [{PROJECT_ID:?}]\ncreated_state_ids = []\n\n[[audit_targets]]\nrelative_path = {relative:?}\n\n[audit_targets.prior]\nkind = \"directory\"\n\n[audit_targets.expected]\nkind = \"regular_file\"\nsha256 = {SCHEMA_V2_SHA256:?}\n"
    );
    std::fs::write(home.join(".migration-v2"), &journal).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::InvalidData(_)));
    assert_eq!(
        std::fs::read(meta.join("Human.md")).unwrap(),
        b"Human directory bytes\n"
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".migration-v2")).unwrap(),
        journal
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only", "--", &relative]),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn pending_audit_rejects_a_persisted_directory_prior_without_clearing_or_staging() {
    let _guard = env_guard();
    let home = unique_home("persisted-pending-directory-prior");
    std::env::set_var("OMNIPROJ_HOME", &home);
    ensure_home().unwrap();
    let relative = "tool-target";
    let target = home.join(relative);
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("Human.md"), b"Human directory bytes\n").unwrap();
    let journal = format!(
        "message = \"tampered mutation\"\nphase = \"prepared\"\n\n[[targets]]\nrelative_path = {relative:?}\n\n[targets.prior]\nkind = \"directory\"\n\n[targets.expected]\nkind = \"regular_file\"\nsha256 = {SCHEMA_V2_SHA256:?}\n"
    );
    let journal_path = home.join(".git/omniproj-pending-audit.toml");
    std::fs::write(&journal_path, &journal).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::InvalidData(_)));
    assert_eq!(
        std::fs::read(target.join("Human.md")).unwrap(),
        b"Human directory bytes\n"
    );
    assert_eq!(std::fs::read_to_string(&journal_path).unwrap(), journal);
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only", "--", relative]),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn schema_audit_recovery_rejects_same_path_human_bytes_before_git_add() {
    let _guard = env_guard();
    let home = unique_home("schema-same-path-audit-conflict");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_audit_commit",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    let hook = install_failing_commit_hook(&home);
    assert!(matches!(ensure_home(), Err(StoreError::AuditCommit(_))));
    std::fs::write(home.join("SCHEMA_VERSION"), "2 \n").unwrap();
    std::fs::remove_file(hook).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2 \n"
    );
    assert_eq!(git_output(&home, &["show", ":SCHEMA_VERSION"]), "2\n");
    assert!(home.join(".migration-v2").exists());
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn round1_projects_written_journal_resumes_when_outputs_are_verifiable() {
    let _guard = env_guard();
    let home = unique_home("round1-projects-written");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
    assert!(ensure_home().is_err());
    write_round1_migration_journal(&home, "projects_written");
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    ensure_home().unwrap();

    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    assert!(!home.join(".migration-v2").exists());
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn round1_post_project_and_schema_phases_resume_when_outputs_are_verifiable() {
    let _guard = env_guard();
    for (phase, failpoint) in [
        ("projects_audited", "migration_after_project_audit_commit"),
        (
            "schema_stamp_pending",
            "migration_after_project_audit_commit",
        ),
        ("schema_stamped", "migration_after_schema_stamp"),
    ] {
        let home = unique_home(&format!("round1-{phase}"));
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);
        assert!(ensure_home().is_err());
        write_round1_migration_journal(&home, phase);
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

        ensure_home().unwrap();

        assert_eq!(
            std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
            "2\n"
        );
        assert!(!home.join(".migration-v2").exists());
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn round1_ignore_audited_partial_write_resumes_only_deterministic_outputs() {
    let _guard = env_guard();
    let home = unique_home("round1-ignore-audited-partial");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
    assert!(ensure_home().is_err());
    write_round1_migration_journal(&home, "ignore_audited");
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    ensure_home().unwrap();

    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    assert!(!home.join(".migration-v2").exists());
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn round1_ignore_audited_rejects_a_valid_human_v2_edit_without_staging_it() {
    let _guard = env_guard();
    let home = unique_home("round1-ignore-audited-human-v2");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
    assert!(ensure_home().is_err());
    let relative_meta = format!("projects/{PROJECT_ID}/meta.toml");
    let meta = home.join(&relative_meta);
    let human = std::fs::read_to_string(&meta).unwrap().replacen(
        "name = \"Legacy Project\"",
        "name = \"Human v2 name\"",
        1,
    );
    std::fs::write(&meta, &human).unwrap();
    write_round1_migration_journal(&home, "ignore_audited");
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(std::fs::read_to_string(&meta).unwrap(), human);
    assert!(!git_output(&home, &["show", &format!(":{relative_meta}")]).contains("Human v2"));
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn migration_rejects_dangling_state_symlink_without_replacing_or_staging_it() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("dangling-state-symlink");
    seed_v1_store(&home);
    let relative = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative);
    symlink("missing-human-target", &state).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(
        std::fs::read_link(&state).unwrap(),
        PathBuf::from("missing-human-target")
    );
    assert_eq!(
        git_output(&home, &["status", "--short", "--", &relative]),
        "?? projects/b8a9e19ef3c91245/notes/project.md\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn migration_rejects_same_bytes_metadata_symlink_without_following_it() {
    use std::os::unix::fs::symlink;

    let _guard = env_guard();
    let home = unique_home("same-bytes-meta-symlink");
    seed_v1_store(&home);
    let relative = format!("projects/{PROJECT_ID}/meta.toml");
    let meta = home.join(&relative);
    let external = home.join("Human-meta.toml");
    let original = std::fs::read(&meta).unwrap();
    std::fs::write(&external, &original).unwrap();
    std::fs::remove_file(&meta).unwrap();
    symlink(&external, &meta).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(std::fs::read_link(&meta).unwrap(), external);
    assert_eq!(std::fs::read(&external).unwrap(), original);
    assert!(
        !git_output(&home, &["diff", "--cached", "--name-only", "--", &relative])
            .contains(&relative)
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn migration_rejects_directory_state_target_without_modifying_or_staging_it() {
    let _guard = env_guard();
    let home = unique_home("directory-state-target");
    seed_v1_store(&home);
    let relative = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative);
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("Human.md"), "Human directory contents\n").unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(
        std::fs::read_to_string(state.join("Human.md")).unwrap(),
        "Human directory contents\n"
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only", "--", &relative]),
        ""
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn legacy_migration_journals_resume_from_verifiable_interruption_states() {
    let _guard = env_guard();
    for failpoint in [
        "migration_after_journal_creation",
        "migration_after_project_audit_commit",
        "migration_after_schema_stamp",
    ] {
        let home = unique_home(&format!("legacy-journal-{failpoint}"));
        seed_v1_store(&home);
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var("OMNIPROJ_TEST_FAILPOINT", failpoint);
        assert!(ensure_home().is_err());
        write_legacy_migration_journal(&home, &[PROJECT_ID]);

        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        ensure_home().unwrap();

        assert_eq!(
            std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
            "2\n"
        );
        assert!(!home.join(".migration-v2").exists());
        assert!(load_project(&ProjectId::parse(PROJECT_ID).unwrap()).is_ok());
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn legacy_journal_preserves_preexisting_untracked_canonical_project_state() {
    let _guard = env_guard();
    let home = unique_home("legacy-journal-preexisting-canonical-state");
    seed_v1_store(&home);
    let project = home.join("projects").join(PROJECT_ID);
    let state = project.join("notes/project.md");
    let human = project.join("notes/Human.md");
    std::fs::write(&state, SETUP_STATE).unwrap();
    std::fs::write(&human, b"Human bytes outside migration scope\n").unwrap();
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    let state_before = std::fs::read(&state).unwrap();
    let human_before = std::fs::read(&human).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    ensure_home().unwrap();

    assert_eq!(std::fs::read(home.join("SCHEMA_VERSION")).unwrap(), b"2\n");
    assert!(!home.join(".migration-v2").exists());
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&human).unwrap(), human_before);
    assert_eq!(
        git_names(&home, "HEAD^"),
        vec![format!("projects/{PROJECT_ID}/meta.toml")],
        "only migrated metadata belongs in the project audit commit"
    );
    assert_eq!(
        git_output(
            &home,
            &[
                "status",
                "--short",
                "--",
                &format!("projects/{PROJECT_ID}/notes/project.md"),
                &format!("projects/{PROJECT_ID}/notes/Human.md"),
            ],
        ),
        format!(
            "?? projects/{PROJECT_ID}/notes/Human.md\n?? projects/{PROJECT_ID}/notes/project.md\n"
        )
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn normalized_legacy_journal_retains_exact_untracked_state_proof_across_retries() {
    let _guard = env_guard();
    let home = unique_home("normalized-legacy-untracked-state-proof");
    seed_v1_store(&home);
    let state = home
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::fs::write(&state, SETUP_STATE).unwrap();
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let noncanonical = SETUP_STATE.replacen("revision = 0", "revision=0", 1);
    assert_eq!(
        omniproj_core::ProjectStateDoc::parse(&noncanonical).unwrap(),
        omniproj_core::ProjectStateDoc::parse(SETUP_STATE).unwrap()
    );
    std::fs::write(&state, &noncanonical).unwrap();
    let journal = home.join(".migration-v2");
    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn normalized_audited_legacy_journal_retains_state_head_proof_across_retries() {
    let _guard = env_guard();
    let home = unique_home("normalized-legacy-audited-state-proof");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative_state);
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::fs::write(&state, SETUP_STATE).unwrap();
    run_git(&home, &["add", "--", &relative_state]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "seed canonical project state",
            "--",
            &relative_state,
        ],
    );
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_audit_commit",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_schema_stamp");
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    run_git(&home, &["rm", "-q", "--cached", "--", &relative_state]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "stop tracking canonical project state",
        ],
    );
    let journal = home.join(".migration-v2");
    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn tampered_false_head_policy_cannot_disable_audited_state_proof() {
    let _guard = env_guard();
    let home = unique_home("tampered-false-audited-state-policy");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative_state);
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::fs::write(&state, SETUP_STATE).unwrap();
    run_git(&home, &["add", "--", &relative_state]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "seed canonical project state",
            "--",
            &relative_state,
        ],
    );
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_audit_commit",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_schema_stamp");
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    let journal = home.join(".migration-v2");
    rewrite_preserved_state_head_policy(&journal, false);
    run_git(&home, &["rm", "-q", "--cached", "--", &relative_state]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "stop tracking canonical project state",
        ],
    );
    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn tampered_true_head_policy_cannot_invent_untracked_state_provenance() {
    let _guard = env_guard();
    let home = unique_home("tampered-true-untracked-state-policy");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative_state);
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::fs::write(&state, SETUP_STATE).unwrap();
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    let journal = home.join(".migration-v2");
    rewrite_preserved_state_head_policy(&journal, true);
    run_git(&home, &["add", "--", &relative_state]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "track state after migration normalization",
        ],
    );
    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn tampered_partition_cannot_reclassify_preserved_untracked_state_as_migration_created() {
    let _guard = env_guard();
    let home = unique_home("tampered-preserved-state-partition");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative_state);
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::fs::write(&state, SETUP_STATE).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let journal = home.join(".migration-v2");
    let normalized: toml::Value = std::fs::read_to_string(&journal).unwrap().parse().unwrap();
    let proofs = normalized
        .get("preserved_state_proofs")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(proofs.len(), 1);
    assert_eq!(
        proofs[0]
            .get("head_required")
            .and_then(toml::Value::as_bool),
        Some(false)
    );
    rewrite_journal_as_ignore_audited_with_created_state(&journal);

    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);
    let head_before = git_output(&home, &["rev-parse", "HEAD"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    assert_eq!(git_output(&home, &["rev-parse", "HEAD"]), head_before);
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn ignore_audited_retry_cannot_claim_human_recreated_canonical_state() {
    let _guard = env_guard();
    let home = unique_home("ignore-audited-human-recreated-state");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    let state = home.join(&relative_state);
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let journal = home.join(".migration-v2");
    rewrite_tagged_journal_phase_without_targets(&journal, "ignore_audited");
    std::fs::remove_file(&state).unwrap();
    std::fs::write(&state, SETUP_STATE).unwrap();

    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&state).unwrap();
    let meta_before = std::fs::read(&meta).unwrap();
    let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);
    let head_before = git_output(&home, &["rev-parse", "HEAD"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&state).unwrap(), state_before);
    assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
    assert_eq!(
        std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&home, &["diff", "--cached", "--name-only"]),
        index_before
    );
    assert_eq!(git_output(&home, &["rev-parse", "HEAD"]), head_before);
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn genuine_migration_created_state_still_recovers_and_is_audited() {
    let _guard = env_guard();
    let home = unique_home("genuine-created-state-recovery");
    seed_v1_store(&home);
    let relative_state = format!("projects/{PROJECT_ID}/notes/project.md");
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    ensure_home().unwrap();

    assert_eq!(std::fs::read(home.join("SCHEMA_VERSION")).unwrap(), b"2\n");
    assert!(!home.join(".migration-v2").exists());
    assert_eq!(
        std::fs::read(home.join(&relative_state)).unwrap(),
        SETUP_STATE.as_bytes()
    );
    assert!(git_names(&home, "HEAD^").contains(&relative_state));
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn prior_current_journal_without_proofs_upgrades_from_verifiable_state() {
    let _guard = env_guard();

    let recoverable = unique_home("prior-current-recoverable-state-proof");
    seed_v1_store(&recoverable);
    let recoverable_state = recoverable
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    std::fs::write(&recoverable_state, SETUP_STATE).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &recoverable);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_journal_creation",
    );
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    remove_preserved_state_proofs(&recoverable.join(".migration-v2"));

    ensure_home().unwrap();

    assert_eq!(
        std::fs::read(recoverable.join("SCHEMA_VERSION")).unwrap(),
        b"2\n"
    );
    assert_eq!(
        std::fs::read(&recoverable_state).unwrap(),
        SETUP_STATE.as_bytes()
    );
    assert_eq!(
        git_output(
            &recoverable,
            &[
                "status",
                "--short",
                "--",
                &format!("projects/{PROJECT_ID}/notes/project.md")
            ],
        ),
        format!("?? projects/{PROJECT_ID}/notes/project.md\n")
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(recoverable).unwrap();
}

#[test]
fn prior_current_audited_journal_without_proofs_rejects_ambiguous_state() {
    let _guard = env_guard();
    let ambiguous = unique_home("prior-current-ambiguous-state-proof");
    seed_v1_store(&ambiguous);
    let ambiguous_state = ambiguous
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    std::fs::write(&ambiguous_state, SETUP_STATE).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &ambiguous);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_schema_stamp");
    assert!(ensure_home().is_err());
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    let journal = ambiguous.join(".migration-v2");
    remove_preserved_state_proofs(&journal);
    let journal_before = std::fs::read(&journal).unwrap();
    let state_before = std::fs::read(&ambiguous_state).unwrap();
    let schema_before = std::fs::read(ambiguous.join("SCHEMA_VERSION")).unwrap();
    let index_before = git_output(&ambiguous, &["diff", "--cached", "--name-only"]);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
    assert_eq!(std::fs::read(&ambiguous_state).unwrap(), state_before);
    assert_eq!(
        std::fs::read(ambiguous.join("SCHEMA_VERSION")).unwrap(),
        schema_before
    );
    assert_eq!(
        git_output(&ambiguous, &["diff", "--cached", "--name-only"]),
        index_before
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(ambiguous).unwrap();
}

#[test]
fn malformed_preserved_state_proofs_are_rejected_before_rewrite() {
    let _guard = env_guard();
    for case in ["missing", "duplicate", "wrong-kind", "unknown-field"] {
        let home = unique_home(&format!("malformed-preserved-state-proof-{case}"));
        seed_v1_store(&home);
        let state = home
            .join("projects")
            .join(PROJECT_ID)
            .join("notes/project.md");
        let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
        std::fs::write(&state, SETUP_STATE).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        std::env::set_var(
            "OMNIPROJ_TEST_FAILPOINT",
            "migration_after_journal_creation",
        );
        assert!(ensure_home().is_err());
        std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
        let journal = home.join(".migration-v2");
        let mut document: toml::Value = std::fs::read_to_string(&journal).unwrap().parse().unwrap();
        let proofs = document
            .get_mut("preserved_state_proofs")
            .unwrap()
            .as_array_mut()
            .unwrap();
        assert_eq!(proofs.len(), 1);
        match case {
            "missing" => proofs.clear(),
            "duplicate" => proofs.push(proofs[0].clone()),
            "wrong-kind" => {
                let expected = proofs[0]
                    .get_mut("expected")
                    .unwrap()
                    .as_table_mut()
                    .unwrap();
                expected.insert("kind".into(), toml::Value::String("missing".into()));
                expected.remove("sha256");
            }
            "unknown-field" => {
                proofs[0]
                    .as_table_mut()
                    .unwrap()
                    .insert("unexpected".into(), toml::Value::Boolean(true));
            }
            _ => unreachable!(),
        }
        std::fs::write(&journal, toml::to_string(&document).unwrap()).unwrap();
        let journal_before = std::fs::read(&journal).unwrap();
        let state_before = std::fs::read(&state).unwrap();
        let meta_before = std::fs::read(&meta).unwrap();
        let schema_before = std::fs::read(home.join("SCHEMA_VERSION")).unwrap();
        let index_before = git_output(&home, &["diff", "--cached", "--name-only"]);

        let error = ensure_home().unwrap_err();

        assert!(
            matches!(error, StoreError::InvalidData(_)),
            "{case}: {error:?}"
        );
        assert_eq!(std::fs::read(&journal).unwrap(), journal_before);
        assert_eq!(std::fs::read(&state).unwrap(), state_before);
        assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
        assert_eq!(
            std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
            schema_before
        );
        assert_eq!(
            git_output(&home, &["diff", "--cached", "--name-only"]),
            index_before
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn noncanonical_legacy_journal_state_is_a_typed_conflict_and_is_not_rewritten() {
    let _guard = env_guard();
    let home = unique_home("ambiguous-legacy-journal");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_project_state_write",
    );
    assert!(ensure_home().is_err());
    write_legacy_migration_journal(&home, &[PROJECT_ID]);
    let state = home
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    let noncanonical = SETUP_STATE.replacen("revision = 0", "revision=0", 1);
    assert_eq!(
        omniproj_core::ProjectStateDoc::parse(&noncanonical).unwrap(),
        omniproj_core::ProjectStateDoc::parse(SETUP_STATE).unwrap()
    );
    std::fs::write(&state, &noncanonical).unwrap();
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&state).unwrap(), noncanonical.as_bytes());
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn malformed_legacy_journal_is_rejected_without_guessing_or_writes() {
    let _guard = env_guard();
    for journal in [
        format!("project_ids = [{PROJECT_ID:?}]\n"),
        format!("target_schema_version = 2\nproject_ids = [{PROJECT_ID:?}]\nunexpected = true\n"),
        format!("target_schema_version = 2\nproject_ids = [{PROJECT_ID:?}, {PROJECT_ID:?}]\n"),
    ] {
        let home = unique_home("malformed-legacy-journal");
        seed_v1_store(&home);
        std::fs::write(home.join(".migration-v2"), &journal).unwrap();
        let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
        let meta_before = std::fs::read(&meta).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);

        let error = ensure_home().unwrap_err();

        assert!(matches!(error, StoreError::InvalidData(_)));
        assert_eq!(std::fs::read(&meta).unwrap(), meta_before);
        assert_eq!(
            std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
            "1\n"
        );
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn migration_refuses_an_unrecognized_project_state_without_overwriting_it() {
    let _guard = env_guard();
    let home = unique_home("conflict");
    seed_v1_store(&home);
    let state = home
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    let unknown = b"# Existing Human project notes\n\nNever overwrite me.\n";
    std::fs::write(&state, unknown).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&state).unwrap(), unknown);
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );

    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn stale_journal_cannot_downgrade_a_newer_or_malformed_schema_stamp() {
    let _guard = env_guard();
    for stamp in ["3\n", "not-a-version\n"] {
        let home = unique_home("stale-journal-stamp");
        seed_v1_store(&home);
        std::fs::write(home.join("SCHEMA_VERSION"), stamp).unwrap();
        let journal = b"target_schema_version = 2\nproject_ids = []\n";
        std::fs::write(home.join(".migration-v2"), journal).unwrap();
        let gitignore_before = std::fs::read(home.join(".gitignore")).unwrap();
        let head_before = git_names(&home, "HEAD");
        std::env::set_var("OMNIPROJ_HOME", &home);

        let error = ensure_home().unwrap_err();

        assert!(matches!(error, StoreError::InvalidData(_)));
        assert_eq!(
            std::fs::read(home.join("SCHEMA_VERSION")).unwrap(),
            stamp.as_bytes()
        );
        assert_eq!(std::fs::read(home.join(".migration-v2")).unwrap(), journal);
        assert_eq!(
            std::fs::read(home.join(".gitignore")).unwrap(),
            gitignore_before
        );
        assert_eq!(git_names(&home, "HEAD"), head_before);
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn migration_retries_gitignore_audit_after_a_real_commit_failure() {
    let _guard = env_guard();
    let home = unique_home("gitignore-audit-retry");
    seed_v1_store(&home);
    let hook = install_failing_commit_hook(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);

    let first = ensure_home().unwrap_err();

    assert!(matches!(first, StoreError::AuditCommit(_)));
    assert!(home.join(".migration-v2").exists());
    std::fs::remove_file(hook).unwrap();
    ensure_home().unwrap();
    assert_eq!(
        git_output(&home, &["status", "--short", "--", ".gitignore"]),
        ""
    );
    assert!(
        git_output(&home, &["log", "--format=%s", "--", ".gitignore"])
            .contains("ignore v2 migration journal")
    );

    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn migration_gitignore_recovery_rejects_a_same_path_human_edit_before_git_add() {
    let _guard = env_guard();
    let home = unique_home("migration-gitignore-audit-conflict");
    seed_v1_store(&home);
    let hook = install_failing_commit_hook(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    assert!(matches!(ensure_home(), Err(StoreError::AuditCommit(_))));
    let mut human_edit = std::fs::read_to_string(home.join(".gitignore")).unwrap();
    human_edit.push_str("# Human ignore edit\n");
    std::fs::write(home.join(".gitignore"), &human_edit).unwrap();
    std::fs::remove_file(hook).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(
        std::fs::read_to_string(home.join(".gitignore")).unwrap(),
        human_edit
    );
    assert!(!git_output(&home, &["show", ":.gitignore"]).contains("Human ignore edit"));
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    assert!(home.join(".migration-v2").exists());
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn migration_audit_recovery_rejects_a_same_path_human_edit_before_git_add() {
    let _guard = env_guard();
    let home = unique_home("migration-same-path-audit-conflict");
    seed_v1_store(&home);
    let mut ignore = std::fs::read_to_string(home.join(".gitignore")).unwrap();
    ignore.push_str("/.migration-v2\n");
    std::fs::write(home.join(".gitignore"), ignore).unwrap();
    run_git(&home, &["add", "--", ".gitignore"]);
    run_git(
        &home,
        &[
            "-c",
            "user.name=omniproj-test",
            "-c",
            "user.email=omniproj-test@local",
            "commit",
            "-q",
            "-m",
            "seed migration ignore",
        ],
    );
    let hook = install_failing_commit_hook(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    assert!(matches!(ensure_home(), Err(StoreError::AuditCommit(_))));
    let relative_meta = format!("projects/{PROJECT_ID}/meta.toml");
    let meta = home.join(&relative_meta);
    let mut human_edit = std::fs::read_to_string(&meta).unwrap();
    human_edit = human_edit.replacen("name = \"Legacy Project\"", "name = \"Human rename\"", 1);
    std::fs::write(&meta, &human_edit).unwrap();
    std::fs::remove_file(hook).unwrap();

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::AuditConflict { .. }));
    assert_eq!(std::fs::read_to_string(&meta).unwrap(), human_edit);
    assert!(!git_output(&home, &["show", &format!(":{relative_meta}")]).contains("Human rename"));
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    assert!(home.join(".migration-v2").exists());
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn migration_rescans_projects_added_after_journal_creation() {
    let _guard = env_guard();
    let home = unique_home("journal-rescan");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var(
        "OMNIPROJ_TEST_FAILPOINT",
        "migration_after_journal_creation",
    );
    assert!(ensure_home().is_err());
    let added_id = "c8a9e19ef3c91246";
    add_v1_project(&home, added_id, "/Users/research/added-during-migration");

    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");
    ensure_home().unwrap();

    let added = ProjectId::parse(added_id).unwrap();
    assert_eq!(load_project(&added).unwrap().id, added);
    assert!(home
        .join("projects")
        .join(added_id)
        .join("notes/project.md")
        .exists());
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "2\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn migration_resume_rejects_malformed_v2_source_metadata() {
    let _guard = env_guard();
    let home = unique_home("malformed-v2-resume");
    seed_v1_store(&home);
    std::env::set_var("OMNIPROJ_HOME", &home);
    std::env::set_var("OMNIPROJ_TEST_FAILPOINT", "migration_after_metadata_write");
    assert!(ensure_home().is_err());
    let meta = home.join("projects").join(PROJECT_ID).join("meta.toml");
    let mut text = std::fs::read_to_string(&meta).unwrap();
    let start = text.rfind("created_at = ").unwrap();
    let end = start + text[start..].find('\n').unwrap();
    text.replace_range(start..end, "created_at = \"not-rfc3339\"");
    std::fs::write(&meta, &text).unwrap();
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    assert!(ensure_home().is_err());

    assert_eq!(std::fs::read_to_string(&meta).unwrap(), text);
    assert_eq!(
        std::fs::read_to_string(home.join("SCHEMA_VERSION")).unwrap(),
        "1\n"
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn migration_audits_only_project_state_created_by_that_migration() {
    let _guard = env_guard();
    let home = unique_home("preexisting-setup-state");
    seed_v1_store(&home);
    let state = home
        .join("projects")
        .join(PROJECT_ID)
        .join("notes/project.md");
    std::fs::write(&state, SETUP_STATE).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    ensure_home().unwrap();

    assert_eq!(
        git_names(&home, "HEAD^"),
        vec![format!("projects/{PROJECT_ID}/meta.toml")]
    );
    assert_eq!(
        git_output(
            &home,
            &[
                "status",
                "--short",
                "--",
                &format!("projects/{PROJECT_ID}/notes/project.md")
            ]
        ),
        format!("?? projects/{PROJECT_ID}/notes/project.md\n")
    );
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
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
