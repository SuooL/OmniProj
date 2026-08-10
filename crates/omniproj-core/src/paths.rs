//! `~/.omniproj` on-disk layout + project identity (spec §4.1).
//!
//! Project identity = sha256 of the tracked directory's absolute path (git optional).
//! State lives outside the user's repo (charter §5 原则2): never written into their project.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn short_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable 16-hex project id from an absolute directory path.
pub fn project_hash(abs_path: &str) -> String {
    short_sha256_hex(abs_path.as_bytes())
}

/// Stable 16-hex digest for captured content fingerprints. This is intentionally
/// separate from Rust's `DefaultHasher`, whose output is not a persistence contract.
pub fn content_hash(text: &str) -> String {
    short_sha256_hex(text.as_bytes())
}

/// `~/.omniproj` — the local state root (markdown + git, portable; charter §5 原则1).
///
/// The `OMNIPROJ_HOME` env var overrides the default `~/.omniproj` when set and non-empty.
/// Read fresh on every call (no caching) so tests can point the whole store at a
/// tempdir without racing a one-time init. Also generally useful for advanced users
/// who keep state on an external volume.
pub fn omniproj_home() -> PathBuf {
    match std::env::var_os("OMNIPROJ_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => dirs::home_dir()
            .expect("could not resolve home directory")
            .join(".omniproj"),
    }
}

/// `~/.omniproj/projects/<hash>/`
pub fn project_dir(hash: &str) -> PathBuf {
    omniproj_home().join("projects").join(hash)
}

/// `~/.omniproj/projects/<hash>/auto/` — AI-written state. User content lives in `notes/`
/// (charter §5 原则4); the thin v1 loop only writes `auto/`.
pub fn auto_dir(hash: &str) -> PathBuf {
    project_dir(hash).join("auto")
}

/// `~/.omniproj/projects/<hash>/notes/` — USER-written state, AI never overwrites it
/// (charter §5 原则4). Physically separate from `auto/`; surfaced read-only by `recall`.
pub fn notes_dir(hash: &str) -> PathBuf {
    project_dir(hash).join("notes")
}

/// `~/.omniproj/projects/<hash>/cache/` — derived, regenerable, NOT versioned
/// (gitignored at store init). Verify reports land here (spec §5.2).
pub fn cache_dir(hash: &str) -> PathBuf {
    project_dir(hash).join("cache")
}

/// `~/.omniproj/projects/<hash>/learned.md` — per-project heuristics distilled from
/// user corrections, injected into future distillation (spec §5.3, self-iteration).
pub fn learned_path(hash: &str) -> PathBuf {
    project_dir(hash).join("learned.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_path_sensitive() {
        let a = project_hash("/Users/x/git/foo");
        let b = project_hash("/Users/x/git/foo");
        let c = project_hash("/Users/x/git/bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn content_hash_is_stable_and_content_sensitive() {
        let a = content_hash(" M src/lib.rs\n");
        let b = content_hash(" M src/lib.rs\n");
        let c = content_hash("");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
    }

    /// `OMNIPROJ_HOME` overrides the default when set (used by tests + advanced users).
    /// Holds the shared env guard because `OMNIPROJ_HOME` is process-global and other
    /// tests (e.g. `store`) mutate it concurrently.
    #[test]
    fn omniproj_home_honors_env_override() {
        let _g = crate::env_guard();
        std::env::set_var("OMNIPROJ_HOME", "/tmp/omniproj-override-test");
        assert_eq!(
            omniproj_home(),
            PathBuf::from("/tmp/omniproj-override-test")
        );
        std::env::remove_var("OMNIPROJ_HOME");
        // Falls back to ~/.omniproj when unset.
        assert!(omniproj_home().ends_with(".omniproj"));
    }
}
