use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use omniproj_core::{
    apply_project_command, ensure_home, ProjectCommand, ProjectId, ProjectStateDoc,
    ProjectStateError, ProjectStatus, WorkItemStatus,
};

const AT_0: &str = "2026-08-10T12:00:00Z";
const AT_1: &str = "2026-08-10T13:00:00Z";
const AT_2: &str = "2026-08-10T14:00:00Z";
const AT_3: &str = "2026-08-10T15:00:00Z";
const AT_4: &str = "2026-08-10T16:00:00Z";

fn unique_home(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "omniproj-project-state-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

fn env_guard() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn state_path(home: &std::path::Path, project_id: &ProjectId) -> PathBuf {
    home.join("projects")
        .join(project_id.as_str())
        .join("notes/project.md")
}

struct TestStore {
    _guard: MutexGuard<'static, ()>,
    home: PathBuf,
    project_id: ProjectId,
}

impl TestStore {
    fn new(tag: &str) -> Self {
        let guard = env_guard();
        let home = unique_home(tag);
        let project_id = ProjectId::parse(format!("project-{tag}")).unwrap();
        std::env::set_var("OMNIPROJ_HOME", &home);
        ensure_home().unwrap();
        std::fs::create_dir_all(
            home.join("projects")
                .join(project_id.as_str())
                .join("notes"),
        )
        .unwrap();
        ProjectStateDoc::new_setup(AT_0)
            .unwrap()
            .save(&project_id)
            .unwrap();
        Self {
            _guard: guard,
            home,
            project_id,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        std::fs::read(state_path(&self.home, &self.project_id)).unwrap()
    }

    fn load(&self) -> ProjectStateDoc {
        ProjectStateDoc::load(&self.project_id).unwrap()
    }

    fn apply(
        &self,
        expected_revision: u64,
        command: ProjectCommand,
        occurred_at: &str,
    ) -> Result<omniproj_core::ProjectMutation, ProjectStateError> {
        apply_project_command(&self.project_id, expected_revision, command, occurred_at)
    }
}

impl Drop for TestStore {
    fn drop(&mut self) {
        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn set_commitment(
    store: &TestStore,
    expected_revision: u64,
    text: &str,
    at: &str,
) -> ProjectStateDoc {
    store
        .apply(
            expected_revision,
            ProjectCommand::SetCommitment { text: text.into() },
            at,
        )
        .unwrap()
        .state
}

fn save_framing(store: &TestStore, expected_revision: u64, at: &str) {
    store
        .apply(
            expected_revision,
            ProjectCommand::SaveFraming {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
            },
            at,
        )
        .unwrap();
}

fn complete_setup(store: &TestStore) -> ProjectStateDoc {
    store
        .apply(
            0,
            ProjectCommand::CompleteSetup {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
                first_commitment: "First commitment".into(),
            },
            AT_1,
        )
        .unwrap()
        .state
}

fn setup_document() -> String {
    "+++\n\
schema_version = 1\n\
revision = 0\n\
status = \"setup\"\n\
status_changed_at = \"2026-08-10T12:00:00Z\"\n\
created_at = \"2026-08-10T12:00:00Z\"\n\
updated_at = \"2026-08-10T12:00:00Z\"\n\
work_items = []\n\
commitment_transitions = []\n\
+++\n\
\n\
# Project notes\n"
        .to_owned()
}

fn rich_document() -> String {
    "+++\n\
schema_version = 1\n\
revision = 2\n\
status = \"active\"\n\
status_changed_at = \"2026-08-10T12:00:00Z\"\n\
objective = \"\"\"Understand\nclinically meaningful failure modes\"\"\"\n\
desired_outcome = \"A defensible result\"\n\
phase = \"Validation\"\n\
current_next_action_id = \"work-1\"\n\
created_at = \"2026-08-10T12:00:00Z\"\n\
updated_at = \"2026-08-10T14:00:00Z\"\n\
\n\
[[work_items]]\n\
id = \"work-1\"\n\
project_id = \"project-1\"\n\
text = \"\"\"Review\n+the cohort\"\"\"\n\
status = \"doing\"\n\
created_at = \"2026-08-10T12:30:00Z\"\n\
updated_at = \"2026-08-10T12:30:00Z\"\n\
\n\
[[work_items]]\n\
id = \"work-2\"\n\
project_id = \"project-1\"\n\
text = \"Resolve access\"\n\
status = \"blocked\"\n\
blocker = \"Data-use approval\"\n\
blocked_at = \"2026-08-10T13:00:00Z\"\n\
created_at = \"2026-08-10T12:45:00Z\"\n\
updated_at = \"2026-08-10T13:00:00Z\"\n\
\n\
[[commitment_transitions]]\n\
id = \"transition-1\"\n\
project_id = \"project-1\"\n\
document_revision = 1\n\
type = \"set\"\n\
next_work_item_id = \"work-1\"\n\
occurred_at = \"2026-08-10T12:30:00Z\"\n\
\n\
[[commitment_transitions]]\n\
id = \"transition-2\"\n\
project_id = \"project-1\"\n\
document_revision = 2\n\
type = \"confirmed\"\n\
previous_work_item_id = \"work-1\"\n\
next_work_item_id = \"work-1\"\n\
occurred_at = \"2026-08-10T14:00:00Z\"\n\
+++\n\
\n\
# Human notes\r\n\
\r\n\
Unknown **Markdown** stays byte-identical.  \r\n"
        .to_owned()
}

fn forge_work_item_status(
    document: &str,
    work_item_id: &str,
    original: &str,
    forged: &str,
) -> String {
    let item_marker = format!("[[work_items]]\nid = \"{work_item_id}\"");
    let item_start = document.find(&item_marker).unwrap();
    let item_tail = &document[item_start..];
    let item_end = item_tail[1..]
        .find("\n[[work_items]]")
        .or_else(|| item_tail.find("\n[[commitment_transitions]]"))
        .map_or(document.len(), |offset| item_start + offset + 1);
    let status = format!("status = \"{original}\"");
    let relative_status = document[item_start..item_end].find(&status).unwrap();
    let status_start = item_start + relative_status;
    let mut forged_document = document.to_owned();
    forged_document.replace_range(
        status_start..status_start + status.len(),
        &format!("status = \"{forged}\""),
    );
    forged_document
}

#[test]
fn parse_rich_document_preserves_values_body_and_round_trip_semantics() {
    let input = rich_document();
    let parsed = ProjectStateDoc::parse(&input).unwrap();

    assert_eq!(parsed.revision, 2);
    assert_eq!(parsed.status, ProjectStatus::Active);
    assert_eq!(
        parsed.objective.as_deref(),
        Some("Understand\nclinically meaningful failure modes")
    );
    assert_eq!(parsed.work_items.len(), 2);
    assert_eq!(parsed.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(parsed.work_items[1].status, WorkItemStatus::Blocked);
    assert_eq!(parsed.commitment_transitions.len(), 2);
    assert_eq!(
        parsed.markdown_body().as_bytes(),
        b"\n# Human notes\r\n\r\nUnknown **Markdown** stays byte-identical.  \r\n"
    );

    let reparsed = ProjectStateDoc::parse(&parsed.render().unwrap()).unwrap();
    assert_eq!(reparsed, parsed);
}

#[test]
fn parse_accepts_every_project_and_work_item_status() {
    for (serialized, expected) in [
        ("setup", ProjectStatus::Setup),
        ("active", ProjectStatus::Active),
        ("waiting", ProjectStatus::Waiting),
        ("parked", ProjectStatus::Parked),
        ("archived", ProjectStatus::Archived),
    ] {
        let mut input =
            setup_document().replacen("status = \"setup\"", &format!("status = {serialized:?}"), 1);
        if expected == ProjectStatus::Waiting {
            input = input.replacen(
                "status_changed_at =",
                "status_reason = \"External review\"\nreview_at = \"2026-08-12T12:00:00Z\"\nstatus_changed_at =",
                1,
            );
        } else if expected == ProjectStatus::Parked {
            input = input.replacen(
                "status_changed_at =",
                "status_reason = \"Deprioritized\"\nstatus_changed_at =",
                1,
            );
        }
        assert_eq!(ProjectStateDoc::parse(&input).unwrap().status, expected);
    }

    for serialized in ["planned", "doing", "blocked", "done", "abandoned"] {
        let input = rich_document().replacen(
            "text = \"Resolve access\"\nstatus = \"blocked\"",
            &format!("text = \"Resolve access\"\nstatus = {serialized:?}"),
            1,
        );
        assert!(
            ProjectStateDoc::parse(&input).is_ok(),
            "rejected {serialized}"
        );
    }
}

#[test]
fn parse_missing_document_is_typed_not_found() {
    let _guard = env_guard();
    let home = unique_home("missing");
    let project_id = ProjectId::parse("project-missing").unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let error = ProjectStateDoc::load(&project_id).unwrap_err();

    assert!(
        matches!(error, ProjectStateError::NotFound(path) if path == state_path(&home, &project_id))
    );
    std::env::remove_var("OMNIPROJ_HOME");
}

#[test]
fn parse_save_does_not_increment_revision() {
    let store = TestStore::new("save-revision");
    let mut state = store.load();
    state.revision = 41;

    state.save(&store.project_id).unwrap();

    assert_eq!(store.load().revision, 41);
}

#[test]
fn parse_rejects_pointer_that_disagrees_with_effective_history() {
    let input = rich_document().replacen(
        "current_next_action_id = \"work-1\"\n",
        "current_next_action_id = \"work-2\"\n",
        1,
    );

    assert!(matches!(
        ProjectStateDoc::parse(&input),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_inconsistent_project_ids_and_out_of_bounds_times() {
    let inconsistent_id = rich_document().replacen(
        "id = \"work-2\"\nproject_id = \"project-1\"",
        "id = \"work-2\"\nproject_id = \"project-other\"",
        1,
    );
    let transition_after_update = rich_document().replacen(
        "occurred_at = \"2026-08-10T14:00:00Z\"",
        "occurred_at = \"2026-08-10T15:00:00Z\"",
        1,
    );

    for input in [inconsistent_id, transition_after_update] {
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }
}

#[test]
fn parse_load_rejects_embedded_ids_from_another_project() {
    let _guard = env_guard();
    let home = unique_home("wrong-project");
    let requested = ProjectId::parse("project-requested").unwrap();
    let path = state_path(&home, &requested);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, rich_document()).unwrap();
    std::env::set_var("OMNIPROJ_HOME", &home);

    let result = ProjectStateDoc::load(&requested);

    assert!(matches!(result, Err(ProjectStateError::InvalidDocument(_))));
    std::env::remove_var("OMNIPROJ_HOME");
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn parse_rejects_correction_of_missing_corrected_or_correction_transition() {
    let base = rich_document()
        .replacen("revision = 2", "revision = 3", 1)
        .replacen(
            "updated_at = \"2026-08-10T14:00:00Z\"",
            "updated_at = \"2026-08-10T15:00:00Z\"",
            1,
        );
    let correction = "\n[[commitment_transitions]]\n\
id = \"transition-3\"\n\
project_id = \"project-1\"\n\
document_revision = 3\n\
type = \"correction\"\n\
previous_work_item_id = \"work-1\"\n\
next_work_item_id = \"work-1\"\n\
occurred_at = \"2026-08-10T15:00:00Z\"\n\
corrects_transition_id = \"transition-2\"\n";
    let corrected_once = base.replacen(
        "+++\n\n# Human notes",
        &format!("{correction}+++\n\n# Human notes"),
        1,
    );

    for input in [
        base.clone().replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-3\"\nproject_id = \"project-1\"\ndocument_revision = 3\ntype = \"correction\"\nprevious_work_item_id = \"work-1\"\nnext_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T15:00:00Z\"\ncorrects_transition_id = \"transition-missing\"\n+++\n\n# Human notes",
            1,
        ),
        corrected_once
            .clone()
            .replacen("revision = 3", "revision = 4", 1)
            .replacen("2026-08-10T15:00:00Z\"\n\n[[work_items]]", "2026-08-10T16:00:00Z\"\n\n[[work_items]]", 1)
            .replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-4\"\nproject_id = \"project-1\"\ndocument_revision = 4\ntype = \"correction\"\nprevious_work_item_id = \"work-1\"\nnext_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T16:00:00Z\"\ncorrects_transition_id = \"transition-2\"\n+++\n\n# Human notes",
            1,
        ),
        corrected_once
            .replacen("revision = 3", "revision = 4", 1)
            .replacen("2026-08-10T15:00:00Z\"\n\n[[work_items]]", "2026-08-10T16:00:00Z\"\n\n[[work_items]]", 1)
            .replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-4\"\nproject_id = \"project-1\"\ndocument_revision = 4\ntype = \"correction\"\nprevious_work_item_id = \"work-1\"\nnext_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T16:00:00Z\"\ncorrects_transition_id = \"transition-3\"\n+++\n\n# Human notes",
            1,
        ),
    ] {
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }
}

#[test]
fn parse_accepts_transition_document_revisions_aligned_with_document_revision() {
    let input = rich_document()
        .replacen(
            "project_id = \"project-1\"\ntype = \"set\"",
            "project_id = \"project-1\"\ndocument_revision = 1\ntype = \"set\"",
            1,
        )
        .replacen(
            "project_id = \"project-1\"\ntype = \"confirmed\"",
            "project_id = \"project-1\"\ndocument_revision = 2\ntype = \"confirmed\"",
            1,
        );

    assert!(ProjectStateDoc::parse(&input).is_ok());
}

#[test]
fn parse_rejects_non_increasing_future_and_masked_invalid_transition_revisions() {
    let non_increasing =
        rich_document().replacen("document_revision = 2", "document_revision = 1", 1);
    let future = rich_document().replacen("document_revision = 2", "document_revision = 3", 1);
    let masked_invalid = rich_document()
        .replacen("revision = 2", "revision = 3", 1)
        .replacen(
            "updated_at = \"2026-08-10T14:00:00Z\"",
            "updated_at = \"2026-08-10T15:00:00Z\"",
            1,
        )
        .replacen(
            "type = \"confirmed\"\nprevious_work_item_id",
            "type = \"confirmed\"\nreason = \"forged\"\nprevious_work_item_id",
            1,
        )
        .replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-3\"\nproject_id = \"project-1\"\ndocument_revision = 3\ntype = \"correction\"\nprevious_work_item_id = \"work-1\"\nnext_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T15:00:00Z\"\ncorrects_transition_id = \"transition-2\"\n+++\n\n# Human notes",
            1,
        );

    for input in [non_increasing, future, masked_invalid] {
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }
}

#[test]
fn parse_rejects_nonadjacent_correction_and_forged_compensation_pointers() {
    let completed_then_old_correction = rich_document()
        .replacen("revision = 2", "revision = 4", 1)
        .replacen(
            "updated_at = \"2026-08-10T14:00:00Z\"",
            "updated_at = \"2026-08-10T16:00:00Z\"",
            1,
        )
        .replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-3\"\nproject_id = \"project-1\"\ndocument_revision = 3\ntype = \"completed\"\nprevious_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T15:00:00Z\"\n\n[[commitment_transitions]]\nid = \"transition-4\"\nproject_id = \"project-1\"\ndocument_revision = 4\ntype = \"correction\"\nprevious_work_item_id = \"work-1\"\nnext_work_item_id = \"work-1\"\noccurred_at = \"2026-08-10T16:00:00Z\"\ncorrects_transition_id = \"transition-2\"\n+++\n\n# Human notes",
            1,
        )
        .replacen("current_next_action_id = \"work-1\"\n", "", 1);
    let forged_pointers = rich_document()
        .replacen("revision = 2", "revision = 3", 1)
        .replacen(
            "updated_at = \"2026-08-10T14:00:00Z\"",
            "updated_at = \"2026-08-10T15:00:00Z\"",
            1,
        )
        .replacen(
            "+++\n\n# Human notes",
            "\n[[commitment_transitions]]\nid = \"transition-3\"\nproject_id = \"project-1\"\ndocument_revision = 3\ntype = \"correction\"\nprevious_work_item_id = \"work-2\"\nnext_work_item_id = \"work-2\"\noccurred_at = \"2026-08-10T15:00:00Z\"\ncorrects_transition_id = \"transition-2\"\n+++\n\n# Human notes",
            1,
        );

    for input in [completed_then_old_correction, forged_pointers] {
        assert!(matches!(
            ProjectStateDoc::parse(&input),
            Err(ProjectStateError::InvalidDocument(_))
        ));
    }
}

#[test]
fn parse_rejects_forged_status_after_undo_set() {
    let store = TestStore::new("parse-forged-undo-set-status");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let transition_id = set.commitment_transitions[0].id.clone();
    let undone = store
        .apply(1, ProjectCommand::Undo { transition_id }, AT_2)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &item_id.to_string(),
        "abandoned",
        "doing",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_after_undo_confirm() {
    let store = TestStore::new("parse-forged-undo-confirm-status");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let confirmed = store
        .apply(
            1,
            ProjectCommand::ConfirmCommitment {
                work_item_id: item_id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let transition_id = confirmed.commitment_transitions.last().unwrap().id.clone();
    let undone = store
        .apply(2, ProjectCommand::Undo { transition_id }, AT_3)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &item_id.to_string(),
        "doing",
        "blocked",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_after_undo_complete() {
    let store = TestStore::new("parse-forged-undo-complete-status");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let completed = store
        .apply(
            1,
            ProjectCommand::CompleteCommitment {
                work_item_id: item_id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let transition_id = completed.commitment_transitions.last().unwrap().id.clone();
    let undone = store
        .apply(2, ProjectCommand::Undo { transition_id }, AT_3)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &item_id.to_string(),
        "doing",
        "done",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_after_undo_replace() {
    let store = TestStore::new("parse-forged-undo-replace-status");
    let set = set_commitment(&store, 0, "Previous", AT_1);
    let previous_id = set.work_items[0].id.clone();
    let replaced = store
        .apply(
            1,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: previous_id,
                text: "Replacement".into(),
                reason: "Priority changed".into(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let replacement_id = replaced.work_items[1].id.clone();
    let transition_id = replaced.commitment_transitions.last().unwrap().id.clone();
    let undone = store
        .apply(2, ProjectCommand::Undo { transition_id }, AT_3)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &replacement_id.to_string(),
        "abandoned",
        "doing",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_after_undo_clear() {
    let store = TestStore::new("parse-forged-undo-clear-status");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let cleared = store
        .apply(
            1,
            ProjectCommand::ClearCommitment {
                work_item_id: item_id.clone(),
                reason: None,
            },
            AT_2,
        )
        .unwrap()
        .state;
    let transition_id = cleared.commitment_transitions.last().unwrap().id.clone();
    let undone = store
        .apply(2, ProjectCommand::Undo { transition_id }, AT_3)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &item_id.to_string(),
        "doing",
        "planned",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_across_transition_revision_gap() {
    let store = TestStore::new("parse-forged-status-revision-gap");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    save_framing(&store, 1, AT_2);
    let confirmed = store
        .apply(
            2,
            ProjectCommand::ConfirmCommitment {
                work_item_id: item_id.clone(),
            },
            AT_3,
        )
        .unwrap()
        .state;
    let transition_id = confirmed.commitment_transitions.last().unwrap().id.clone();
    let undone = store
        .apply(3, ProjectCommand::Undo { transition_id }, AT_4)
        .unwrap()
        .state;
    let forged = forge_work_item_status(
        &undone.render().unwrap(),
        &item_id.to_string(),
        "doing",
        "planned",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn parse_rejects_forged_status_after_undo_and_tail_revision() {
    let store = TestStore::new("parse-forged-status-tail-revision");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let transition_id = set.commitment_transitions[0].id.clone();
    store
        .apply(1, ProjectCommand::Undo { transition_id }, AT_2)
        .unwrap();
    save_framing(&store, 2, AT_3);
    let after_framing = store.load();
    let forged = forge_work_item_status(
        &after_framing.render().unwrap(),
        &item_id.to_string(),
        "abandoned",
        "doing",
    );

    assert!(matches!(
        ProjectStateDoc::parse(&forged),
        Err(ProjectStateError::InvalidDocument(_))
    ));
}

#[test]
fn lifecycle_set_on_empty_creates_doing_item_pointer_and_event_once() {
    let store = TestStore::new("set-empty");

    let mutation = store
        .apply(
            0,
            ProjectCommand::SetCommitment {
                text: "Review cohort labels".into(),
            },
            AT_1,
        )
        .unwrap();

    assert_eq!(mutation.revision, 1);
    assert_eq!(mutation.state.revision, 1);
    assert_eq!(mutation.state.work_items.len(), 1);
    let item = &mutation.state.work_items[0];
    assert_eq!(item.project_id, store.project_id);
    assert_eq!(item.text, "Review cohort labels");
    assert_eq!(item.status, WorkItemStatus::Doing);
    assert_eq!(item.created_at, AT_1);
    assert_eq!(item.updated_at, AT_1);
    assert_eq!(
        mutation.state.current_next_action_id.as_ref(),
        Some(&item.id)
    );
    assert_eq!(mutation.state.commitment_transitions.len(), 1);
    let event = &mutation.state.commitment_transitions[0];
    assert_eq!(event.kind, omniproj_core::CommitmentTransitionKind::Set);
    assert_eq!(event.document_revision, 1);
    assert_eq!(event.next_work_item_id.as_ref(), Some(&item.id));
    assert_eq!(event.occurred_at, AT_1);
    assert_eq!(store.load(), mutation.state);
}

#[test]
fn lifecycle_set_on_occupied_is_typed_error_and_byte_identical() {
    let store = TestStore::new("set-occupied");
    let state = set_commitment(&store, 0, "First", AT_1);
    let before = store.bytes();

    let error = store
        .apply(
            state.revision,
            ProjectCommand::SetCommitment {
                text: "Second".into(),
            },
            AT_2,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ProjectStateError::CurrentCommitmentExists { .. }
    ));
    assert_eq!(store.bytes(), before);
}

#[test]
fn lifecycle_confirm_preserves_original_set_time_and_appends_event() {
    let store = TestStore::new("confirm");
    let set = set_commitment(&store, 0, "Review cohort", AT_1);
    let item = set.work_items[0].clone();

    let confirmed = store
        .apply(
            1,
            ProjectCommand::ConfirmCommitment {
                work_item_id: item.id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;

    assert_eq!(confirmed.revision, 2);
    assert_eq!(confirmed.work_items[0].created_at, AT_1);
    assert_eq!(confirmed.work_items[0].updated_at, AT_1);
    assert_eq!(confirmed.commitment_transitions[0].occurred_at, AT_1);
    assert_eq!(confirmed.commitment_transitions[0].document_revision, 1);
    assert_eq!(confirmed.commitment_transitions[1].occurred_at, AT_2);
    assert_eq!(confirmed.commitment_transitions[1].document_revision, 2);
    assert_eq!(
        confirmed.commitment_transitions[1].kind,
        omniproj_core::CommitmentTransitionKind::Confirmed
    );
}

#[test]
fn lifecycle_complete_marks_item_done_and_clears_pointer() {
    let store = TestStore::new("complete");
    let set = set_commitment(&store, 0, "Review cohort", AT_1);
    let item_id = set.work_items[0].id.clone();

    let completed = store
        .apply(
            1,
            ProjectCommand::CompleteCommitment {
                work_item_id: item_id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;

    assert_eq!(completed.revision, 2);
    assert_eq!(completed.current_next_action_id, None);
    assert_eq!(completed.work_items[0].status, WorkItemStatus::Done);
    assert_eq!(completed.work_items[0].updated_at, AT_2);
    assert_eq!(
        completed.commitment_transitions.last().unwrap().kind,
        omniproj_core::CommitmentTransitionKind::Completed
    );
    assert_eq!(
        completed
            .commitment_transitions
            .last()
            .unwrap()
            .document_revision,
        2
    );
}

#[test]
fn lifecycle_replace_retains_previous_item_status_and_requires_reason() {
    let store = TestStore::new("replace");
    let set = set_commitment(&store, 0, "Old action", AT_1);
    let previous_id = set.work_items[0].id.clone();
    let before = store.bytes();

    let error = store
        .apply(
            1,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: previous_id.clone(),
                text: "New action".into(),
                reason: "  ".into(),
            },
            AT_2,
        )
        .unwrap_err();
    assert!(matches!(error, ProjectStateError::ReasonRequired));
    assert_eq!(store.bytes(), before);

    let replaced = store
        .apply(
            1,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: previous_id.clone(),
                text: "New action".into(),
                reason: "Access is delayed".into(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    assert_eq!(replaced.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(replaced.work_items[0].updated_at, AT_1);
    assert_eq!(replaced.work_items[1].status, WorkItemStatus::Doing);
    assert_eq!(
        replaced.current_next_action_id.as_ref(),
        Some(&replaced.work_items[1].id)
    );
    let event = replaced.commitment_transitions.last().unwrap();
    assert_eq!(event.previous_work_item_id.as_ref(), Some(&previous_id));
    assert_eq!(
        event.next_work_item_id.as_ref(),
        Some(&replaced.work_items[1].id)
    );
    assert_eq!(event.reason.as_deref(), Some("Access is delayed"));
    assert_eq!(event.document_revision, 2);
}

#[test]
fn lifecycle_clear_retains_previous_item_and_status() {
    let store = TestStore::new("clear");
    let set = set_commitment(&store, 0, "Current action", AT_1);
    let item_id = set.work_items[0].id.clone();

    let cleared = store
        .apply(
            1,
            ProjectCommand::ClearCommitment {
                work_item_id: item_id.clone(),
                reason: Some("Reassessing scope".into()),
            },
            AT_2,
        )
        .unwrap()
        .state;

    assert_eq!(cleared.current_next_action_id, None);
    assert_eq!(cleared.work_items.len(), 1);
    assert_eq!(cleared.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(cleared.work_items[0].updated_at, AT_1);
    assert_eq!(
        cleared.commitment_transitions.last().unwrap().kind,
        omniproj_core::CommitmentTransitionKind::Cleared
    );
    assert_eq!(
        cleared
            .commitment_transitions
            .last()
            .unwrap()
            .document_revision,
        2
    );
}

#[test]
fn lifecycle_waiting_requires_reason_and_review_date() {
    let store = TestStore::new("waiting");
    complete_setup(&store);
    for (reason, review_at) in [
        (None, Some(AT_3.to_owned())),
        (Some("External review".to_owned()), None),
    ] {
        let before = store.bytes();
        let error = store
            .apply(
                1,
                ProjectCommand::SetStatus {
                    status: ProjectStatus::Waiting,
                    reason,
                    review_at,
                },
                AT_2,
            )
            .unwrap_err();
        assert!(matches!(error, ProjectStateError::FieldRequired { .. }));
        assert_eq!(store.bytes(), before);
    }

    let waiting = store
        .apply(
            1,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Waiting,
                reason: Some("External review".into()),
                review_at: Some(AT_3.into()),
            },
            AT_2,
        )
        .unwrap()
        .state;
    assert_eq!(waiting.status, ProjectStatus::Waiting);
    assert_eq!(waiting.status_reason.as_deref(), Some("External review"));
    assert_eq!(waiting.review_at.as_deref(), Some(AT_3));
}

#[test]
fn lifecycle_parked_requires_reason_and_allows_no_review_date() {
    let store = TestStore::new("parked");
    complete_setup(&store);
    let before = store.bytes();
    let error = store
        .apply(
            1,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Parked,
                reason: Some("".into()),
                review_at: None,
            },
            AT_2,
        )
        .unwrap_err();
    assert!(matches!(error, ProjectStateError::ReasonRequired));
    assert_eq!(store.bytes(), before);

    let parked = store
        .apply(
            1,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Parked,
                reason: Some("Not a current priority".into()),
                review_at: None,
            },
            AT_2,
        )
        .unwrap()
        .state;
    assert_eq!(parked.status, ProjectStatus::Parked);
    assert_eq!(parked.review_at, None);
}

#[test]
fn lifecycle_archived_status_is_persisted_for_later_index_exclusion() {
    let store = TestStore::new("archived");
    complete_setup(&store);

    let archived = store
        .apply(
            1,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Archived,
                reason: Some("Study complete".into()),
                review_at: None,
            },
            AT_2,
        )
        .unwrap()
        .state;

    assert_eq!(archived.status, ProjectStatus::Archived);
    assert_eq!(store.load().status, ProjectStatus::Archived);
}

#[test]
fn lifecycle_complete_setup_is_one_atomic_revision() {
    let store = TestStore::new("full-setup");

    let active = store
        .apply(
            0,
            ProjectCommand::CompleteSetup {
                objective: "Characterize failure modes".into(),
                desired_outcome: "A clinically defensible result".into(),
                phase: Some("Validation".into()),
                first_commitment: "Audit labels".into(),
            },
            AT_1,
        )
        .unwrap()
        .state;

    assert_eq!(active.revision, 1);
    assert_eq!(active.status, ProjectStatus::Active);
    assert_eq!(active.status_changed_at, AT_1);
    assert_eq!(
        active.objective.as_deref(),
        Some("Characterize failure modes")
    );
    assert_eq!(
        active.desired_outcome.as_deref(),
        Some("A clinically defensible result")
    );
    assert_eq!(active.phase.as_deref(), Some("Validation"));
    assert_eq!(active.work_items.len(), 1);
    assert_eq!(active.commitment_transitions.len(), 1);
}

#[test]
fn lifecycle_incomplete_setup_is_typed_error_and_byte_identical() {
    let store = TestStore::new("incomplete-setup");
    let before = store.bytes();

    let error = store
        .apply(
            0,
            ProjectCommand::CompleteSetup {
                objective: "  ".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
                first_commitment: "First".into(),
            },
            AT_1,
        )
        .unwrap_err();

    assert!(matches!(error, ProjectStateError::FieldRequired { field } if field == "objective"));
    assert_eq!(store.bytes(), before);
}

#[test]
fn lifecycle_save_framing_changes_only_framing_in_one_revision() {
    let store = TestStore::new("framing");

    let framed = store
        .apply(
            0,
            ProjectCommand::SaveFraming {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
            },
            AT_1,
        )
        .unwrap()
        .state;

    assert_eq!(framed.revision, 1);
    assert_eq!(framed.status, ProjectStatus::Setup);
    assert_eq!(framed.objective.as_deref(), Some("Objective"));
    assert_eq!(framed.desired_outcome.as_deref(), Some("Outcome"));
    assert!(framed.work_items.is_empty());
}

#[test]
fn lifecycle_revision_conflict_is_typed_and_byte_identical() {
    let store = TestStore::new("revision-conflict");
    let before = store.bytes();

    let error = store
        .apply(
            9,
            ProjectCommand::SaveFraming {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
            },
            AT_1,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ProjectStateError::RevisionConflict {
            expected: 9,
            actual: 0
        }
    ));
    assert_eq!(store.bytes(), before);
}

#[test]
fn lifecycle_invalid_occurred_at_is_typed_and_byte_identical() {
    let store = TestStore::new("invalid-time");
    let before = store.bytes();

    let error = store
        .apply(
            0,
            ProjectCommand::SetCommitment {
                text: "Next".into(),
            },
            "not-a-time",
        )
        .unwrap_err();

    assert!(matches!(error, ProjectStateError::InvalidTimestamp { .. }));
    assert_eq!(store.bytes(), before);
}

#[test]
fn lifecycle_setup_status_cannot_bypass_complete_setup_or_be_reentered() {
    let store = TestStore::new("setup-gate");
    for status in [ProjectStatus::Active, ProjectStatus::Waiting] {
        let before = store.bytes();
        let error = store
            .apply(
                0,
                ProjectCommand::SetStatus {
                    status,
                    reason: (status == ProjectStatus::Waiting).then(|| "Dependency".into()),
                    review_at: (status == ProjectStatus::Waiting).then(|| AT_3.into()),
                },
                AT_1,
            )
            .unwrap_err();
        assert!(matches!(error, ProjectStateError::InvalidCommand(_)));
        assert_eq!(store.bytes(), before);
    }

    let active = store
        .apply(
            0,
            ProjectCommand::CompleteSetup {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: None,
                first_commitment: "First".into(),
            },
            AT_1,
        )
        .unwrap()
        .state;
    let before = store.bytes();
    let error = store
        .apply(
            active.revision,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Setup,
                reason: None,
                review_at: None,
            },
            AT_2,
        )
        .unwrap_err();
    assert!(matches!(error, ProjectStateError::InvalidCommand(_)));
    assert_eq!(store.bytes(), before);
}

#[cfg(unix)]
#[test]
fn lifecycle_audit_commit_failure_reports_durable_revision() {
    use std::os::unix::fs::PermissionsExt;

    let store = TestStore::new("audit-failure");
    let hook = store.home.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();

    let error = store
        .apply(
            0,
            ProjectCommand::SetCommitment {
                text: "Durable".into(),
            },
            AT_1,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ProjectStateError::AuditCommitFailed {
            durable_revision: 1,
            ..
        }
    ));
    assert_eq!(store.load().revision, 1);
    std::fs::remove_file(hook).unwrap();
    ensure_home().unwrap();
}

#[test]
fn undo_set_clears_pointer_abandons_item_and_preserves_event() {
    let store = TestStore::new("undo-set");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let original = set.commitment_transitions[0].clone();

    let undone = store
        .apply(
            1,
            ProjectCommand::Undo {
                transition_id: original.id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;

    assert_eq!(undone.revision, 2);
    assert_eq!(undone.current_next_action_id, None);
    assert_eq!(undone.work_items[0].status, WorkItemStatus::Abandoned);
    assert_eq!(undone.commitment_transitions[0], original);
    let correction = undone.commitment_transitions.last().unwrap();
    assert_eq!(
        correction.kind,
        omniproj_core::CommitmentTransitionKind::Correction
    );
    assert_eq!(correction.document_revision, 2);
    assert_eq!(
        correction.corrects_transition_id.as_ref(),
        Some(&original.id)
    );
}

#[test]
fn undo_confirm_keeps_pointer_status_and_original_set_clock() {
    let store = TestStore::new("undo-confirm");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let confirmed = store
        .apply(
            1,
            ProjectCommand::ConfirmCommitment {
                work_item_id: item_id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let original = confirmed.commitment_transitions.last().unwrap().clone();

    let undone = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: original.id.clone(),
            },
            AT_3,
        )
        .unwrap()
        .state;

    assert_eq!(undone.current_next_action_id.as_ref(), Some(&item_id));
    assert_eq!(undone.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(undone.work_items[0].created_at, AT_1);
    assert_eq!(undone.work_items[0].updated_at, AT_1);
    assert!(undone.commitment_transitions.contains(&original));
    assert_eq!(
        undone
            .commitment_transitions
            .last()
            .unwrap()
            .corrects_transition_id
            .as_ref(),
        Some(&original.id)
    );
}

#[test]
fn undo_complete_restores_pointer_and_doing_status() {
    let store = TestStore::new("undo-complete");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let completed = store
        .apply(
            1,
            ProjectCommand::CompleteCommitment {
                work_item_id: item_id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let original = completed.commitment_transitions.last().unwrap().clone();

    let undone = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: original.id.clone(),
            },
            AT_3,
        )
        .unwrap()
        .state;

    assert_eq!(undone.current_next_action_id.as_ref(), Some(&item_id));
    assert_eq!(undone.work_items[0].status, WorkItemStatus::Doing);
    assert!(undone.commitment_transitions.contains(&original));
    assert_eq!(
        undone
            .commitment_transitions
            .last()
            .unwrap()
            .corrects_transition_id
            .as_ref(),
        Some(&original.id)
    );
}

#[test]
fn undo_replace_restores_previous_pointer_without_changing_previous_status() {
    let store = TestStore::new("undo-replace");
    let set = set_commitment(&store, 0, "Previous", AT_1);
    let previous_id = set.work_items[0].id.clone();
    let replaced = store
        .apply(
            1,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: previous_id.clone(),
                text: "Replacement".into(),
                reason: "Priority changed".into(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let replacement_id = replaced.work_items[1].id.clone();
    let original = replaced.commitment_transitions.last().unwrap().clone();

    let undone = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: original.id.clone(),
            },
            AT_3,
        )
        .unwrap()
        .state;

    assert_eq!(undone.current_next_action_id.as_ref(), Some(&previous_id));
    assert_eq!(undone.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(undone.work_items[0].updated_at, AT_1);
    assert_eq!(
        undone
            .work_items
            .iter()
            .find(|item| item.id == replacement_id)
            .unwrap()
            .status,
        WorkItemStatus::Abandoned
    );
    assert!(undone.commitment_transitions.contains(&original));
    assert_eq!(
        undone
            .commitment_transitions
            .last()
            .unwrap()
            .corrects_transition_id
            .as_ref(),
        Some(&original.id)
    );
}

#[test]
fn undo_clear_restores_pointer_without_changing_item_status() {
    let store = TestStore::new("undo-clear");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let item_id = set.work_items[0].id.clone();
    let cleared = store
        .apply(
            1,
            ProjectCommand::ClearCommitment {
                work_item_id: item_id.clone(),
                reason: None,
            },
            AT_2,
        )
        .unwrap()
        .state;
    let original = cleared.commitment_transitions.last().unwrap().clone();

    let undone = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: original.id.clone(),
            },
            AT_3,
        )
        .unwrap()
        .state;

    assert_eq!(undone.current_next_action_id.as_ref(), Some(&item_id));
    assert_eq!(undone.work_items[0].status, WorkItemStatus::Doing);
    assert_eq!(undone.work_items[0].updated_at, AT_1);
    assert!(undone.commitment_transitions.contains(&original));
    assert_eq!(
        undone
            .commitment_transitions
            .last()
            .unwrap()
            .corrects_transition_id
            .as_ref(),
        Some(&original.id)
    );
}

#[test]
fn undo_older_transition_is_conflict_and_byte_identical() {
    let store = TestStore::new("undo-older");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let set_id = set.commitment_transitions[0].id.clone();
    let item_id = set.work_items[0].id.clone();
    store
        .apply(
            1,
            ProjectCommand::ConfirmCommitment {
                work_item_id: item_id,
            },
            AT_2,
        )
        .unwrap();
    let before = store.bytes();

    let error = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: set_id.clone(),
            },
            AT_3,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ProjectStateError::UndoConflict { transition_id } if transition_id == set_id
    ));
    assert_eq!(store.bytes(), before);
}

#[test]
fn undo_correction_is_conflict_and_byte_identical() {
    let store = TestStore::new("undo-correction");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let set_id = set.commitment_transitions[0].id.clone();
    let corrected = store
        .apply(
            1,
            ProjectCommand::Undo {
                transition_id: set_id,
            },
            AT_2,
        )
        .unwrap()
        .state;
    let correction_id = corrected.commitment_transitions.last().unwrap().id.clone();
    let before = store.bytes();

    let error = store
        .apply(
            2,
            ProjectCommand::Undo {
                transition_id: correction_id.clone(),
            },
            AT_3,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ProjectStateError::UndoConflict { transition_id } if transition_id == correction_id
    ));
    assert_eq!(store.bytes(), before);
}

fn assert_stale_undo_conflict(
    store: &TestStore,
    expected_revision: u64,
    transition_id: omniproj_core::CommitmentTransitionId,
    at: &str,
) {
    let before = store.bytes();
    let error = store
        .apply(
            expected_revision,
            ProjectCommand::Undo {
                transition_id: transition_id.clone(),
            },
            at,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ProjectStateError::UndoConflict { transition_id: actual } if actual == transition_id
    ));
    assert_eq!(store.bytes(), before);
}

#[test]
fn undo_freshness_rejects_set_after_later_framing_revision() {
    let store = TestStore::new("stale-set");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let target = set.commitment_transitions.last().unwrap().id.clone();
    save_framing(&store, 1, AT_2);
    assert_stale_undo_conflict(&store, 2, target, AT_3);
}

#[test]
fn undo_freshness_rejects_confirm_after_later_framing_revision() {
    let store = TestStore::new("stale-confirm");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let confirmed = store
        .apply(
            1,
            ProjectCommand::ConfirmCommitment {
                work_item_id: set.work_items[0].id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let target = confirmed.commitment_transitions.last().unwrap().id.clone();
    save_framing(&store, 2, "2026-08-10T14:30:00Z");
    assert_stale_undo_conflict(&store, 3, target, AT_3);
}

#[test]
fn undo_freshness_rejects_complete_after_later_framing_revision() {
    let store = TestStore::new("stale-complete");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let completed = store
        .apply(
            1,
            ProjectCommand::CompleteCommitment {
                work_item_id: set.work_items[0].id.clone(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let target = completed.commitment_transitions.last().unwrap().id.clone();
    save_framing(&store, 2, "2026-08-10T14:30:00Z");
    assert_stale_undo_conflict(&store, 3, target, AT_3);
}

#[test]
fn undo_freshness_rejects_replace_after_later_framing_revision() {
    let store = TestStore::new("stale-replace");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let replaced = store
        .apply(
            1,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: set.work_items[0].id.clone(),
                text: "Replacement".into(),
                reason: "Changed".into(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let target = replaced.commitment_transitions.last().unwrap().id.clone();
    save_framing(&store, 2, "2026-08-10T14:30:00Z");
    assert_stale_undo_conflict(&store, 3, target, AT_3);
}

#[test]
fn undo_freshness_rejects_clear_after_later_framing_revision() {
    let store = TestStore::new("stale-clear");
    let set = set_commitment(&store, 0, "Current", AT_1);
    let cleared = store
        .apply(
            1,
            ProjectCommand::ClearCommitment {
                work_item_id: set.work_items[0].id.clone(),
                reason: None,
            },
            AT_2,
        )
        .unwrap()
        .state;
    let target = cleared.commitment_transitions.last().unwrap().id.clone();
    save_framing(&store, 2, "2026-08-10T14:30:00Z");
    assert_stale_undo_conflict(&store, 3, target, AT_3);
}

#[test]
fn preserves_legacy_documents_across_all_r0_project_commands() {
    let store = TestStore::new("legacy-documents");
    let notes = store
        .home
        .join("projects")
        .join(store.project_id.as_str())
        .join("notes");
    let next_path = notes.join("next.md");
    let plan_path = notes.join("plan.md");
    let next_bytes = b"# Hand-authored next\r\n\r\n- [ ] id-less task  \r\n- prose stays mine\r\n";
    let plan_bytes = b"# Plan\n\nUnstructured prose.\n\n- no machine identifiers\n";
    std::fs::write(&next_path, next_bytes).unwrap();
    std::fs::write(&plan_path, plan_bytes).unwrap();

    store
        .apply(
            0,
            ProjectCommand::SaveFraming {
                objective: "Objective draft".into(),
                desired_outcome: "Outcome draft".into(),
                phase: None,
            },
            AT_1,
        )
        .unwrap();
    let setup = store
        .apply(
            1,
            ProjectCommand::CompleteSetup {
                objective: "Objective".into(),
                desired_outcome: "Outcome".into(),
                phase: Some("Phase".into()),
                first_commitment: "First".into(),
            },
            AT_2,
        )
        .unwrap()
        .state;
    let first_id = setup.work_items[0].id.clone();
    store
        .apply(
            2,
            ProjectCommand::ConfirmCommitment {
                work_item_id: first_id.clone(),
            },
            "2026-08-10T14:10:00Z",
        )
        .unwrap();
    let replaced = store
        .apply(
            3,
            ProjectCommand::ReplaceCommitment {
                previous_work_item_id: first_id,
                text: "Replacement".into(),
                reason: "More actionable".into(),
            },
            "2026-08-10T14:20:00Z",
        )
        .unwrap()
        .state;
    let replacement_id = replaced.current_next_action_id.unwrap();
    store
        .apply(
            4,
            ProjectCommand::ClearCommitment {
                work_item_id: replacement_id,
                reason: Some("Reassess".into()),
            },
            "2026-08-10T14:30:00Z",
        )
        .unwrap();
    let set = store
        .apply(
            5,
            ProjectCommand::SetCommitment {
                text: "Final action".into(),
            },
            "2026-08-10T14:40:00Z",
        )
        .unwrap()
        .state;
    let final_id = set.current_next_action_id.unwrap();
    let completed = store
        .apply(
            6,
            ProjectCommand::CompleteCommitment {
                work_item_id: final_id,
            },
            "2026-08-10T14:50:00Z",
        )
        .unwrap()
        .state;
    let completed_transition_id = completed.commitment_transitions.last().unwrap().id.clone();
    store
        .apply(
            7,
            ProjectCommand::Undo {
                transition_id: completed_transition_id,
            },
            "2026-08-10T15:00:00Z",
        )
        .unwrap();
    store
        .apply(
            8,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Waiting,
                reason: Some("External review".into()),
                review_at: Some("2026-08-12T12:00:00Z".into()),
            },
            "2026-08-10T15:10:00Z",
        )
        .unwrap();
    store
        .apply(
            9,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Parked,
                reason: Some("Paused".into()),
                review_at: None,
            },
            "2026-08-10T15:20:00Z",
        )
        .unwrap();
    store
        .apply(
            10,
            ProjectCommand::SetStatus {
                status: ProjectStatus::Archived,
                reason: Some("Closed".into()),
                review_at: None,
            },
            "2026-08-10T15:30:00Z",
        )
        .unwrap();

    assert_eq!(std::fs::read(next_path).unwrap(), next_bytes);
    assert_eq!(std::fs::read(plan_path).unwrap(), plan_bytes);
}

#[test]
fn preserves_markdown_body_bytes_during_domain_mutation() {
    let store = TestStore::new("body-bytes");
    let input = rich_document().replace("project-1", store.project_id.as_str());
    std::fs::write(state_path(&store.home, &store.project_id), &input).unwrap();
    let body = ProjectStateDoc::parse(&input)
        .unwrap()
        .markdown_body()
        .as_bytes()
        .to_vec();

    store
        .apply(
            2,
            ProjectCommand::SaveFraming {
                objective: "Updated objective".into(),
                desired_outcome: "Updated outcome".into(),
                phase: None,
            },
            "2026-08-10T15:00:00Z",
        )
        .unwrap();

    assert_eq!(store.load().markdown_body().as_bytes(), body);
}
