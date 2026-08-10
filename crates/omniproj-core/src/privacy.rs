//! omniproj-core::privacy — the deny-list + secret-redaction primitives (spec §5, W1-1).
//!
//! Trust boundary: persistent state in `~/.omniproj` is local and faithful, but the
//! distill **digest is sent to a third-party LLM provider**. These pure functions
//! scrub the outbound digest: deny-listed paths are dropped and known secret shapes
//! (`sk-…`, `AKIA…`, `Bearer …`, provider tokens, `KEY=value` assignments) are
//! masked before any text leaves the machine. Deterministic, no IO — trust is
//! enforced by code, not by asking the model to behave (charter §原则8, §4d).
//!
//! Redaction is intentionally CONSERVATIVE about high-entropy strings: git commit
//! SHAs are legitimately in the digest AND are the verify-gate whitelist, so a
//! blanket "long hex" rule would nuke them. We match secret *shapes* and
//! `key = value` *assignments*, never bare hex.

use std::sync::OnceLock;

use regex::Regex;

/// Outbound-digest privacy policy. `Default` is **secure**: default deny-list +
/// redaction on, so every caller that builds a digest with `Default::default()`
/// is protected without opting in.
#[derive(Debug, Clone)]
pub struct PrivacyPolicy {
    /// Path globs whose matches are dropped from the digest (never sent).
    pub deny_globs: Vec<String>,
    /// Mask known secret shapes in session/git text before sending.
    pub redact: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            deny_globs: default_deny_globs(),
            redact: true,
        }
    }
}

/// Built-in sensitive-path globs (spec W1-1). Users can override via `[privacy]
/// deny_globs` in `~/.omniproj/config.toml`.
pub fn default_deny_globs() -> Vec<String> {
    [
        ".env*",
        "*.key",
        "*.pem",
        "*.p12",
        "*.pfx",
        "id_rsa*",
        "id_ed25519*",
        "id_dsa*",
        "id_ecdsa*",
        "*.keystore",
        "secrets/",
        ".secrets/",
        "credentials*",
        ".aws/",
        ".ssh/",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl PrivacyPolicy {
    /// A policy with everything off — for callers that must see raw substrate
    /// (never the outbound digest). Rarely correct; prefer `Default`.
    pub fn permissive() -> Self {
        Self {
            deny_globs: Vec::new(),
            redact: false,
        }
    }

    /// True if `path` (repo-relative, `/`-separated) matches any deny glob.
    pub fn path_denied(&self, path: &str) -> bool {
        let path = path.trim();
        if path.is_empty() {
            return false;
        }
        self.deny_globs.iter().any(|g| glob_match(g, path))
    }

    /// Mask known secret shapes in `text`. Returns the scrubbed text and the number
    /// of masked spans. A no-op (returns `(text, 0)`) when `redact` is off.
    pub fn redact_text(&self, text: &str) -> (String, usize) {
        if !self.redact {
            return (text.to_string(), 0);
        }
        redact_secrets(text)
    }
}

/// Match `path` against a single glob. Supported forms (deliberately small — this
/// is a deny-list, not a shell):
/// - `secrets/` — trailing slash → matches any path with that directory *segment*.
/// - `*.key` / `id_rsa*` / `*foo*` — one leading and/or trailing `*` on the
///   **basename** (and, for a bare `name`, an exact basename match).
fn glob_match(glob: &str, path: &str) -> bool {
    // Directory-segment glob: "secrets/" matches a/secrets/b, secrets/x, etc.
    if let Some(seg) = glob.strip_suffix('/') {
        let seg = seg.trim_start_matches("./");
        return path.split('/').any(|c| c == seg);
    }
    let base = path.rsplit('/').next().unwrap_or(path);
    let star_lead = glob.starts_with('*');
    let star_tail = glob.ends_with('*');
    let core = glob.trim_matches('*');
    match (star_lead, star_tail) {
        (true, true) => base.contains(core),
        (true, false) => base.ends_with(core),
        (false, true) => base.starts_with(core),
        (false, false) => base == core,
    }
}

/// Compiled secret patterns, each paired with a short type label used in the mask
/// marker (so a reader still sees *what kind* of secret was there).
fn patterns() -> &'static [(Regex, &'static str)] {
    static PATS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATS.get_or_init(|| {
        let raw: &[(&str, &str)] = &[
            // OpenAI / DeepSeek / etc.: sk-..., sk-proj-...
            (r"sk-[A-Za-z0-9_-]{12,}", "openai-key"),
            // AWS access key id.
            (r"AKIA[0-9A-Z]{16}", "aws-akid"),
            // GitHub tokens: ghp_ gho_ ghu_ ghs_ ghr_ + fine-grained github_pat_.
            (r"gh[opusr]_[A-Za-z0-9]{16,}", "github-token"),
            (r"github_pat_[A-Za-z0-9_]{20,}", "github-token"),
            // Slack tokens: xoxb- xoxp- xoxa- xoxr-.
            (r"xox[baprs]-[A-Za-z0-9-]{10,}", "slack-token"),
            // Google API key.
            (r"AIza[0-9A-Za-z_-]{20,}", "google-key"),
            // Private key PEM header.
            (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "pem-private-key"),
            // Authorization: Bearer <token>  /  bare "Bearer <token>".
            (r"(?i)bearer\s+[A-Za-z0-9._~+/-]{12,}=*", "bearer-token"),
            // key = "value" / password: value / token=value  (value >= 8 chars).
            (
                r#"(?i)(api[_-]?key|secret|token|password|passwd|access[_-]?key)"?\s*[:=]\s*"?[A-Za-z0-9._~+/-]{8,}"?"#,
                "credential",
            ),
        ];
        raw.iter()
            .map(|(re, label)| (Regex::new(re).expect("static regex compiles"), *label))
            .collect()
    })
}

/// Mask every known secret shape in `text`. Returns `(scrubbed, count)`.
/// Overlapping matches are handled by masking patterns in priority order and
/// re-scanning the already-masked string, so a value can't be double-counted.
pub fn redact_secrets(text: &str) -> (String, usize) {
    let mut out = text.to_string();
    let mut count = 0usize;
    for (re, label) in patterns() {
        // Count first (find_iter over the current text), then replace all.
        let n = re.find_iter(&out).count();
        if n == 0 {
            continue;
        }
        count += n;
        let marker = format!("«redacted:{label}»");
        out = re.replace_all(&out, marker.as_str()).into_owned();
    }
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_secure() {
        let p = PrivacyPolicy::default();
        assert!(p.redact, "redaction on by default (--no-redact opts out)");
        assert!(!p.deny_globs.is_empty());
    }

    #[test]
    fn deny_globs_match_sensitive_paths() {
        let p = PrivacyPolicy::default();
        for hit in [
            ".env",
            ".env.local",
            ".env.production",
            "config/app.key",
            "certs/server.pem",
            "keys/bundle.p12",
            "id_rsa",
            "id_rsa.pub",
            "id_ed25519",
            "secrets/db.txt",
            "infra/secrets/prod.yaml",
            "credentials",
            "credentials.json",
            ".aws/config",
            "home/.ssh/known_hosts",
        ] {
            assert!(p.path_denied(hit), "should deny: {hit}");
        }
    }

    #[test]
    fn deny_globs_allow_normal_paths() {
        let p = PrivacyPolicy::default();
        for ok in [
            "src/main.rs",
            "README.md",
            "environment.rs",      // not .env*
            "keyboard.rs",         // not *.key
            "docs/credentials.md", // credentials* matches basename credentials.md -> DENIED, keep out
        ] {
            // last one intentionally excluded below
            if ok.ends_with("credentials.md") {
                continue;
            }
            assert!(!p.path_denied(ok), "should allow: {ok}");
        }
        // credentials* is a prefix glob: it DOES match credentials.md by design.
        assert!(p.path_denied("docs/credentials.md"));
    }

    #[test]
    fn redacts_named_secret_shapes() {
        let cases = [
            "here is my key sk-abcdef0123456789ABCDEF",
            "aws AKIAIOSFODNN7EXAMPLE in use",
            "token ghp_abcdefghijklmnop0123456789",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig",
            "AIzaSyA1234567890abcdefghijklmnopqrstuv",
            r#"API_KEY = "s3cr3tvalue123""#,
            "password: hunter2hunter2",
        ];
        for c in cases {
            let (out, n) = redact_secrets(c);
            assert!(n >= 1, "expected a redaction in: {c}");
            assert!(out.contains("«redacted:"), "marker present: {out}");
        }
    }

    #[test]
    fn does_not_redact_git_sha_or_prose() {
        // Commit SHAs are legitimately in the digest and are the verify whitelist —
        // must survive redaction. Plain prose must be untouched.
        let sha = "fixed in commit 6e5df67 and 4a0cf9e0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6";
        let (out, n) = redact_secrets(sha);
        assert_eq!(n, 0, "no redaction over commit hashes: {out}");
        assert_eq!(out, sha);

        let prose = "We refactored render_digest to keep the newest messages.";
        let (out, n) = redact_secrets(prose);
        assert_eq!(n, 0);
        assert_eq!(out, prose);
    }

    #[test]
    fn permissive_policy_is_a_noop() {
        let p = PrivacyPolicy::permissive();
        assert!(!p.path_denied(".env"));
        let (out, n) = p.redact_text("sk-abcdefghijklmnop0123");
        assert_eq!(n, 0);
        assert_eq!(out, "sk-abcdefghijklmnop0123");
    }
}
