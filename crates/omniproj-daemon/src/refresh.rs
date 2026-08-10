//! The idempotent refresh step: capture a project, compare its change fingerprint
//! against the stored cursor, and re-distill only when something moved (spec §5).
//!
//! `force = true` is the explicit `omniproj briefing` path (always distill, always
//! print). `force = false` is the staleness floor used by `omniproj refresh` and the
//! background timer: distill on change, otherwise [`RefreshOutcome::UpToDate`] with
//! no LLM call and no output (charter §4d — quiet when there's nothing to say).

use anyhow::{Context, Result};
use omniproj_capture::Substrate;
use omniproj_core::Fingerprint;
use omniproj_distill::VerifyReport;

/// Knobs for one refresh.
#[derive(Default)]
pub struct RefreshOpts<'a> {
    /// Distill unconditionally, ignoring the staleness floor (the `briefing` path).
    pub force: bool,
    /// Model as `provider/model`; `None` falls back to config / env (spec §4.1).
    pub model: Option<&'a str>,
    /// Reasoning depth override ("shallow"/"deep"); `None` falls back to config
    /// `default_depth`, then shallow (spec §5.2 推理深度 knob).
    pub depth: Option<&'a str>,
    /// Disable outbound secret redaction for this run (CLI `--no-redact`, W1-1).
    /// Deny-list path filtering still applies. Default false → redaction on.
    pub no_redact: bool,
}

/// A completed distillation, with enough for the caller to render a summary.
pub struct Distilled {
    pub name: String,
    pub hash: String,
    pub briefing: String,
    pub verify: VerifyReport,
    pub provider_label: String,
    pub session_count: usize,
    pub had_git: bool,
}

/// The outcome of a refresh. Only [`Distilled`](RefreshOutcome::Distilled) touches an LLM.
pub enum RefreshOutcome {
    /// Substrate changed (or `force`) → distilled and wrote the state files.
    Distilled(Distilled),
    /// Registered and fresh — fingerprint matched the cursor. No LLM, no output.
    UpToDate { name: String, hash: String },
    /// Nothing to distill: no git repo and no matching sessions.
    NoSubstrate { name: String },
    /// `omniproj refresh` on a dir that was never `omniproj add`ed — no cursor to compare,
    /// so the floor can't apply. (The `force` path distills these anyway.)
    Unregistered { name: String, hash: String },
}

/// Current change fingerprint of a captured substrate (spec §5): short `HEAD` +
/// newest session mtime. Sessions are stored ascending by mtime, so the last is newest.
fn fingerprint(sub: &Substrate) -> Fingerprint {
    Fingerprint {
        head: sub.git.as_ref().map(|g| g.head.clone()),
        status_digest: sub.git.as_ref().map(|g| g.status_digest.clone()),
        latest_session_mtime: sub.sessions.last().map(|s| s.mtime),
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Capture → (staleness gate) → resolve provider → distill → verify → write →
/// version the store.
///
/// `log` receives human-readable progress lines (the CLI prints them with a
/// `[omniproj]` prefix; the daemon routes them to its log). Pure data comes back in
/// [`RefreshOutcome`] so the orchestration stays UI-agnostic.
///
/// The post-capture core is factored into [`distill_and_write`] so it can be driven
/// with an injected provider (the E2E test path uses a mock LLM — this fn resolves
/// the real one from config/env, which needs network + keys the tests don't have).
pub async fn refresh_project(
    dir: &std::path::Path,
    opts: RefreshOpts<'_>,
    log: impl Fn(&str),
) -> Result<RefreshOutcome> {
    let sub = omniproj_capture::capture(dir)?;
    let fp = fingerprint(&sub);

    // Nothing captured at all → never an LLM call.
    if sub.sessions.is_empty() && sub.git.is_none() {
        return Ok(RefreshOutcome::NoSubstrate { name: sub.name });
    }

    // Staleness floor (skipped when forced). Needs a cursor, which only registered
    // projects have.
    if !opts.force {
        match omniproj_core::load_meta(&sub.hash) {
            None => {
                return Ok(RefreshOutcome::Unregistered {
                    name: sub.name,
                    hash: sub.hash,
                })
            }
            Some(meta) if !fp.is_stale(&meta) => {
                return Ok(RefreshOutcome::UpToDate {
                    name: sub.name,
                    hash: sub.hash,
                });
            }
            Some(_) => {} // stale → fall through and distill
        }
    }

    let resolved = omniproj_distill::resolve(opts.model)
        .context("could not resolve an LLM provider (see `omniproj providers`)")?;
    let provider_label = format!("{}/{}", resolved.provider_name, resolved.model);

    let distilled =
        distill_and_write(&sub, &fp, &opts, &resolved.provider, &provider_label, log).await?;
    Ok(RefreshOutcome::Distilled(distilled))
}

/// The post-capture core (spec §5): render the outbound digest, distill with the
/// given `provider`, run the deterministic verify gate (spec §5.2), persist the
/// verify report, and write+cursor+commit the state files as ONE store transaction.
///
/// Split out from [`refresh_project`] so the whole pipeline can be exercised with an
/// injected mock provider — no real LLM, no network — in the E2E test. `sub` is an
/// already-captured substrate; `fp` its change fingerprint; `provider_label` is only
/// used for logging + the returned [`Distilled`].
pub async fn distill_and_write(
    sub: &Substrate,
    fp: &Fingerprint,
    opts: &RefreshOpts<'_>,
    provider: &impl omniproj_distill::LlmProvider,
    provider_label: &str,
    log: impl Fn(&str),
) -> Result<Distilled> {
    log(&format!(
        "{} — {} sessions (claude {}, codex {}){}",
        sub.name,
        sub.sessions.len(),
        sub.claude_n,
        sub.codex_n,
        if sub.git.is_some() { "" } else { " — no git" }
    ));

    // Outbound-privacy policy (spec §5, W1-1): deny-listed paths dropped + secret
    // shapes masked before the digest reaches the provider. `--no-redact` disables
    // masking only; the deny-list still applies.
    let privacy = omniproj_distill::resolve_privacy(opts.no_redact);
    let digest_opts = omniproj_capture::DigestOpts {
        privacy: privacy.clone(),
        ..Default::default()
    };
    let digest = omniproj_capture::render_digest(sub, &digest_opts);
    let facts = sub.factsheet();
    // Per-project heuristics learned from past corrections (spec §5.3). Empty if none.
    let learned =
        std::fs::read_to_string(omniproj_core::learned_path(&sub.hash)).unwrap_or_default();
    if !learned.trim().is_empty() {
        log("applying learned heuristics from learned.md");
    }
    // Enabled user-model dimensions as a presentation lens (spec §4.4). Empty if none.
    let user_model = omniproj_core::UserModel::load().render_for_prompt(&[]);
    if !user_model.is_empty() {
        log("applying user model from ~/.omniproj/user/model.md");
    }

    let depth = omniproj_distill::resolve_depth(opts.depth);
    log(&format!(
        "distilling with {provider_label} ({}) …",
        depth.as_str()
    ));

    let raw = match depth {
        omniproj_distill::Depth::Shallow => {
            omniproj_distill::distill(&digest, &facts, &learned, &user_model, provider).await?
        }
        omniproj_distill::Depth::Deep => {
            // Sessions outside the digest window, newest-first, for the map pass.
            let older = omniproj_capture::older_session_texts(sub, 4, 1400, 12_000, &privacy);
            omniproj_distill::distill_deep(
                &digest,
                &facts,
                &learned,
                &user_model,
                &older,
                provider,
                &log,
            )
            .await?
        }
    };
    // Deterministic verify gate (spec §5.2): annotate hashes the FactSheet can't vouch for.
    let (out, report) = omniproj_distill::verify_output(&raw, &facts);
    if report.is_clean() {
        log("verify: clean — no unverified commit hashes");
    } else {
        log(&format!(
            "verify: flagged {} unverified hash(es): {}",
            report.flagged.len(),
            report.flagged.join(", ")
        ));
    }
    if !report.flagged_paths.is_empty() {
        log(&format!(
            "verify: flagged {} unverified path(s): {}",
            report.flagged_paths.len(),
            report.flagged_paths.join(", ")
        ));
    }

    omniproj_core::ensure_home()?;

    // Persist the verify report for quality observation (spec §5.2 — cache/ is
    // derived + gitignored). Best-effort: a failed report write never blocks state.
    let cache = omniproj_core::cache_dir(&sub.hash);
    let _ = std::fs::create_dir_all(&cache);
    let report_json = serde_json::json!({
        "at": now_rfc3339(),
        "clean": report.is_clean(),
        "flagged_hashes": report.flagged,
        "flagged_paths": report.flagged_paths,
    });
    let _ = std::fs::write(
        cache.join("verify-report.json"),
        serde_json::to_string_pretty(&report_json).unwrap_or_default(),
    );

    // Write state + advance the cursor + commit as ONE store transaction (spec §5
    // provenance): without the lock, a concurrent distill's `commit_all` would smear
    // both projects' files into a single commit, breaking per-distill revertability.
    let head = sub.git.as_ref().map(|g| g.head.as_str());
    let status_digest = sub.git.as_ref().map(|g| g.status_digest.as_str());
    let outcome =
        omniproj_core::store_txn(|| -> anyhow::Result<omniproj_distill::WriteOutcome> {
            let outcome = omniproj_distill::write_outputs(&sub.hash, &out)?;
            omniproj_core::set_last_distilled(
                &sub.hash,
                &now_rfc3339(),
                head,
                status_digest,
                fp.latest_session_mtime,
            );
            omniproj_core::commit_all(&format!(
                "distill {}{}",
                sub.name,
                head.map(|h| format!(" @ {h}")).unwrap_or_default()
            ));
            Ok(outcome)
        })?;
    // Conflict-aware write (charter §5 原则4): a hand-edited auto/ file was preserved
    // and the AI version parked in <f>.incoming for explicit reconcile — never blocks.
    for f in &outcome.conflicts {
        log(&format!(
            "user edit detected in {f} — wrote AI version to {f}.incoming; run `omniproj reconcile {}`",
            sub.name
        ));
    }
    log(&format!("wrote {}", outcome.dir.display()));

    Ok(Distilled {
        name: sub.name.clone(),
        hash: sub.hash.clone(),
        briefing: out.briefing,
        verify: report,
        provider_label: provider_label.to_string(),
        session_count: sub.sessions.len(),
        had_git: sub.git.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};

    /// A mock LLM: returns one canned completion, no network. Implements the real
    /// `LlmProvider` trait so it drops straight into [`distill_and_write`] — the seam
    /// that lets the whole capture→distill→verify→commit pipeline run without keys.
    struct MockLlm {
        response: String,
    }

    impl omniproj_distill::LlmProvider for MockLlm {
        async fn complete(&self, _system: &str, _user: &str) -> anyhow::Result<String> {
            Ok(self.response.clone())
        }
    }

    /// `OMNIPROJ_HOME` is a process-global env var; serialize the tests that repoint it
    /// so they don't read each other's home mid-run.
    fn env_lock() -> &'static Mutex<()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
    }

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("omniproj-e2e-{}-{}-{}", std::process::id(), tag, n));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git on PATH")
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// A temp git repo with one commit; returns the worktree path.
    fn temp_repo() -> std::path::PathBuf {
        let dir = unique_dir("repo");
        run_git(&dir, &["init", "-q", "-b", "main"]);
        run_git(&dir, &["config", "user.name", "omniproj-test"]);
        run_git(&dir, &["config", "user.email", "test@local"]);
        // Neutralize global config that would break commits on CI/contributor boxes.
        run_git(&dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("main.rs"), "fn main() {}\n").unwrap();
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "seed commit"]);
        dir
    }

    /// Format a canned three-section completion citing `hash` in the briefing.
    fn canned(hash: &str) -> String {
        format!(
            "===BRIEFING===\n分支 main,HEAD {hash}。回来第一件事:继续。\n\
             ===DECISIONS===\n- 选定方案 A\n\
             ===OPEN===\n- 待办:补文档\n"
        )
    }

    #[tokio::test]
    // The guard only serializes the process-global OMNIPROJ_HOME env var across test
    // threads and must stay held while distill_and_write() reads it; there is no
    // re-entrant contention on this std Mutex, so holding it across the await is safe.
    #[allow(clippy::await_holding_lock)]
    async fn e2e_clean_verify_writes_and_commits() {
        let _guard = env_lock().lock().unwrap();
        let home = unique_dir("home");
        std::env::set_var("OMNIPROJ_HOME", &home);
        omniproj_core::ensure_home().unwrap();

        let repo = temp_repo();
        let sub = omniproj_capture::capture(&repo).unwrap();
        let fp = fingerprint(&sub);
        // Cite the real short HEAD so the verify gate stays clean.
        let head = sub.git.as_ref().unwrap().head.clone();
        let mock = MockLlm {
            response: canned(&head),
        };
        let opts = RefreshOpts {
            force: true,
            ..Default::default()
        };

        let distilled = distill_and_write(&sub, &fp, &opts, &mock, "mock/test", |_| {})
            .await
            .expect("pipeline succeeds with the mock");

        // briefing.md landed in auto/.
        let briefing = omniproj_core::auto_dir(&sub.hash).join("briefing.md");
        assert!(briefing.exists(), "briefing.md written");
        assert!(std::fs::read_to_string(&briefing).unwrap().contains(&head));

        // verify-report.json exists and is clean (the cited hash was in the FactSheet).
        let report = omniproj_core::cache_dir(&sub.hash).join("verify-report.json");
        assert!(report.exists(), "verify report persisted");
        let rj: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(rj["clean"], serde_json::json!(true), "report: {rj}");
        assert!(distilled.verify.is_clean());

        // The store git repo gained a distill commit.
        let log = Command::new("git")
            .arg("-C")
            .arg(&home)
            .args(["log", "--oneline"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&log.stdout);
        assert!(
            log.contains(&format!("distill {}", sub.name)),
            "store has a distill commit; log:\n{log}"
        );

        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    // See e2e_clean_verify_writes_and_commits: guard serializes the OMNIPROJ_HOME env
    // var across test threads; safe to hold across the await.
    #[allow(clippy::await_holding_lock)]
    async fn e2e_fabricated_hash_is_flagged() {
        let _guard = env_lock().lock().unwrap();
        let home = unique_dir("home");
        std::env::set_var("OMNIPROJ_HOME", &home);
        omniproj_core::ensure_home().unwrap();

        let repo = temp_repo();
        let sub = omniproj_capture::capture(&repo).unwrap();
        let fp = fingerprint(&sub);
        // A hash-shaped token (hex + a letter, 13 chars) that cannot prefix the real
        // 40-char SHA — the gate must flag it (spec §5.2, the DeepSeek failure mode).
        let fake = "abcdef1234567";
        let mock = MockLlm {
            response: canned(fake),
        };
        let opts = RefreshOpts {
            force: true,
            ..Default::default()
        };

        let distilled = distill_and_write(&sub, &fp, &opts, &mock, "mock/test", |_| {})
            .await
            .expect("pipeline still succeeds; the gate annotates, not aborts");

        assert!(!distilled.verify.is_clean(), "fabricated hash flagged");
        assert!(distilled.verify.flagged.iter().any(|h| h == fake));

        let report = omniproj_core::cache_dir(&sub.hash).join("verify-report.json");
        let rj: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(rj["clean"], serde_json::json!(false), "report: {rj}");
        // The written briefing carries the ⚠未核实 annotation next to the fake hash.
        let briefing =
            std::fs::read_to_string(omniproj_core::auto_dir(&sub.hash).join("briefing.md"))
                .unwrap();
        assert!(
            briefing.contains(&format!(
                "{fake}{}",
                omniproj_distill::verify::UNVERIFIED_MARK
            )),
            "unverified mark appended in briefing: {briefing}"
        );

        std::env::remove_var("OMNIPROJ_HOME");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&repo);
    }
}
