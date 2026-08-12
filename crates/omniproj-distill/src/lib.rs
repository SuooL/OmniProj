//! omniproj-distill — Layer 2 (spec §5). Turn a substrate digest into the three state files.
//! The only crate that links an LLM.

// Legacy surface: the distill tests still version via the deprecated `commit_all`
// helper pending migration to `commit_paths_checked`. Silence the staged-migration
// deprecation warnings, matching `omniproj-core`'s own allows.
#![allow(deprecated)]

pub mod breakdown;
pub mod clarify;
pub mod config;
pub mod prompt;
pub mod provider;
pub mod refine;
pub mod verify;

pub use breakdown::{breakdown, parse_steps};
pub use clarify::{clarify_round, count_rounds_within, render_round};
pub use config::{
    config_path, default_model_string, is_local_provider, list, resolve, resolve_clarify,
    resolve_depth, resolve_privacy, Depth, PrivacyConfig, ProviderKind, ProviderStatus, Resolved,
    CONFIG_TEMPLATE,
};
pub use provider::{AnyProvider, LlmProvider};
pub use refine::refine;
pub use verify::VerifyReport;

use omniproj_core::FactSheet;
use std::path::PathBuf;

pub struct DistillOutput {
    pub briefing: String,
    pub decisions: String,
    pub open: String,
}

/// Run distillation: digest (+ grounded FactSheet + learned heuristics + user model)
/// -> three sections. The FactSheet is injected so the model cites commits only from
/// a verified whitelist (spec §5.2); `learned` carries per-project presentation
/// preferences from past corrections (spec §5.3); `user_model` carries the enabled
/// profile dimensions as a presentation lens (spec §4.4). Output still goes through
/// `verify_output`. `learned` / `user_model` may be empty.
pub async fn distill(
    digest: &str,
    facts: &FactSheet,
    learned: &str,
    user_model: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<DistillOutput> {
    let raw = provider
        .complete(
            prompt::SYSTEM_PROMPT,
            &prompt::user_message(digest, facts, learned, user_model),
        )
        .await?;
    Ok(parse_output(&raw))
}

/// Deterministic verify gate (spec §5.2): annotate any commit hash across the
/// three sections that the FactSheet can't vouch for. No LLM. Returns the
/// annotated output plus a combined report of what was flagged.
pub fn verify_output(out: &DistillOutput, facts: &FactSheet) -> (DistillOutput, VerifyReport) {
    // Two deterministic passes per section: hash whitelist, then path whitelist
    // (spec §5.2 "hash/数字/路径" — numbers remain future work).
    let gate = |text: &str, flagged: &mut Vec<String>, flagged_paths: &mut Vec<String>| {
        let (t, r1) = verify::verify_hashes(text, facts);
        let (t, r2) = verify::verify_paths(&t, facts);
        flagged.extend(r1.flagged);
        flagged_paths.extend(r2.flagged_paths);
        t
    };
    let mut flagged = Vec::new();
    let mut flagged_paths = Vec::new();
    let briefing = gate(&out.briefing, &mut flagged, &mut flagged_paths);
    let decisions = gate(&out.decisions, &mut flagged, &mut flagged_paths);
    let open = gate(&out.open, &mut flagged, &mut flagged_paths);
    flagged.sort();
    flagged.dedup();
    flagged_paths.sort();
    flagged_paths.dedup();
    (
        DistillOutput {
            briefing,
            decisions,
            open,
        },
        VerifyReport {
            flagged,
            flagged_paths,
        },
    )
}

fn section(raw: &str, start: &str, end: Option<&str>) -> Option<String> {
    let s = raw.find(start)? + start.len();
    let rest = &raw[s..];
    let body = match end.and_then(|e| rest.find(e)) {
        Some(i) => &rest[..i],
        None => rest,
    };
    Some(body.trim().to_string())
}

/// Split the delimited completion into the three files. Falls back to putting the
/// whole text in `briefing` if the model didn't honor the markers.
fn parse_output(raw: &str) -> DistillOutput {
    let briefing = section(raw, "===BRIEFING===", Some("===DECISIONS==="));
    let decisions = section(raw, "===DECISIONS===", Some("===OPEN==="));
    let open = section(raw, "===OPEN===", None);

    match briefing {
        Some(b) => DistillOutput {
            briefing: b,
            decisions: decisions.unwrap_or_default(),
            open: open.unwrap_or_default(),
        },
        None => DistillOutput {
            briefing: raw.trim().to_string(),
            decisions: String::new(),
            open: String::new(),
        },
    }
}

/// Outcome of [`write_outputs`]: where the files landed, plus any `auto/` file whose
/// AI version was diverted to a `.incoming` sibling because the user hand-edited it
/// (charter §5 原则4 — never silent-merge; a pending reconcile).
pub struct WriteOutcome {
    pub dir: PathBuf,
    /// Basenames (e.g. `"briefing.md"`) written to `<f>.incoming` instead of overwritten.
    pub conflicts: Vec<String>,
}

/// Write the three files to `~/.omniproj/projects/<hash>/auto/`.
///
/// Conflict-aware (charter §5 原则4, spec §8 reconcile): the `~/.omniproj` store is a git
/// repo and every distill commits `auto/`, so a *dirty* `auto/<f>.md` (uncommitted
/// change vs HEAD) is a **user hand-edit** the store hasn't versioned. Rather than
/// silently clobber it, the AI's new version is written to `auto/<f>.md.incoming` and
/// the user's file is left intact — the presence of a `.incoming` sibling IS the
/// pending-reconcile marker (resolved via `omniproj reconcile`). An already-present
/// `.incoming` (an earlier unresolved conflict) also blocks overwrite. The common case
/// (user never touches `auto/`) is unchanged: plain overwrite.
pub fn write_outputs(hash: &str, out: &DistillOutput) -> anyhow::Result<WriteOutcome> {
    let dir = omniproj_core::auto_dir(hash);
    std::fs::create_dir_all(&dir)?;
    let mut conflicts = Vec::new();
    write_one(hash, &dir, "briefing.md", &out.briefing, &mut conflicts)?;
    if !out.decisions.is_empty() {
        write_one(hash, &dir, "decisions.md", &out.decisions, &mut conflicts)?;
    }
    if !out.open.is_empty() {
        write_one(hash, &dir, "open.md", &out.open, &mut conflicts)?;
    }
    Ok(WriteOutcome { dir, conflicts })
}

/// Write one `auto/` file, diverting to `<fname>.incoming` when the user's copy has an
/// unversioned edit (see [`write_outputs`]).
fn write_one(
    hash: &str,
    dir: &std::path::Path,
    fname: &str,
    content: &str,
    conflicts: &mut Vec<String>,
) -> anyhow::Result<()> {
    let target = dir.join(fname);
    let incoming = dir.join(format!("{fname}.incoming"));
    let rel = format!("projects/{hash}/auto/{fname}");
    // A user edit is pending if the file diverges from HEAD, or a prior conflict's
    // `.incoming` is still unresolved. Either way, never clobber the user's file.
    let user_edit_pending =
        target.exists() && (incoming.exists() || omniproj_core::worktree_diff(&rel).is_some());
    if user_edit_pending {
        std::fs::write(&incoming, content)?;
        conflicts.push(fname.to_string());
    } else {
        std::fs::write(&target, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LlmProvider;
    use omniproj_core::GitFacts;
    use std::sync::Mutex;

    #[test]
    fn parse_three_sections() {
        let raw = "junk\n===BRIEFING===\nB body\n===DECISIONS===\nD body\n===OPEN===\nO body\n";
        let out = parse_output(raw);
        assert_eq!(out.briefing, "B body");
        assert_eq!(out.decisions, "D body");
        assert_eq!(out.open, "O body");
    }

    #[test]
    fn parse_fallback_when_no_markers() {
        let out = parse_output("just a blob");
        assert_eq!(out.briefing, "just a blob");
        assert!(out.decisions.is_empty());
    }

    /// A recording test double for [`LlmProvider`]: returns a canned completion and
    /// captures the exact system+user prompts it was handed, so tests can assert the
    /// distill pipeline wires digest + FactSheet + learned + user_model into the
    /// prompt (spec §5.1/§5.2). Kept test-local — no cargo-feature plumbing.
    struct RecordingMock {
        response: String,
        last_system: Mutex<Option<String>>,
        last_user: Mutex<Option<String>>,
    }

    impl RecordingMock {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                last_system: Mutex::new(None),
                last_user: Mutex::new(None),
            }
        }
    }

    impl LlmProvider for RecordingMock {
        async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
            *self.last_system.lock().expect("mock lock") = Some(system.to_string());
            *self.last_user.lock().expect("mock lock") = Some(user.to_string());
            Ok(self.response.clone())
        }
    }

    fn facts_with(hash: &str) -> FactSheet {
        FactSheet {
            git: Some(GitFacts {
                branch: "main".into(),
                head_short: hash.chars().take(8).collect(),
                commit_hashes: vec![hash.into()],
                file_paths: vec!["src/main.rs".into()],
            }),
        }
    }

    #[tokio::test]
    async fn distill_wires_all_context_into_the_prompt_and_parses_output() {
        let mock = RecordingMock::new(
            "===BRIEFING===\non branch main\n===DECISIONS===\nchose X\n===OPEN===\nblocker Y\n",
        );
        let facts = facts_with("0d906c3f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d");
        let out = distill(
            "SUBSTRATE-DIGEST-MARKER",
            &facts,
            "LEARNED-HEURISTIC-MARKER",
            "USER-MODEL-MARKER",
            &mock,
        )
        .await
        .expect("mock never fails");

        // Output parsed into the three sections.
        assert_eq!(out.briefing, "on branch main");
        assert_eq!(out.decisions, "chose X");
        assert_eq!(out.open, "blocker Y");

        // The user prompt carried every context source (grounded prompting, spec §5.2).
        let user = mock.last_user.lock().unwrap().clone().expect("recorded");
        assert!(user.contains("SUBSTRATE-DIGEST-MARKER"), "digest injected");
        assert!(
            user.contains("0d906c3"),
            "FactSheet commit whitelist injected"
        );
        assert!(
            user.contains("LEARNED-HEURISTIC-MARKER"),
            "learned injected"
        );
        assert!(user.contains("USER-MODEL-MARKER"), "user model injected");
        // The system prompt is the distillation contract.
        let system = mock.last_system.lock().unwrap().clone().expect("recorded");
        assert!(system.contains("认知状态蒸馏器"));
    }

    /// `OMNIPROJ_HOME` is process-global; serialize the tests that repoint it so they
    /// don't read each other's home mid-run (mirrors the daemon E2E env lock).
    fn env_lock() -> &'static Mutex<()> {
        use std::sync::OnceLock;
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
    }

    fn unique_home(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "omniproj-distill-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ))
    }

    fn out(briefing: &str) -> DistillOutput {
        DistillOutput {
            briefing: briefing.to_string(),
            decisions: String::new(),
            open: String::new(),
        }
    }

    /// With NO user edit (the common path), `write_outputs` overwrites `briefing.md`
    /// exactly as before and reports no conflict.
    #[test]
    fn write_outputs_overwrites_when_no_user_edit() {
        let _g = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let home = unique_home("clean");
        std::env::set_var("OMNIPROJ_HOME", &home);
        omniproj_core::ensure_home().unwrap();
        let hash = "deadbeefdeadbeef";

        // First distill: writes + commit versions it.
        let o1 = write_outputs(hash, &out("AI v1")).unwrap();
        assert!(o1.conflicts.is_empty());
        omniproj_core::commit_all("distill v1");

        // Second distill with no user edit → plain overwrite, still no conflict.
        let o2 = write_outputs(hash, &out("AI v2")).unwrap();
        assert!(o2.conflicts.is_empty(), "no conflict without a user edit");
        let briefing = omniproj_core::auto_dir(hash).join("briefing.md");
        assert_eq!(std::fs::read_to_string(&briefing).unwrap(), "AI v2");
        assert!(!omniproj_core::auto_dir(hash)
            .join("briefing.md.incoming")
            .exists());

        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&home);
    }

    /// A hand-edited (uncommitted) `auto/briefing.md` must NOT be clobbered: the AI
    /// version goes to `briefing.md.incoming`, the user's file survives, and the file
    /// is reported as a conflict (charter §5 原则4, spec §8).
    #[test]
    fn write_outputs_diverts_to_incoming_on_user_edit() {
        let _g = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let home = unique_home("conflict");
        std::env::set_var("OMNIPROJ_HOME", &home);
        omniproj_core::ensure_home().unwrap();
        let hash = "cafebabecafebabe";

        // Distill once and commit so the file is versioned (clean vs HEAD).
        write_outputs(hash, &out("AI v1")).unwrap();
        omniproj_core::commit_all("distill v1");

        // User hand-edits the briefing (now dirty vs HEAD).
        let briefing = omniproj_core::auto_dir(hash).join("briefing.md");
        std::fs::write(&briefing, "USER EDIT — do not clobber").unwrap();

        // Next distill must divert, not overwrite.
        let o = write_outputs(hash, &out("AI v2")).unwrap();
        assert_eq!(o.conflicts, vec!["briefing.md".to_string()]);
        assert_eq!(
            std::fs::read_to_string(&briefing).unwrap(),
            "USER EDIT — do not clobber",
            "user file preserved"
        );
        let incoming = omniproj_core::auto_dir(hash).join("briefing.md.incoming");
        assert_eq!(std::fs::read_to_string(&incoming).unwrap(), "AI v2");

        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&home);
    }
}
