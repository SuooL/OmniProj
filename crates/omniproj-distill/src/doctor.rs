//! `omniproj doctor` — a read-only setup diagnostic (W2-3).
//!
//! Checks are safe to run anytime: they only READ `~/.omniproj`, load config, and do
//! one best-effort network reachability probe. Nothing is written or mutated. The
//! status-classification and formatting logic is pure so it can be unit-tested
//! without touching the filesystem or the network.

use std::time::Duration;

use crate::config;

/// Outcome of a single diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Everything is in order.
    Pass,
    /// Non-fatal: something to be aware of, but OmniProj can still work.
    Warn,
    /// A real problem that will block distillation.
    Fail,
    /// Not applicable / not probed (e.g. connectivity for a local provider that
    /// isn't running).
    Skip,
}

impl CheckStatus {
    pub fn label(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "PASS",
            CheckStatus::Warn => "WARN",
            CheckStatus::Fail => "FAIL",
            CheckStatus::Skip => "SKIP",
        }
    }
}

/// A single named diagnostic result with a human-readable detail line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

impl Check {
    fn new(name: &str, status: CheckStatus, detail: impl Into<String>) -> Self {
        Check {
            name: name.to_string(),
            status,
            detail: detail.into(),
        }
    }

    /// One aligned line, e.g. `[PASS] store              ~/.omniproj is writable`.
    pub fn format_line(&self) -> String {
        format!(
            "[{}] {:<18} {}",
            self.status.label(),
            self.name,
            self.detail
        )
    }
}

/// Whether the store directory is writable, probed by creating and removing a
/// temp file. Best-effort: any IO error means "not writable".
fn dir_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".omniproj-doctor-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Check 1: `~/.omniproj` exists, is writable, and its `SCHEMA_VERSION` is readable
/// and understood by this binary (W2-2).
fn check_home() -> Check {
    let home = omniproj_core::omniproj_home();
    if !home.exists() {
        return Check::new(
            "store",
            CheckStatus::Warn,
            format!(
                "{} does not exist yet — it's created on first use (e.g. `omniproj add <repo>`)",
                home.display()
            ),
        );
    }
    if !dir_writable(&home) {
        return Check::new(
            "store",
            CheckStatus::Fail,
            format!("{} exists but is not writable", home.display()),
        );
    }
    let vpath = home.join(omniproj_core::SCHEMA_VERSION_FILE);
    match std::fs::read_to_string(&vpath) {
        Ok(text) => match text.trim().parse::<u32>() {
            Ok(v) => classify_schema(
                v,
                omniproj_core::CURRENT_SCHEMA_VERSION,
                &home.display().to_string(),
            ),
            Err(_) => Check::new(
                "store",
                CheckStatus::Fail,
                format!("{} is not a valid version number", vpath.display()),
            ),
        },
        // Existing store predating versioning: adopted as v1 on next init (W2-2),
        // so its absence is informational, not an error.
        Err(_) => Check::new(
            "store",
            CheckStatus::Warn,
            format!(
                "{} has no SCHEMA_VERSION yet — it will be adopted as v{} on next run",
                home.display(),
                omniproj_core::CURRENT_SCHEMA_VERSION
            ),
        ),
    }
}

/// Pure: compare an on-disk schema version against the one this binary understands.
fn classify_schema(on_disk: u32, current: u32, home: &str) -> Check {
    if on_disk > current {
        Check::new(
            "store",
            CheckStatus::Fail,
            format!(
                "{home} is schema v{on_disk} but this binary understands v{current} — upgrade omniproj"
            ),
        )
    } else if on_disk < current {
        Check::new(
            "store",
            CheckStatus::Warn,
            format!("{home} is schema v{on_disk}; it will migrate to v{current} on next run"),
        )
    } else {
        Check::new(
            "store",
            CheckStatus::Pass,
            format!("{home} writable, schema v{on_disk}"),
        )
    }
}

/// Check 2: config.toml. Loading always succeeds (built-in defaults), so this
/// reports whether a user config file is present.
fn check_config() -> Check {
    let path = config::config_path();
    if path.exists() {
        Check::new(
            "config",
            CheckStatus::Pass,
            format!("{} loaded", path.display()),
        )
    } else {
        Check::new(
            "config",
            CheckStatus::Pass,
            "no config.toml — using built-in defaults (`omniproj init` to customize)",
        )
    }
}

/// Pure: classify the API-key situation for the resolved default model.
fn classify_model_key(
    model_str: &str,
    provider_name: &str,
    api_key_env: Option<&str>,
    key_present: bool,
    is_local: bool,
) -> Check {
    if is_local {
        return Check::new(
            "model",
            CheckStatus::Pass,
            format!("default `{model_str}` → local provider `{provider_name}` (no API key needed)"),
        );
    }
    match api_key_env {
        None => Check::new(
            "model",
            CheckStatus::Pass,
            format!("default `{model_str}` → `{provider_name}` needs no API key"),
        ),
        Some(env) if key_present => Check::new(
            "model",
            CheckStatus::Pass,
            format!("default `{model_str}` → `{provider_name}` key {env} is set"),
        ),
        Some(env) => Check::new(
            "model",
            CheckStatus::Fail,
            format!(
                "default `{model_str}` → `{provider_name}` needs {env} (unset). \
                 Set it, or use a local Ollama model (`omniproj providers`)"
            ),
        ),
    }
}

/// Check 3: resolve the default model and report whether its provider has a key.
/// Returns the resolved `(provider_name, base_url, is_local)` for the connectivity
/// probe to reuse, or `None` if the provider is unknown.
fn check_model() -> (Check, Option<(String, String, bool)>) {
    let model_str = config::default_model_string();
    let pname = model_str
        .split_once('/')
        .map(|(p, _)| p)
        .unwrap_or(model_str.as_str())
        .to_string();

    let providers = config::list();
    let Some(p) = providers.into_iter().find(|p| p.name == pname) else {
        return (
            Check::new(
                "model",
                CheckStatus::Fail,
                format!(
                    "default `{model_str}` names unknown provider `{pname}` (`omniproj providers`)"
                ),
            ),
            None,
        );
    };
    let is_local = p.base_url.contains("localhost") || p.base_url.contains("127.0.0.1");
    let check = classify_model_key(
        &model_str,
        &pname,
        p.api_key_env.as_deref(),
        p.key_present,
        is_local,
    );
    (check, Some((pname, p.base_url, is_local)))
}

/// Pure: turn a reachability probe into a connectivity check status. `reachable`
/// is `None` when we skipped the probe (local provider not running is not a fault).
fn classify_connectivity(
    provider_name: &str,
    base_url: &str,
    is_local: bool,
    reachable: bool,
) -> Check {
    if reachable {
        return Check::new(
            "connectivity",
            CheckStatus::Pass,
            format!("reached {provider_name} at {base_url}"),
        );
    }
    if is_local {
        Check::new(
            "connectivity",
            CheckStatus::Skip,
            format!("local provider {provider_name} not reachable at {base_url} (is it running?)"),
        )
    } else {
        Check::new(
            "connectivity",
            CheckStatus::Warn,
            format!(
                "could not reach {provider_name} at {base_url} (best-effort; network/endpoint?)"
            ),
        )
    }
}

/// Best-effort reachability: any HTTP response (even 401/404) means the endpoint is
/// up. A transport error / timeout within the short window means unreachable.
async fn probe_reachable(base_url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .build()
    else {
        return false;
    };
    client.get(base_url).send().await.is_ok()
}

/// Run every diagnostic and return the results in display order. Read-only; does a
/// single best-effort network probe (labeled as such). Never mutates state.
pub async fn run() -> Vec<Check> {
    let mut checks = vec![check_home(), check_config()];
    let (model_check, resolved) = check_model();
    checks.push(model_check);
    if let Some((pname, base_url, is_local)) = resolved {
        let reachable = probe_reachable(&base_url).await;
        checks.push(classify_connectivity(
            &pname, &base_url, is_local, reachable,
        ));
    }
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_line_is_aligned_and_labeled() {
        let c = Check::new("store", CheckStatus::Pass, "ok");
        let line = c.format_line();
        assert!(line.starts_with("[PASS] "));
        assert!(line.contains("store"));
        assert!(line.ends_with("ok"));
    }

    #[test]
    fn schema_classification_covers_all_orderings() {
        assert_eq!(classify_schema(1, 1, "/h").status, CheckStatus::Pass);
        // Older store → migrates, non-fatal.
        assert_eq!(classify_schema(1, 2, "/h").status, CheckStatus::Warn);
        // Newer store than the binary → refuse (upgrade omniproj).
        assert_eq!(classify_schema(3, 2, "/h").status, CheckStatus::Fail);
    }

    #[test]
    fn model_key_local_provider_needs_no_key() {
        let c = classify_model_key("ollama/llama3.1", "ollama", None, false, true);
        assert_eq!(c.status, CheckStatus::Pass);
        assert!(c.detail.contains("local"));
    }

    #[test]
    fn model_key_present_passes_missing_fails() {
        let present = classify_model_key(
            "deepseek/deepseek-chat",
            "deepseek",
            Some("DEEPSEEK_API_KEY"),
            true,
            false,
        );
        assert_eq!(present.status, CheckStatus::Pass);

        let missing = classify_model_key(
            "deepseek/deepseek-chat",
            "deepseek",
            Some("DEEPSEEK_API_KEY"),
            false,
            false,
        );
        assert_eq!(missing.status, CheckStatus::Fail);
        assert!(missing.detail.contains("DEEPSEEK_API_KEY"));
    }

    #[test]
    fn model_key_no_env_needed_passes() {
        let c = classify_model_key("custom/model", "custom", None, false, false);
        assert_eq!(c.status, CheckStatus::Pass);
    }

    #[test]
    fn connectivity_status_depends_on_reach_and_locality() {
        // Reachable → PASS regardless of locality.
        assert_eq!(
            classify_connectivity("p", "http://x", false, true).status,
            CheckStatus::Pass
        );
        // Unreachable remote → WARN (best-effort, not fatal).
        assert_eq!(
            classify_connectivity("p", "http://x", false, false).status,
            CheckStatus::Warn
        );
        // Unreachable local → SKIP (probably just not running).
        assert_eq!(
            classify_connectivity("ollama", "http://localhost:11434/v1", true, false).status,
            CheckStatus::Skip
        );
    }
}
