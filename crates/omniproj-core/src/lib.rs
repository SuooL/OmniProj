//! omniproj-core — domain types + `~/.omniproj` layout.
//!
//! This crate is the hexagonal core: pure domain, no async / network / LLM deps.
//! Everything else (capture, distill, cli) depends inward on these types.

pub mod factsheet;
pub mod ids;
pub mod model;
pub mod notes;
pub mod paths;
pub mod plan;
pub mod privacy;
pub mod project;
pub mod store;
pub mod user_model;

pub use factsheet::{FactSheet, GitFacts};
pub use ids::{CommitmentTransitionId, ProjectId, ProjectSourceId, WorkItemId};
pub use model::{Message, Role, Session, Source};
pub use notes::{next_path, NextDoc, NextItem, TaskStatus};
pub use paths::{
    auto_dir, cache_dir, content_hash, learned_path, notes_dir, omniproj_home, project_dir,
    project_hash,
};
pub use plan::{plan_path, PlanDoc, PlanEntry, PlanStatus};
pub use privacy::{default_deny_globs, redact_secrets, PrivacyPolicy};
pub use project::{
    find_by_cwd, list_projects, load_meta, register, remove_project, set_last_distilled, Cadence,
    Fingerprint, ProjectMeta,
};
#[allow(deprecated)]
pub use store::{
    atomic_write, commit_all, commit_paths_checked, ensure_home, store_txn, with_store_txn,
    worktree_diff, StoreError, CURRENT_SCHEMA_VERSION, SCHEMA_VERSION_FILE,
};
pub use user_model::{
    user_model_path, Dimension, UserModel, DIMENSIONS, USER_MODEL_DIM_CAP_CHARS,
    USER_MODEL_TEMPLATE,
};

/// Shared serialization lock for tests that mutate the process-global `OMNIPROJ_HOME`
/// env var. `OMNIPROJ_HOME` is read by `paths::omniproj_home()`, so any two tests that set
/// it can race each other; every such test must hold this guard for its duration.
/// Poison-tolerant: a panicking test must not wedge the rest of the suite.
#[cfg(test)]
pub(crate) fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
