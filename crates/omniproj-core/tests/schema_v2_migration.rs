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
fn ambiguous_legacy_journal_state_is_a_typed_conflict_and_is_not_rewritten() {
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
    let before = std::fs::read(&state).unwrap();
    std::env::remove_var("OMNIPROJ_TEST_FAILPOINT");

    let error = ensure_home().unwrap_err();

    assert!(matches!(error, StoreError::MigrationConflict { .. }));
    assert_eq!(std::fs::read(&state).unwrap(), before);
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
