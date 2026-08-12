//! Last-successful repository observation cache: `cache/r0-observation.json`.
//!
//! This is the ONLY persisted repository fact. It is derived, regenerable, and lives
//! under the gitignored `projects/<id>/cache/` tree, so it is written atomically but
//! never audited/committed. A missing or corrupt cache is a legitimate "no observation
//! yet" — not a swallowed domain error — so `load` returns `Option`.
//!
//! On a successful refresh the whole file is replaced. On a failed refresh the cache is
//! left byte-for-byte untouched, so the UI keeps showing the last good facts.

use serde::{Deserialize, Serialize};

use omniproj_capture::git::{HeadState, RepositoryObservation};
use omniproj_core::ids::{ProjectId, ProjectSourceId, WorkItemId};
use omniproj_core::paths::cache_dir_for;
use omniproj_core::store::{atomic_write, StoreError};

use crate::dto::{CommitDto, HeadStateDto, ObservedActualDto};

const CACHE_FILE: &str = "r0-observation.json";

/// The persisted last-successful observation for one source of one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedObservation {
    pub project_id: ProjectId,
    pub source_id: ProjectSourceId,
    pub observed_at: String,
    pub head: HeadStateDto,
    pub last_commit: Option<CommitDto>,
    pub changed_files: u32,
    pub staged_files: u32,
    pub unstaged_files: u32,
    pub untracked_files: u32,
    pub status_digest: String,
    /// The commitment the `commits_since_commitment` count was computed against. If the
    /// current commitment differs, the count is stale and must not be shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_work_item_id: Option<WorkItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits_since_commitment: Option<u32>,
}

impl CachedObservation {
    /// Build a cache record from a fresh observation. `commits_since_commitment` is the
    /// count computed against `commitment_work_item_id` (both present only when a
    /// current commitment existed at observation time and the count succeeded).
    pub fn from_observation(
        project_id: ProjectId,
        source_id: ProjectSourceId,
        observation: &RepositoryObservation,
        commitment_work_item_id: Option<WorkItemId>,
        commits_since_commitment: Option<u32>,
    ) -> Self {
        Self {
            project_id,
            source_id,
            observed_at: observation.observed_at.clone(),
            head: head_state_dto(&observation.head_state),
            last_commit: observation.last_commit.as_ref().map(|commit| CommitDto {
                sha: commit.sha.clone(),
                short_sha: commit.short_sha.clone(),
                subject: commit.subject.clone(),
                committed_at: commit.committed_at.clone(),
            }),
            changed_files: observation.changed_files as u32,
            staged_files: observation.staged_files as u32,
            unstaged_files: observation.unstaged_files as u32,
            untracked_files: observation.untracked_files as u32,
            status_digest: observation.status_digest.clone(),
            commitment_work_item_id,
            commits_since_commitment,
        }
    }

    /// Render the observed-actual for display. `current_commitment` gates the
    /// commitment-relative count: it is shown only when the cache was computed against
    /// exactly that commitment.
    pub fn to_observed_actual(&self, current_commitment: Option<&WorkItemId>) -> ObservedActualDto {
        let commits_since_commitment = match (current_commitment, &self.commitment_work_item_id) {
            (Some(current), Some(cached)) if current == cached => self.commits_since_commitment,
            _ => None,
        };
        ObservedActualDto {
            observed_at: self.observed_at.clone(),
            head: self.head.clone(),
            last_commit: self.last_commit.clone(),
            changed_files: self.changed_files,
            staged_files: self.staged_files,
            unstaged_files: self.unstaged_files,
            untracked_files: self.untracked_files,
            status_digest: self.status_digest.clone(),
            commits_since_commitment,
        }
    }
}

/// Convert a capture HEAD state into its wire form.
pub fn head_state_dto(head: &HeadState) -> HeadStateDto {
    match head {
        HeadState::Attached { branch } => HeadStateDto::Attached {
            branch: branch.clone(),
        },
        HeadState::Detached => HeadStateDto::Detached,
        HeadState::Unborn { branch } => HeadStateDto::Unborn {
            branch: branch.clone(),
        },
    }
}

fn cache_path(project_id: &ProjectId) -> std::path::PathBuf {
    cache_dir_for(project_id).join(CACHE_FILE)
}

/// Load the cached observation, or `None` if absent/corrupt (the cache is disposable).
/// A cache that belongs to a different source (after a relink) is treated as absent.
pub fn load(project_id: &ProjectId, source_id: &ProjectSourceId) -> Option<CachedObservation> {
    let bytes = std::fs::read(cache_path(project_id)).ok()?;
    let cached: CachedObservation = serde_json::from_slice(&bytes).ok()?;
    if &cached.source_id != source_id {
        return None;
    }
    Some(cached)
}

/// Atomically replace the cache file. Creates the `cache/` directory if needed.
pub fn store(cached: &CachedObservation) -> Result<(), StoreError> {
    let dir = cache_dir_for(&cached.project_id);
    std::fs::create_dir_all(&dir).map_err(StoreError::Io)?;
    let bytes = serde_json::to_vec_pretty(cached)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    atomic_write(&dir.join(CACHE_FILE), &bytes)
}
