//! Project registry (spec §4.1). A tracked project is a `~/.omniproj/projects/<hash>/`
//! dir with a `meta.toml`. Registration is explicit (`omniproj add`) and lives entirely
//! in `~/.omniproj`, never in the user's repo (charter §5 原则2).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{omniproj_home, project_dir, project_hash};

/// Per-project metadata. Tool-managed but human-readable/editable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Absolute path of the tracked directory (identity source).
    pub path: String,
    pub name: String,
    pub hash: String,
    /// RFC3339, supplied by the caller (core stays free of a clock dependency at call sites).
    pub added_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_distilled: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_head: Option<String>,
    /// Cursor: digest of full `git status --porcelain` at the last distill.
    /// This catches dirty worktree/staging/untracked changes even when HEAD did not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status_digest: Option<String>,
    /// Cursor: mtime (epoch secs) of the newest session seen at the last distill.
    /// Part of the change fingerprint (with `last_head` and `last_status_digest`);
    /// see [`Fingerprint`] (spec §5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_session_mtime: Option<f64>,
    /// Per-project cadence overrides (charter §5 原则6: cadence 可为不同项目、不同
    /// 阶段设置). Absent → the project follows the global config / daemon defaults.
    /// Additive optional field: an older store simply lacks it, so no schema bump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<Cadence>,
}

/// Per-project cadence knobs (charter §5 原则6). Both optional so a project can
/// override just one. Empty tables serialize to nothing (all `None` → parent field
/// skipped), keeping `meta.toml` clean for projects that don't customize cadence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cadence {
    /// Staleness-floor override in seconds — how long the daemon may go without a
    /// forced refresh for THIS project (孵化期调高、冲刺期调低). None → global floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_floor_secs: Option<u64>,
    /// Reasoning depth override for this project: "shallow" | "deep". None → config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<String>,
}

impl Cadence {
    /// True when nothing is set — used to drop an all-empty table rather than persist it.
    pub fn is_empty(&self) -> bool {
        self.refresh_floor_secs.is_none() && self.depth.is_none()
    }
}

/// A deterministic change signal for a tracked project (spec §5): the current git
/// `HEAD` plus the newest captured session mtime. Compared against the cursor stored
/// in [`ProjectMeta`] to decide whether a re-distill is warranted — no LLM, zero cost.
/// `None` fields mean "no git" / "no sessions", which still register as change when
/// the cursor side differs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fingerprint {
    pub head: Option<String>,
    pub status_digest: Option<String>,
    pub latest_session_mtime: Option<f64>,
}

impl Fingerprint {
    /// Has the substrate changed since `meta`'s last distill? `true` when never
    /// distilled, when `HEAD` moved, or when a newer session appeared. Pure +
    /// deterministic so the staleness floor is testable without IO (spec §5.2).
    pub fn is_stale(&self, meta: &ProjectMeta) -> bool {
        // Never distilled → always stale (first run must produce output).
        if meta.last_distilled.is_none() {
            return true;
        }
        if self.head != meta.last_head {
            return true;
        }
        if self.status_digest != meta.last_status_digest {
            return true;
        }
        match (self.latest_session_mtime, meta.last_session_mtime) {
            // A strictly-newer session than the cursor → new conversation to fold in.
            (Some(now), Some(prev)) => now > prev,
            // Sessions appeared where there were none before.
            (Some(_), None) => true,
            // No sessions now: mtime side adds no new signal (HEAD already compared).
            (None, _) => false,
        }
    }
}

pub fn meta_path(hash: &str) -> PathBuf {
    project_dir(hash).join("meta.toml")
}

fn write_meta(meta: &ProjectMeta) -> std::io::Result<()> {
    std::fs::create_dir_all(project_dir(&meta.hash))?;
    let text = toml::to_string_pretty(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(meta_path(&meta.hash), text)
}

pub fn load_meta(hash: &str) -> Option<ProjectMeta> {
    let text = std::fs::read_to_string(meta_path(hash)).ok()?;
    toml::from_str(&text).ok()
}

/// Register (or refresh) a tracked project. Creates the `auto/`/`notes/`/`cache/`
/// skeleton and writes `meta.toml`. Idempotent: re-registering updates path/name
/// but preserves distill bookkeeping. `now` is an RFC3339 timestamp from the caller.
pub fn register(abs_path: &str, name: &str, now: &str) -> std::io::Result<ProjectMeta> {
    let hash = project_hash(abs_path);
    let dir = project_dir(&hash);
    for sub in ["auto", "notes", "cache"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    let meta = match load_meta(&hash) {
        Some(mut m) => {
            m.path = abs_path.to_string();
            m.name = name.to_string();
            m
        }
        None => ProjectMeta {
            path: abs_path.to_string(),
            name: name.to_string(),
            hash: hash.clone(),
            added_at: now.to_string(),
            last_distilled: None,
            last_head: None,
            last_status_digest: None,
            last_session_mtime: None,
            cadence: None,
        },
    };
    write_meta(&meta)?;
    Ok(meta)
}

/// All registered projects, sorted by name.
pub fn list_projects() -> Vec<ProjectMeta> {
    let root = omniproj_home().join("projects");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(m) = load_meta(name) {
                    out.push(m);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Unregister a project: removes `meta.toml` + `auto/` + `cache/` (AI/derived,
/// regenerable). **Preserves `notes/`** if it holds user content (charter §5 原则4).
/// Returns `true` when `notes/` was kept (so the caller can tell the user).
pub fn remove_project(hash: &str) -> bool {
    let dir = project_dir(hash);
    let notes_has_content = std::fs::read_dir(dir.join("notes"))
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);
    let _ = std::fs::remove_file(meta_path(hash));
    let _ = std::fs::remove_dir_all(dir.join("auto"));
    let _ = std::fs::remove_dir_all(dir.join("cache"));
    if notes_has_content {
        true
    } else {
        let _ = std::fs::remove_dir_all(&dir);
        false
    }
}

/// Update distill bookkeeping after a successful distill. No-op if unregistered.
/// Persists the full change cursor (`now` + `head` + newest session mtime) so the
/// next staleness check (spec §5) compares against exactly what was just distilled.
pub fn set_last_distilled(
    hash: &str,
    now: &str,
    head: Option<&str>,
    status_digest: Option<&str>,
    session_mtime: Option<f64>,
) {
    if let Some(mut m) = load_meta(hash) {
        m.last_distilled = Some(now.to_string());
        m.last_head = head.map(str::to_string);
        m.last_status_digest = status_digest.map(str::to_string);
        m.last_session_mtime = session_mtime;
        let _ = write_meta(&m);
    }
}

/// The registered project whose `path` is the longest prefix of `cwd`, so running
/// from a subdirectory resolves to its project.
pub fn find_by_cwd(cwd: &Path) -> Option<ProjectMeta> {
    best_prefix_match(&cwd.to_string_lossy(), list_projects())
}

/// Pure prefix-match core of `find_by_cwd` (separated so it's testable without IO).
fn best_prefix_match(cwd: &str, metas: Vec<ProjectMeta>) -> Option<ProjectMeta> {
    metas
        .into_iter()
        .filter(|m| cwd == m.path || cwd.starts_with(&format!("{}/", m.path)))
        .max_by_key(|m| m.path.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(path: &str) -> ProjectMeta {
        ProjectMeta {
            path: path.into(),
            name: "p".into(),
            hash: project_hash(path),
            added_at: "2026-06-04T00:00:00Z".into(),
            last_distilled: None,
            last_head: None,
            last_status_digest: None,
            last_session_mtime: None,
            cadence: None,
        }
    }

    /// A meta that has already been distilled once at `head` / `status` / `mtime`.
    fn distilled(
        path: &str,
        head: Option<&str>,
        status_digest: Option<&str>,
        mtime: Option<f64>,
    ) -> ProjectMeta {
        let mut m = meta(path);
        m.last_distilled = Some("2026-06-04T01:00:00Z".into());
        m.last_head = head.map(str::to_string);
        m.last_status_digest = status_digest.map(str::to_string);
        m.last_session_mtime = mtime;
        m
    }

    fn fp(head: Option<&str>, status_digest: Option<&str>, mtime: Option<f64>) -> Fingerprint {
        Fingerprint {
            head: head.map(str::to_string),
            status_digest: status_digest.map(str::to_string),
            latest_session_mtime: mtime,
        }
    }

    #[test]
    fn never_distilled_is_always_stale() {
        let m = meta("/p"); // last_distilled = None
        assert!(fp(None, None, None).is_stale(&m));
        assert!(fp(Some("abc"), Some("clean"), Some(10.0)).is_stale(&m));
    }

    #[test]
    fn unchanged_fingerprint_is_fresh() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(!fp(Some("abc123"), Some("clean"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn moved_head_is_stale() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("def456"), Some("clean"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn dirty_status_digest_is_stale_even_when_head_did_not_move() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("abc123"), Some("dirty"), Some(100.0)).is_stale(&m));
    }

    #[test]
    fn newer_session_is_stale_but_older_or_equal_is_not() {
        let m = distilled("/p", Some("abc123"), Some("clean"), Some(100.0));
        assert!(fp(Some("abc123"), Some("clean"), Some(100.5)).is_stale(&m)); // newer
        assert!(!fp(Some("abc123"), Some("clean"), Some(100.0)).is_stale(&m)); // equal
        assert!(!fp(Some("abc123"), Some("clean"), Some(99.0)).is_stale(&m)); // older (e.g. file removed)
    }

    #[test]
    fn first_session_after_distill_is_stale() {
        let m = distilled("/p", Some("abc123"), Some("clean"), None);
        assert!(fp(Some("abc123"), Some("clean"), Some(50.0)).is_stale(&m));
    }

    #[test]
    fn toml_roundtrips() {
        let m = meta("/Users/x/git/foo");
        let text = toml::to_string_pretty(&m).unwrap();
        let back: ProjectMeta = toml::from_str(&text).unwrap();
        assert_eq!(m, back);
        assert!(!text.contains("last_distilled")); // None skipped
        assert!(!text.contains("cadence")); // None skipped
    }

    #[test]
    fn pre_cadence_meta_loads_without_the_field() {
        // A meta.toml written by an older OmniProj has no `[cadence]` — it must still
        // parse (additive optional field, no schema bump).
        let text = r#"
path = "/Users/x/git/foo"
name = "foo"
hash = "deadbeefdeadbeef"
added_at = "2026-06-04T00:00:00Z"
"#;
        let m: ProjectMeta = toml::from_str(text).unwrap();
        assert_eq!(m.name, "foo");
        assert!(m.cadence.is_none());
    }

    #[test]
    fn cadence_roundtrips_and_partial_override_is_allowed() {
        let mut m = meta("/p");
        m.cadence = Some(Cadence {
            refresh_floor_secs: Some(3600),
            depth: None, // only one knob set
        });
        let text = toml::to_string_pretty(&m).unwrap();
        let back: ProjectMeta = toml::from_str(&text).unwrap();
        assert_eq!(m, back);
        let c = back.cadence.unwrap();
        assert_eq!(c.refresh_floor_secs, Some(3600));
        assert!(c.depth.is_none());
        assert!(!c.is_empty());
    }

    #[test]
    fn cwd_resolves_to_longest_prefix() {
        let metas = vec![meta("/Users/x/git"), meta("/Users/x/git/foo")];
        let hit = best_prefix_match("/Users/x/git/foo/sub", metas).unwrap();
        assert_eq!(hit.path, "/Users/x/git/foo"); // longest prefix wins, not the shorter parent
    }

    #[test]
    fn cwd_no_match_is_none() {
        let metas = vec![meta("/Users/x/git/foo")];
        assert!(best_prefix_match("/Users/y/other", metas).is_none());
        // a sibling sharing a string prefix but not a path boundary must not match
        assert!(best_prefix_match("/Users/x/git/foobar", vec![meta("/Users/x/git/foo")]).is_none());
    }
}
