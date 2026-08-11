use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use omniproj_core::{ensure_home, load_project, ProjectId, StoreError};

const PROJECT_ID: &str = "b8a9e19ef3c91245";
const CREATED_AT: &str = "2026-08-10T12:00:00Z";
const LEGACY_PATH: &str = "/Users/research/legacy-project";
const LEGACY_NEXT: &str = "# Next\n\n- [ ] Preserve this Human note.\n";
const LEGACY_PLAN: &str = "# Plan\n\nA hand-authored plan.\n";
const LEGACY_BRIEFING: &str = "# Briefing\n\nAgent-authored legacy briefing.\n";

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
        "+++\nschema_version = 1\nrevision = 0\nstatus = \"setup\"\nstatus_changed_at = \"2026-08-10T12:00:00Z\"\ncreated_at = \"2026-08-10T12:00:00Z\"\nupdated_at = \"2026-08-10T12:00:00Z\"\nwork_items = []\ncommitment_transitions = []\n+++\n\n# Project notes\n"
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
