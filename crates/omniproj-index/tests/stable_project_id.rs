use std::path::{Path, PathBuf};

use omniproj_core::{
    project_hash, register_project, relink_primary_git_source, Message, RegisterOutcome,
    RegisterProjectInput, RelinkSourceInput, Role, Session, Source,
};

struct TempStore {
    root: PathBuf,
}

impl TempStore {
    fn new() -> Self {
        let unique = format!(
            "omniproj-stable-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("OMNIPROJ_HOME", root.join("store"));
        Self { root }
    }

    fn source(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        std::env::remove_var("OMNIPROJ_HOME");
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn one_session(cwd: &Path) -> Session {
    Session {
        id: "session-stable-id".into(),
        source: Source::Codex,
        cwd: cwd.to_string_lossy().into_owned(),
        started_at: None,
        ended_at: None,
        mtime: 1_723_396_800.0,
        messages: vec![Message {
            idx: 0,
            role: Role::User,
            text: "permanent project identity keeps this searchable".into(),
            ts: None,
        }],
    }
}

#[test]
fn moving_and_relinking_a_source_keeps_one_project_index() {
    let temp = TempStore::new();
    let original = temp.source("original-source");
    let moved = temp.source("moved-source");
    std::fs::create_dir_all(&original).unwrap();

    let project = match register_project(RegisterProjectInput {
        location: &original,
        name: "Stable index",
        created_at: "2026-08-11T12:00:00Z",
    })
    .unwrap()
    {
        RegisterOutcome::Created(project) => project,
        RegisterOutcome::Existing(id) => panic!("unexpected existing project {id}"),
    };
    let source = project.primary_git_source().unwrap().clone();
    let sessions = vec![one_session(&original)];

    assert!(omniproj_index::ensure_index_for(&project.id, &sessions).unwrap());
    let index_before = omniproj_index::index_path(&project.id);
    assert_eq!(
        omniproj_index::search_for(&project.id, "project identity", 10)
            .unwrap()
            .len(),
        1
    );

    std::fs::rename(&original, &moved).unwrap();
    let relinked = relink_primary_git_source(RelinkSourceInput {
        project_id: &project.id,
        expected_source_revision: source.revision,
        expected_location: &source.location,
        new_location: &moved,
    })
    .unwrap();

    assert_eq!(omniproj_index::index_path(&relinked.id), index_before);
    assert_eq!(
        omniproj_index::search_for(&relinked.id, "project identity", 10)
            .unwrap()
            .len(),
        1,
        "relinking must not orphan the existing search index"
    );
    let moved_location = &relinked.primary_git_source().unwrap().location;
    let path_derived_cache = omniproj_core::cache_dir(&project_hash(moved_location));
    assert!(
        !path_derived_cache.exists(),
        "relink created a second cache at {}",
        path_derived_cache.display()
    );
}
