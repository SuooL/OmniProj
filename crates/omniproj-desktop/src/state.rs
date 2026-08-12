//! In-flight refresh coordination. This is work coordination only — never canonical
//! persistence. It exists so a second refresh request for a project that is already
//! being observed returns a stable "in progress" outcome instead of launching a second,
//! overlapping set of Git commands (or clearing the visible cached facts mid-flight).

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

use omniproj_core::ids::ProjectId;

/// Desktop-only, non-persistent coordination state.
#[derive(Debug, Clone, Default)]
pub struct DesktopState {
    refreshes: Arc<Mutex<HashSet<ProjectId>>>,
}

impl DesktopState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to claim the refresh slot for `project_id`. Returns a guard on success, or
    /// `None` if a refresh for this project is already in flight (skip semantics). The
    /// guard releases the slot on drop — on success, error, or cancellation.
    pub async fn begin_refresh(&self, project_id: &ProjectId) -> Option<RefreshGuard> {
        let mut set = self.refreshes.lock().await;
        if set.contains(project_id) {
            return None;
        }
        set.insert(project_id.clone());
        Some(RefreshGuard {
            set: self.refreshes.clone(),
            project_id: project_id.clone(),
        })
    }

    /// Whether a project currently holds the refresh slot (introspection for tests/UX).
    pub async fn is_refreshing(&self, project_id: &ProjectId) -> bool {
        self.refreshes.lock().await.contains(project_id)
    }
}

/// RAII release of one project's refresh slot. Guarantees removal even under contention
/// or panic, so a slot is never leaked (which would wedge that project's refreshes).
pub struct RefreshGuard {
    set: Arc<Mutex<HashSet<ProjectId>>>,
    project_id: ProjectId,
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        // Fast path: uncontended, remove immediately.
        if let Ok(mut set) = self.set.try_lock() {
            set.remove(&self.project_id);
            return;
        }
        // Contended. If we are on the async runtime, schedule the removal there; the slot
        // is released as soon as the current holder finishes.
        let set = self.set.clone();
        let project_id = self.project_id.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    set.lock().await.remove(&project_id);
                });
            }
            // Off-runtime drop: it is safe to block briefly here.
            Err(_) => {
                self.set.blocking_lock().remove(&self.project_id);
            }
        }
    }
}
