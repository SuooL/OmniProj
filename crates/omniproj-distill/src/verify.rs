//! Deterministic verify gate (spec §5.2). No LLM, zero cost. Scans distilled
//! output for commit-hash-shaped tokens and annotates any the FactSheet can't
//! vouch for — the code-level backstop for "never fabricate a commit", which a
//! prompt rule alone failed to enforce (DeepSeek invented `b87d023` despite it).
//!
//! Grounds commit hashes (the proven failure mode) AND file paths (review P1#5,
//! spec §5.2 "hash/数字/路径"). Numbers remain the future slot.

use omniproj_core::FactSheet;

/// Appended inline after any hash the FactSheet can't vouch for.
pub const UNVERIFIED_MARK: &str = "(⚠未核实)";

#[derive(Debug, Default)]
pub struct VerifyReport {
    /// Distinct hash-shaped tokens not present in the FactSheet whitelist.
    pub flagged: Vec<String>,
    /// Distinct path-shaped tokens not present in the FactSheet path whitelist.
    pub flagged_paths: Vec<String>,
}

impl VerifyReport {
    pub fn is_clean(&self) -> bool {
        self.flagged.is_empty() && self.flagged_paths.is_empty()
    }
}

/// A token is a commit-hash candidate if it is 7..=40 hex chars AND contains at
/// least one a–f letter. The letter requirement keeps plain decimals (years,
/// counts, line numbers like `1234567`) from being mistaken for hashes; real
/// short hashes almost always contain a hex letter. Rare hexish English words
/// (e.g. "deadbeef") may be annotated — acceptable since we annotate, not delete.
fn is_hash_candidate(tok: &str) -> bool {
    let len = tok.len();
    if !(7..=40).contains(&len) {
        return false;
    }
    let mut has_alpha = false;
    for c in tok.chars() {
        match c {
            '0'..='9' => {}
            'a'..='f' | 'A'..='F' => has_alpha = true,
            _ => return false,
        }
    }
    has_alpha
}

fn flush_run(run: &mut String, out: &mut String, report: &mut VerifyReport, facts: &FactSheet) {
    if run.is_empty() {
        return;
    }
    out.push_str(run);
    if is_hash_candidate(run) && !facts.knows_hash(run) {
        out.push_str(UNVERIFIED_MARK);
        if !report.flagged.iter().any(|f| f == run.as_str()) {
            report.flagged.push(run.clone());
        }
    }
    run.clear();
}

/// Walk `text` char-by-char (UTF-8 safe), find maximal hex runs, and annotate any
/// that look like a commit hash but aren't in the FactSheet. Returns the annotated
/// text plus a report of what was flagged.
pub fn verify_hashes(text: &str, facts: &FactSheet) -> (String, VerifyReport) {
    let mut report = VerifyReport::default();
    let mut out = String::with_capacity(text.len() + 16);
    let mut run = String::new();
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            run.push(c);
        } else {
            flush_run(&mut run, &mut out, &mut report, facts);
            out.push(c);
        }
    }
    flush_run(&mut run, &mut out, &mut report, facts);
    (out, report)
}

/// Is this token shaped like a repo file path worth verifying? Conservative on
/// purpose (we'd rather miss than flag URLs/prose): needs a `/`, a file extension
/// in the last segment, path-safe charset, and a non-domain-looking first segment.
fn is_path_candidate(tok: &str) -> bool {
    // No leading '/': absolute paths aren't in the repo-relative whitelist's scope,
    // and "//host/x.png" URL remnants (after the charset split ate "https:") start
    // with slashes too. Conservative — we'd rather miss than flag.
    if !tok.contains('/') || tok.starts_with('/') || tok.starts_with("http") {
        return false;
    }
    if !tok
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        return false;
    }
    // last segment must look like a file: name.ext with a short alpha-ish extension
    let last = tok.rsplit('/').next().unwrap_or("");
    let Some((name, ext)) = last.rsplit_once('.') else {
        return false;
    };
    if name.is_empty()
        || ext.is_empty()
        || ext.len() > 5
        || !ext.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return false;
    }
    // first segment looking like a bare domain (contains '.', doesn't start with '.')
    // → likely a URL fragment, skip
    let first = tok.split('/').next().unwrap_or("");
    !first.contains('.') || first.starts_with('.')
}

/// Annotate path-shaped tokens the FactSheet's path whitelist can't vouch for
/// (spec §5.2 "路径"). Only runs when a whitelist exists — for no-git projects we
/// can't enumerate real files, and flagging everything would be noise.
pub fn verify_paths(text: &str, facts: &FactSheet) -> (String, VerifyReport) {
    let mut report = VerifyReport::default();
    if !facts.has_path_whitelist() {
        return (text.to_string(), report);
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut run = String::new();
    let is_tok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-');
    let flush = |run: &mut String, out: &mut String, report: &mut VerifyReport| {
        if run.is_empty() {
            return;
        }
        // trim trailing sentence punctuation that the charset admits
        let trimmed = run.trim_end_matches('.');
        out.push_str(run);
        if is_path_candidate(trimmed) && !facts.knows_path(trimmed) {
            out.push_str(UNVERIFIED_MARK);
            if !report.flagged_paths.iter().any(|f| f == trimmed) {
                report.flagged_paths.push(trimmed.to_string());
            }
        }
        run.clear();
    };
    for c in text.chars() {
        if is_tok_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out, &mut report);
            out.push(c);
        }
    }
    flush(&mut run, &mut out, &mut report);
    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniproj_core::GitFacts;

    fn facts(hashes: &[&str]) -> FactSheet {
        FactSheet {
            git: Some(GitFacts {
                branch: "main".into(),
                head_short: "0d906c3".into(),
                commit_hashes: hashes.iter().map(|s| s.to_string()).collect(),
                file_paths: vec!["src/main.rs".into(), "crates/core/src/lib.rs".into()],
            }),
        }
    }

    #[test]
    fn flags_fabricated_path_leaves_real() {
        let fs = facts(&[]);
        let (out, rep) = verify_paths("改了 src/main.rs 和 src/ghost.rs 两个文件", &fs);
        assert!(out.contains(&format!("src/ghost.rs{UNVERIFIED_MARK}")));
        assert!(!out.contains(&format!("src/main.rs{UNVERIFIED_MARK}")));
        assert_eq!(rep.flagged_paths, vec!["src/ghost.rs".to_string()]);
    }

    #[test]
    fn path_gate_skips_urls_prose_and_trailing_punctuation() {
        let fs = facts(&[]);
        let (out, rep) = verify_paths(
            "见 https://example.com/a.png 与 example.com/b.rs;结尾句号的 src/main.rs.",
            &fs,
        );
        assert!(rep.flagged_paths.is_empty(), "got: {:?}", rep.flagged_paths);
        assert!(!out.contains(UNVERIFIED_MARK));
    }

    #[test]
    fn path_gate_noop_without_whitelist() {
        let fs = FactSheet::default();
        let (out, rep) = verify_paths("提到 src/anything.rs 也不动", &fs);
        assert!(rep.is_clean());
        assert!(!out.contains(UNVERIFIED_MARK));
    }

    #[test]
    fn flags_fabricated_leaves_real() {
        let fs = facts(&["0d906c3f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"]);
        let (out, rep) = verify_hashes("done in 0d906c3 but b87d023 is fake", &fs);
        assert!(out.contains(&format!("b87d023{UNVERIFIED_MARK}")));
        assert!(out.contains("0d906c3 ")); // real hash untouched
        assert!(!out.contains(&format!("0d906c3{UNVERIFIED_MARK}")));
        assert_eq!(rep.flagged, vec!["b87d023".to_string()]);
    }

    #[test]
    fn ignores_plain_numbers_and_short_tokens() {
        let fs = facts(&["0d906c3f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"]);
        let (out, rep) = verify_hashes("ran 1234567 tests in 2026, abc ok", &fs);
        assert!(rep.is_clean()); // 1234567 = no hex letter; 2026/abc too short
        assert_eq!(out, "ran 1234567 tests in 2026, abc ok");
    }

    #[test]
    fn utf8_text_preserved() {
        let fs = facts(&["0d906c3f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"]);
        let (out, _) = verify_hashes("中文说明:提交 deadbeef 完成。", &fs);
        assert!(out.starts_with("中文说明:提交 deadbeef"));
        assert!(out.contains(&format!("deadbeef{UNVERIFIED_MARK}")));
    }
}
