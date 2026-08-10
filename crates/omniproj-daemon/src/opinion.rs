//! Shared second-opinion orchestration.
//!
//! CLI and dashboard both use this path so the counter-convergent view is grounded
//! on fresh capture, passes the deterministic verify gate, and lands in the store
//! as a revertable commit.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use omniproj_distill::VerifyReport;

#[derive(Default)]
pub struct OpinionOpts<'a> {
    /// Model as `provider/model`; `None` falls back to config / env.
    pub model: Option<&'a str>,
    /// User-model dimensions to ignore. Empty means ignore all enabled dimensions.
    pub ignore: Vec<String>,
}

pub struct OpinionOutput {
    pub name: String,
    pub text: String,
    pub verify: VerifyReport,
    pub provider_label: String,
    pub ignored: Vec<String>,
    pub path: PathBuf,
}

/// Generate and persist a grounded second opinion for a project.
pub async fn generate_opinion(
    dir: &Path,
    opts: OpinionOpts<'_>,
    log: impl Fn(&str),
) -> Result<OpinionOutput> {
    let abs = std::fs::canonicalize(dir)
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .to_string();
    let name = Path::new(&abs)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let hash = omniproj_core::project_hash(&abs);

    // The convergent view to challenge. An opinion without a briefing has no target.
    let auto = omniproj_core::auto_dir(&hash);
    let briefing = std::fs::read_to_string(auto.join("briefing.md")).unwrap_or_default();
    if briefing.trim().is_empty() {
        anyhow::bail!("no briefing for {name} yet — run `omniproj briefing` first");
    }
    let decisions = std::fs::read_to_string(auto.join("decisions.md")).unwrap_or_default();
    let open = std::fs::read_to_string(auto.join("open.md")).unwrap_or_default();

    // Fresh substrate so the contrast view grounds on live facts, not on the
    // briefing's own claims. Capture itself does not call an LLM.
    let sub = omniproj_capture::capture(Path::new(&abs))?;
    // Same outbound-privacy policy as distill (spec §5, W1-1): the opinion pass
    // sends the digest to the provider too.
    let digest_opts = omniproj_capture::DigestOpts {
        privacy: omniproj_distill::resolve_privacy(false),
        ..Default::default()
    };
    let digest = omniproj_capture::render_digest(&sub, &digest_opts);
    let facts = sub.factsheet();

    // Counter-convergence axes (spec §4.5): explicit ignore list wins; otherwise
    // ignore all enabled dimensions, which is the maximally "unlike you" lens.
    let user_model = omniproj_core::UserModel::load();
    let ignored = if opts.ignore.is_empty() {
        user_model.enabled().map(|d| d.name.clone()).collect()
    } else {
        opts.ignore
    };
    let ignored_refs: Vec<&str> = ignored.iter().map(String::as_str).collect();
    let kept = user_model.render_for_prompt(&ignored_refs);

    let resolved = omniproj_distill::resolve(opts.model)
        .context("could not resolve an LLM provider (see `omniproj providers`)")?;
    let provider_label = format!("{}/{}", resolved.provider_name, resolved.model);
    log(&format!(
        "second opinion with {provider_label} — ignoring [{}]…",
        if ignored.is_empty() {
            "—".to_string()
        } else {
            ignored.join(", ")
        }
    ));

    let raw = omniproj_distill::second_opinion(
        &omniproj_distill::OpinionInput {
            briefing: &briefing,
            decisions: &decisions,
            open: &open,
            digest: &digest,
            facts: &facts,
            ignored_dims: &ignored,
            kept_model: &kept,
        },
        &resolved.provider,
    )
    .await?;

    let (text, verify) = verify_opinion(&raw, &facts);
    if !verify.is_clean() {
        log(&format!(
            "verify: flagged {} unverified hash(es), {} unverified path(s)",
            verify.flagged.len(),
            verify.flagged_paths.len()
        ));
    }

    omniproj_core::ensure_home()?;
    let path = auto.join("opinion.md");
    omniproj_core::store_txn(|| -> Result<()> {
        std::fs::write(&path, format!("{text}\n"))?;
        omniproj_core::commit_all(&format!("opinion {name}"));
        Ok(())
    })?;

    Ok(OpinionOutput {
        name,
        text,
        verify,
        provider_label,
        ignored,
        path,
    })
}

fn verify_opinion(raw: &str, facts: &omniproj_core::FactSheet) -> (String, VerifyReport) {
    let (text, hash_report) = omniproj_distill::verify::verify_hashes(raw, facts);
    let (text, path_report) = omniproj_distill::verify::verify_paths(&text, facts);
    let mut flagged = hash_report.flagged;
    let mut flagged_paths = path_report.flagged_paths;
    flagged.sort();
    flagged.dedup();
    flagged_paths.sort();
    flagged_paths.dedup();
    (
        text,
        VerifyReport {
            flagged,
            flagged_paths,
        },
    )
}
