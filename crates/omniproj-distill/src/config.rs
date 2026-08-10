//! Provider configuration (Hermes/Goose-style): a built-in catalog of predefined
//! providers + `~/.omniproj/config.toml` to override them or add fully custom ones.
//! A model is selected by a `provider/model` string (e.g. `openrouter/anthropic/claude-...`).

use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::provider::{
    AnthropicProvider, AnyProvider, OpenAiProvider, Tuning, DEFAULT_MAX_OUTPUT_TOKENS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// Native Anthropic `/v1/messages`.
    Anthropic,
    /// OpenAI `/v1/responses` (the reasoning-model surface). Covers OpenAI, OpenRouter,
    /// DeepSeek, Ollama, and any endpoint that speaks Responses via base_url. OmniProj is
    /// all-in on Responses — there is no chat/completions kind.
    Openai,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Openai => "openai",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderDef {
    pub kind: ProviderKind,
    /// Base URL. Optional for the anthropic default; required for custom openai providers.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Env var holding the API key (optional for local endpoints like Ollama).
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Model used when nothing overrides it, as `provider/model`.
    pub default_model: Option<String>,
    /// Reasoning depth when nothing overrides it: "shallow" (default) or "deep".
    pub default_depth: Option<String>,
    /// Global output-token cap for distill calls. `None` → [`DEFAULT_MAX_OUTPUT_TOKENS`]
    /// (raised for reasoning headroom now that OpenAI calls go through Responses).
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderDef>,
    /// Outbound-digest privacy (spec §5, W1-1). Absent → secure defaults.
    #[serde(default)]
    pub privacy: PrivacyConfig,
    /// `[clarify]` — the per-item discussion feature's own model/effort/budget
    /// (cockpit; charter §6 「条目内多轮澄清讨论」例外). Separate from distill because
    /// discussion is multi-round, more model-sensitive, and wants a higher token
    /// budget for reasoning headroom.
    #[serde(default)]
    pub clarify: ClarifyConfig,
}

/// `[clarify]` in `~/.omniproj/config.toml`. All optional: an absent section means
/// clarify falls back to `default_model` with no effort and the clarify token default.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClarifyConfig {
    /// `provider/model` for clarify. `None` → `default_model`. Any provider works;
    /// reasoning effort applies on both kinds (Anthropic + OpenAI Responses).
    pub model: Option<String>,
    /// Reasoning effort level (`low`..`max`). Applies on any provider (both kinds carry
    /// an effort parameter). `None` omits it.
    pub effort: Option<String>,
    /// Output-token cap for clarify. `None` → [`CLARIFY_DEFAULT_MAX_OUTPUT_TOKENS`],
    /// which leaves headroom for reasoning + a substantive reply.
    pub max_output_tokens: Option<u32>,
}

/// Clarify's default output budget. Higher than distill's because a reasoning model
/// spends the budget on thinking first (verified on DeepSeek), and truncation there
/// is a hard failure, not a shorter answer.
pub const CLARIFY_DEFAULT_MAX_OUTPUT_TOKENS: u32 = 16_000;

/// `[privacy]` in `~/.omniproj/config.toml`. Every field is optional; a missing file
/// or section yields the secure default (deny-list + redaction on, consent off).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrivacyConfig {
    /// Override the built-in sensitive-path deny-list. `None` → built-in defaults.
    pub deny_globs: Option<Vec<String>>,
    /// Mask secret shapes in the outbound digest. `None` → true (on).
    pub redact: Option<bool>,
    /// User has acknowledged that the digest is sent to the LLM provider. `None`/
    /// false → the CLI prompts for consent before the first distill.
    pub send_consent: Option<bool>,
}

impl PrivacyConfig {
    /// Resolve into the `omniproj-core` policy the digest renderer consumes. `no_redact`
    /// (CLI `--no-redact`) forces redaction off regardless of config.
    pub fn to_policy(&self, no_redact: bool) -> omniproj_core::PrivacyPolicy {
        omniproj_core::PrivacyPolicy {
            deny_globs: self
                .deny_globs
                .clone()
                .unwrap_or_else(omniproj_core::default_deny_globs),
            redact: !no_redact && self.redact.unwrap_or(true),
        }
    }

    /// Whether the user has consented to sending the digest to the LLM provider.
    pub fn consented(&self) -> bool {
        self.send_consent.unwrap_or(false)
    }
}

/// Load the resolved privacy policy from config, applying a CLI `--no-redact`
/// override. Convenience for callers that only need the policy.
pub fn resolve_privacy(no_redact: bool) -> omniproj_core::PrivacyPolicy {
    load().privacy.to_policy(no_redact)
}

/// Whether a `provider/model` (or bare provider name) resolves to a LOCAL endpoint
/// (localhost/127.0.0.1) — i.e. the digest never leaves the machine, so no
/// send-consent notice is warranted (spec §5, W1-1 local-first path).
pub fn is_local_provider(model_or_provider: &str) -> bool {
    let cfg = load();
    let pname = model_or_provider
        .split_once('/')
        .map(|(p, _)| p)
        .unwrap_or(model_or_provider);
    match cfg.providers.get(pname) {
        Some(d) => resolve_base(pname, d)
            .map(|b| b.contains("localhost") || b.contains("127.0.0.1"))
            .unwrap_or(false),
        None => false,
    }
}

/// The 推理深度 knob (charter §5 原则6; spec §5.2). Shallow = the single-pass
/// distill (one LLM call — "LLM 只在必要时"). Deep = map-reduce older sessions +
/// structured extraction + completeness critic (several calls, opt-in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Depth {
    #[default]
    Shallow,
    Deep,
}

impl Depth {
    pub fn parse(s: &str) -> Option<Depth> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shallow" => Some(Depth::Shallow),
            "deep" => Some(Depth::Deep),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Depth::Shallow => "shallow",
            Depth::Deep => "deep",
        }
    }
}

/// Resolve reasoning depth. Precedence: explicit override (CLI) > config
/// `default_depth` > Shallow. Unknown strings fall back to Shallow (never
/// surprise the user with extra LLM spend).
pub fn resolve_depth(cli: Option<&str>) -> Depth {
    if let Some(s) = cli {
        return Depth::parse(s).unwrap_or_default();
    }
    load()
        .default_depth
        .as_deref()
        .and_then(Depth::parse)
        .unwrap_or_default()
}

fn def(kind: ProviderKind, base: &str, key_env: &str) -> ProviderDef {
    ProviderDef {
        kind,
        base_url: Some(base.to_string()),
        api_key_env: Some(key_env.to_string()),
    }
}

/// Predefined providers shipped with OmniProj. Override or extend in `~/.omniproj/config.toml`.
pub fn builtin() -> BTreeMap<String, ProviderDef> {
    use ProviderKind::{Anthropic, Openai};
    let mut m = BTreeMap::new();
    m.insert(
        "anthropic".into(),
        def(Anthropic, "https://api.anthropic.com", "ANTHROPIC_API_KEY"),
    );
    m.insert(
        "openai".into(),
        def(Openai, "https://api.openai.com/v1", "OPENAI_API_KEY"),
    );
    m.insert(
        "openrouter".into(),
        def(Openai, "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY"),
    );
    m.insert(
        "groq".into(),
        def(Openai, "https://api.groq.com/openai/v1", "GROQ_API_KEY"),
    );
    m.insert(
        "deepseek".into(),
        def(Openai, "https://api.deepseek.com/v1", "DEEPSEEK_API_KEY"),
    );
    m.insert(
        "together".into(),
        def(Openai, "https://api.together.xyz/v1", "TOGETHER_API_KEY"),
    );
    m.insert(
        "xai".into(),
        def(Openai, "https://api.x.ai/v1", "XAI_API_KEY"),
    );
    m.insert(
        "gemini".into(),
        def(
            Openai,
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "GEMINI_API_KEY",
        ),
    );
    m.insert("ollama".into(), {
        let mut d = def(Openai, "http://localhost:11434/v1", "OLLAMA_API_KEY");
        d.api_key_env = None; // local: no key required
        d
    });
    m
}

pub fn config_path() -> std::path::PathBuf {
    omniproj_core::omniproj_home().join("config.toml")
}

/// Load config: built-in catalog, with `~/.omniproj/config.toml` entries layered on top
/// (a file entry for an existing name overrides it; new names are added).
pub fn load() -> Config {
    let mut cfg = Config {
        providers: builtin(),
        ..Default::default()
    };
    if let Ok(text) = std::fs::read_to_string(config_path()) {
        if let Ok(file) = toml::from_str::<Config>(&text) {
            cfg.default_model = file.default_model;
            cfg.default_depth = file.default_depth;
            cfg.max_output_tokens = file.max_output_tokens;
            cfg.privacy = file.privacy;
            cfg.clarify = file.clarify;
            cfg.providers.extend(file.providers);
        }
    }
    cfg
}

fn resolve_base(name: &str, d: &ProviderDef) -> Result<String> {
    if let Some(b) = &d.base_url {
        return Ok(b.clone());
    }
    match d.kind {
        ProviderKind::Anthropic => Ok("https://api.anthropic.com".to_string()),
        ProviderKind::Openai => Err(anyhow!(
            "provider '{name}' (openai kind) needs a base_url in ~/.omniproj/config.toml"
        )),
    }
}

pub struct Resolved {
    pub provider: AnyProvider,
    pub provider_name: String,
    pub model: String,
}

/// The `provider/model` string that a distill would use when nothing overrides it.
/// Same precedence as [`resolve`] minus the explicit CLI override: `OMNIPROJ_MODEL` env,
/// then config `default_model`, then built-in `anthropic/claude-sonnet-4-6`. Read-only
/// — used by `omniproj doctor` to report the effective default without resolving a client.
pub fn default_model_string() -> String {
    std::env::var("OMNIPROJ_MODEL")
        .ok()
        .or_else(|| load().default_model.clone())
        .unwrap_or_else(|| "anthropic/claude-sonnet-4-6".to_string())
}

/// Resolve a `provider/model` string (or fall back to env/config/default) into a ready
/// provider. Precedence: explicit override, then `OMNIPROJ_MODEL` env, then config
/// `default_model`, then built-in `anthropic/claude-sonnet-4-6`. Uses the global token
/// cap and no reasoning effort — the historical distill behavior.
pub fn resolve(model_override: Option<&str>) -> Result<Resolved> {
    let cfg = load();
    let model_str = model_override
        .map(str::to_string)
        .or_else(|| std::env::var("OMNIPROJ_MODEL").ok())
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| "anthropic/claude-sonnet-4-6".to_string());
    let tuning = Tuning {
        effort: None,
        max_output_tokens: cfg.max_output_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS),
    };
    resolve_tuned(&cfg, &model_str, tuning)
}

/// Resolve the `[clarify]` provider (cockpit). Model = `[clarify] model` else
/// `default_model`; effort + a higher token budget come from `[clarify]`. Effort is
/// dropped with a warning if the resolved kind can't carry it.
pub fn resolve_clarify() -> Result<Resolved> {
    let cfg = load();
    let model_str = cfg
        .clarify
        .model
        .clone()
        .or_else(|| std::env::var("OMNIPROJ_MODEL").ok())
        .or_else(|| cfg.default_model.clone())
        .unwrap_or_else(|| "anthropic/claude-sonnet-4-6".to_string());
    let tuning = Tuning {
        effort: cfg.clarify.effort.clone(),
        max_output_tokens: cfg
            .clarify
            .max_output_tokens
            .unwrap_or(CLARIFY_DEFAULT_MAX_OUTPUT_TOKENS),
    };
    resolve_tuned(&cfg, &model_str, tuning)
}

/// Core resolver: `provider/model` + explicit [`Tuning`] → a ready provider. Warns
/// (once, to stderr) and drops effort when the resolved kind doesn't support it, so a
/// misconfigured effort degrades gracefully rather than erroring or silently vanishing.
fn resolve_tuned(cfg: &Config, model_str: &str, tuning: Tuning) -> Result<Resolved> {
    let (pname, model) = model_str.split_once('/').ok_or_else(|| {
        anyhow!(
            "model must be 'provider/model' (e.g. anthropic/claude-sonnet-4-6), got '{model_str}'"
        )
    })?;

    let d = cfg.providers.get(pname).ok_or_else(|| {
        let known: Vec<&str> = cfg.providers.keys().map(String::as_str).collect();
        anyhow!("unknown provider '{pname}'. known: {}", known.join(", "))
    })?;

    let base = resolve_base(pname, d)?;
    let api_key = match &d.api_key_env {
        Some(env) => std::env::var(env).unwrap_or_default(),
        None => String::new(),
    };
    let is_local = base.contains("localhost") || base.contains("127.0.0.1");
    if d.api_key_env.is_some() && api_key.is_empty() && !is_local {
        let env = d.api_key_env.as_deref().unwrap_or("?");
        return Err(anyhow!(
            "provider '{pname}' needs an API key.\n\
             \x20 · Set the {env} environment variable (get a key from {pname}'s dashboard),\n\
             \x20   e.g. `export {env}=...`.\n\
             \x20 · Run `omniproj providers` to see every configured provider and its key status,\n\
             \x20   `omniproj doctor` to diagnose your setup.\n\
             \x20 · Prefer local, nothing-leaves-the-machine? Use Ollama (no key required): set\n\
             \x20   `default_model = \"ollama/<model>\"` in ~/.omniproj/config.toml (`omniproj init`)."
        ))
        .with_context(|| format!("model = {model_str}"));
    }

    // Both kinds carry a reasoning-effort parameter (Anthropic output_config.effort,
    // OpenAI Responses reasoning.effort), so no effort is ever silently dropped.
    let provider = match d.kind {
        ProviderKind::Anthropic => AnyProvider::Anthropic(AnthropicProvider::new(
            base,
            api_key,
            model.to_string(),
            tuning,
        )),
        ProviderKind::Openai => AnyProvider::OpenAi(OpenAiProvider::new(
            base,
            api_key,
            model.to_string(),
            tuning,
        )),
    };
    Ok(Resolved {
        provider,
        provider_name: pname.to_string(),
        model: model.to_string(),
    })
}

/// Listing for `omniproj providers` — name, kind, base, key env, whether the key is present.
pub struct ProviderStatus {
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub key_present: bool,
}

pub fn list() -> Vec<ProviderStatus> {
    let cfg = load();
    cfg.providers
        .into_iter()
        .map(|(name, d)| {
            let base_url = d.base_url.clone().unwrap_or_else(|| match d.kind {
                ProviderKind::Anthropic => "https://api.anthropic.com".into(),
                ProviderKind::Openai => "(needs base_url)".into(),
            });
            let key_present = match &d.api_key_env {
                Some(env) => std::env::var(env).map(|v| !v.is_empty()).unwrap_or(false),
                None => true, // local / no key needed
            };
            ProviderStatus {
                name,
                kind: d.kind,
                base_url,
                api_key_env: d.api_key_env,
                key_present,
            }
        })
        .collect()
}

/// A commented config template for `omniproj init`.
pub const CONFIG_TEMPLATE: &str = r#"# OmniProj config — ~/.omniproj/config.toml
#
# Model is chosen as "provider/model". Predefined providers ship built-in
# (anthropic, openai, openrouter, groq, deepseek, together, xai, gemini, ollama);
# you only need entries here to OVERRIDE one or ADD a custom endpoint.

# Default model when a project doesn't override it:
default_model = "anthropic/claude-sonnet-4-6"

# Reasoning depth (spec §5.2 推理深度 knob): "shallow" (default, single LLM pass)
# or "deep" (map-reduce older sessions + structured extraction + completeness
# critic — several LLM calls per distill, noticeably better on long histories).
# default_depth = "shallow"

# Output-token cap for distill calls (default 4096). This is a COMBINED budget of
# reasoning + visible text on reasoning models — raise it if you point distill at a
# reasoning model and see truncated output.
# max_output_tokens = 4096

# --- examples (uncomment / edit) ---

# Override a predefined provider's key env or base:
# [providers.anthropic]
# kind = "anthropic"
# api_key_env = "ANTHROPIC_API_KEY"

# A custom OpenAI-Responses endpoint (kind = "openai" → POST <base_url>/responses):
# [providers.myllm]
# kind = "openai"
# base_url = "https://my-endpoint.example/v1"
# api_key_env = "MYLLM_API_KEY"

# Local Ollama (no key needed) is predefined; use e.g. default_model = "ollama/llama3.1"

# --- clarify (cockpit): the per-note-item discussion feature ---
# `omniproj clarify` challenges a not-yet-clear next action (标记+理由, never 建议).
# It gets its own model/effort/budget because discussion is multi-round and wants
# reasoning headroom. Reasoning effort applies on any provider (Anthropic +
# OpenAI Responses).
# [clarify]
# model = "deepseek/deepseek-chat"   # falls back to default_model
# effort = "high"                    # low | medium | high | xhigh | max
# max_output_tokens = 16000          # combined reasoning + reply budget

# --- privacy (spec W1-1) ---
# The distill digest (git + session text) is sent to your configured LLM provider.
# For the strongest privacy, point default_model at a LOCAL endpoint (Ollama) so
# nothing leaves the machine. OmniProj scrubs the OUTBOUND digest by default:
#
# [privacy]
# # Acknowledge that the digest is sent to the provider (else the CLI asks once).
# send_consent = false
# # Mask secret shapes (sk-…, AKIA…, Bearer …, KEY=value) in the digest. Default on.
# # redact = true
# # Override the built-in sensitive-path deny-list (paths dropped from the digest):
# # deny_globs = [".env*", "*.key", "*.pem", "id_rsa*", "secrets/", "credentials*"]
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_parses_known_values_and_rejects_garbage() {
        assert_eq!(Depth::parse("shallow"), Some(Depth::Shallow));
        assert_eq!(Depth::parse("Deep"), Some(Depth::Deep));
        assert_eq!(Depth::parse(" DEEP "), Some(Depth::Deep));
        assert_eq!(Depth::parse("ultra"), None);
    }

    #[test]
    fn cli_override_wins_and_garbage_falls_back_shallow() {
        // CLI value is authoritative when valid; an invalid CLI value must fall
        // back to Shallow (never surprise-spend on extra LLM calls).
        assert_eq!(resolve_depth(Some("deep")), Depth::Deep);
        assert_eq!(resolve_depth(Some("bogus")), Depth::Shallow);
    }

    #[test]
    fn config_toml_with_depth_deserializes() {
        let cfg: Config = toml::from_str("default_depth = \"deep\"").unwrap();
        assert_eq!(cfg.default_depth.as_deref(), Some("deep"));
    }

    #[test]
    fn privacy_defaults_are_secure_when_absent() {
        // No [privacy] section → redaction on, deny-list populated, consent off.
        let cfg: Config = toml::from_str("").unwrap();
        let policy = cfg.privacy.to_policy(false);
        assert!(policy.redact);
        assert!(!policy.deny_globs.is_empty());
        assert!(!cfg.privacy.consented());
    }

    #[test]
    fn openai_kind_parses() {
        let cfg: Config =
            toml::from_str("[providers.myr]\nkind = \"openai\"\nbase_url = \"https://x/v1\"")
                .unwrap();
        assert_eq!(cfg.providers["myr"].kind, ProviderKind::Openai);
    }

    #[test]
    fn clarify_section_parses_and_defaults_are_empty() {
        let cfg: Config = toml::from_str(
            "[clarify]\nmodel = \"deepseek/deepseek-chat\"\neffort = \"high\"\nmax_output_tokens = 20000",
        )
        .unwrap();
        assert_eq!(cfg.clarify.model.as_deref(), Some("deepseek/deepseek-chat"));
        assert_eq!(cfg.clarify.effort.as_deref(), Some("high"));
        assert_eq!(cfg.clarify.max_output_tokens, Some(20000));
        // Absent section → all None.
        let empty: Config = toml::from_str("").unwrap();
        assert!(empty.clarify.model.is_none());
        assert!(empty.clarify.effort.is_none());
        assert!(empty.clarify.max_output_tokens.is_none());
    }

    #[test]
    fn openai_builtins_are_responses_kind() {
        // Every OpenAI-family builtin is the Responses kind (OmniProj is all-in on Responses).
        let b = builtin();
        for name in ["openai", "deepseek", "openrouter", "ollama"] {
            assert_eq!(b[name].kind, ProviderKind::Openai, "{name}");
        }
        assert_eq!(b["anthropic"].kind, ProviderKind::Anthropic);
        assert!(b["ollama"].api_key_env.is_none()); // local → no key
    }

    #[test]
    fn privacy_config_parses_and_no_redact_overrides() {
        let cfg: Config = toml::from_str(
            "[privacy]\nredact = true\nsend_consent = true\ndeny_globs = [\"*.secret\"]",
        )
        .unwrap();
        assert!(cfg.privacy.consented());
        let policy = cfg.privacy.to_policy(false);
        assert!(policy.redact);
        assert_eq!(policy.deny_globs, vec!["*.secret".to_string()]);
        // CLI --no-redact forces redaction off regardless of config.
        assert!(!cfg.privacy.to_policy(true).redact);
    }
}
