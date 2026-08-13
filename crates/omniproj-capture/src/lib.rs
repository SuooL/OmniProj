//! omniproj-capture — Layer 1 (spec §4.2 / §5).
//! Passively gather git activity + Claude/Codex sessions for a project directory,
//! normalize to `Session`, and render a compact substrate digest for distillation.
//! No LLM here.

pub mod claude;
pub mod codex;
pub mod git;

use std::path::Path;

use chrono::{DateTime, Utc};
use omniproj_core::{
    project_hash, FactSheet, GitFacts, Message, PrivacyPolicy, ProjectId, ProjectSource, Role,
    Session,
};

pub use git::GitInfo;

/// Where agent session transcripts live on this machine (existing dirs only).
/// The daemon watches these so conversation-only activity — work that never touches
/// the worktree — still triggers a refresh (benchmark review P0#2).
pub fn session_roots() -> Vec<std::path::PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    [
        home.join(".claude").join("projects"),
        home.join(".codex").join("sessions"),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect()
}

/// The project cwd a session transcript belongs to, regardless of source format.
/// Tries the Claude shape (per-line `cwd`) then the Codex shape (first-line
/// `session_meta.payload.cwd`). `None` for files that are neither.
pub fn session_owner_cwd(path: &Path) -> Option<String> {
    claude::first_cwd(path).or_else(|| codex::meta_cwd(path).map(|(cwd, _, _)| cwd))
}

pub struct Substrate {
    /// Permanent identity assigned at registration. This must never be derived from
    /// a source location: sources can move while the project and its derived state
    /// remain the same.
    pub project_id: ProjectId,
    /// The source location observed during this capture.
    pub location: String,
    pub name: String,
    pub git: Option<GitInfo>,
    /// All matched sessions, ascending by mtime.
    pub sessions: Vec<Session>,
    pub claude_n: usize,
    pub codex_n: usize,
}

impl Substrate {
    /// Build the deterministic ground-truth FactSheet (spec §4.7 / §5.2) the
    /// distiller is grounded on and the verify gate checks output against. No LLM.
    pub fn factsheet(&self) -> FactSheet {
        FactSheet {
            git: self.git.as_ref().map(|g| GitFacts {
                branch: g.branch.clone(),
                head_short: g.head.clone(),
                commit_hashes: g.commit_hashes.clone(),
                file_paths: g.file_paths.clone(),
            }),
        }
    }
}

pub struct DigestOpts {
    pub last_k: usize,
    pub msg_trunc: usize,
    pub cap_chars: usize,
    /// Outbound-privacy policy (spec §5, W1-1): deny-listed paths are dropped and
    /// secret shapes masked before the digest leaves the machine. `Default` is
    /// secure (default deny-list + redaction on).
    pub privacy: PrivacyPolicy,
}

impl Default for DigestOpts {
    fn default() -> Self {
        Self {
            last_k: 4,
            msg_trunc: 1400,
            cap_chars: 70_000,
            privacy: PrivacyPolicy::default(),
        }
    }
}

/// Capture the substrate for one registered project source (git optional).
pub fn capture_source(project_id: &ProjectId, source: &ProjectSource) -> anyhow::Result<Substrate> {
    if source.project_id != *project_id {
        anyhow::bail!(
            "source {} belongs to project {}, not {project_id}",
            source.id,
            source.project_id
        );
    }
    let location = std::fs::canonicalize(&source.location)
        .unwrap_or_else(|_| Path::new(&source.location).to_path_buf())
        .to_string_lossy()
        .to_string();
    let name = Path::new(&location)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let git = git::collect(Path::new(&location));

    let mut sessions = claude::sessions_for_cwd(&location);
    let claude_n = sessions.len();
    let codex = codex::sessions_for_cwd(&location);
    let codex_n = codex.len();
    sessions.extend(codex);
    sessions.sort_by(|a, b| {
        a.mtime
            .partial_cmp(&b.mtime)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Substrate {
        project_id: project_id.clone(),
        location,
        name,
        git,
        sessions,
        claude_n,
        codex_n,
    })
}

/// Capture by a path-derived legacy identity.
///
/// New callers must resolve a registered [`ProjectSource`] and call
/// [`capture_source`], so moving a source cannot create a second project cache.
#[deprecated(note = "resolve a registered ProjectSource and use capture_source")]
pub fn capture(dir: &Path) -> anyhow::Result<Substrate> {
    let location = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let location_text = location.to_string_lossy().into_owned();
    let project_id = ProjectId::parse(project_hash(&location_text))
        .expect("path-derived legacy project hash must be a valid ProjectId");
    let source = ProjectSource {
        id: omniproj_core::ProjectSourceId::parse("legacy-source")
            .expect("static legacy source id must be valid"),
        project_id: project_id.clone(),
        kind: omniproj_core::ProjectSourceKind::GitRepo,
        location: location_text,
        is_primary: true,
        status: omniproj_core::ProjectSourceStatus::Available,
        created_at: "1970-01-01T00:00:00Z".into(),
        last_observed_at: None,
        last_successful_refresh_at: None,
        last_error_category: None,
        revision: 0,
    };
    capture_source(&project_id, &source)
}

fn fmt_mtime(secs: f64) -> String {
    DateTime::<Utc>::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "?".into())
}

/// Heuristic: injected skill/AGENTS preludes appear as "user" messages but aren't
/// conversation (spec §4.2 capture rules). Conservative — only the obvious markers.
fn is_injected(text: &str) -> bool {
    const MARKERS: [&str; 5] = [
        "Base directory for this skill:",
        "# AGENTS.md instructions",
        "<INSTRUCTIONS>",
        "Caveat: The messages below",
        "<system-reminder>",
    ];
    let head = text.trim_start();
    MARKERS.iter().any(|m| head.starts_with(m)) || head.starts_with("# 全局 AGENTS")
}

const ELISION: &str = "…[较旧内容已省略,仅保留最近]\n";

fn session_header(s: &Session) -> String {
    format!(
        "\n### session {} [{}] ({} msgs) {}",
        fmt_mtime(s.mtime),
        s.source.as_str(),
        s.user_assistant_count(),
        s.id
    )
}

/// One rendered `U: …` / `A: …` line for a message; `None` if it should be skipped
/// (injected noise, or a non-user/assistant role). Secret shapes are masked per
/// `policy` before the line can enter the outbound digest (spec §5, W1-1).
fn render_msg_line(m: &Message, msg_trunc: usize, policy: &PrivacyPolicy) -> Option<String> {
    if is_injected(&m.text) {
        return None;
    }
    let tag = match m.role {
        Role::User => "U",
        Role::Assistant => "A",
        _ => return None,
    };
    let (text, _) = policy.redact_text(&m.text);
    let text = if text.chars().count() > msg_trunc {
        let t: String = text.chars().take(msg_trunc).collect();
        format!("{t} …[truncated]")
    } else {
        text
    };
    Some(format!("{tag}: {text}"))
}

/// Drop deny-listed paths from a git text block (porcelain status or numstat
/// diffstat). `path_of` extracts the repo-relative path from one line; a denied
/// line is replaced by a marker so the count of hidden files is still visible
/// (spec §5, W1-1). Non-matching lines pass through unchanged.
fn filter_paths_block(
    block: &str,
    policy: &PrivacyPolicy,
    path_of: impl Fn(&str) -> &str,
) -> String {
    block
        .lines()
        .map(|line| {
            let p = path_of(line);
            if !p.is_empty() && policy.path_denied(p) {
                "  «redacted: sensitive path (deny-list)»".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Path from a porcelain status line ("XY <path>", renames "old -> new").
fn porcelain_path(line: &str) -> &str {
    if line.len() > 3 {
        let p = &line[3..];
        p.split(" -> ").last().unwrap_or(p).trim().trim_matches('"')
    } else {
        ""
    }
}

/// Path from a `--numstat` line ("added\tdeleted\tpath").
fn numstat_path(line: &str) -> &str {
    line.rsplit('\t').next().unwrap_or("").trim()
}

/// Render ONE session as standalone text (header + message lines, same filtering as
/// the digest). Input to the deep pipeline's map pass (spec §5.2): older sessions
/// outside the digest window get compressed individually instead of discarded.
/// `policy` masks secrets before the text can leave the machine (W1-1).
pub fn render_session_text(
    s: &Session,
    msg_trunc: usize,
    cap_chars: usize,
    policy: &PrivacyPolicy,
) -> String {
    let mut out = session_header(s);
    out.push('\n');
    for m in &s.messages {
        if let Some(l) = render_msg_line(m, msg_trunc, policy) {
            out.push_str(&l);
            out.push('\n');
        }
    }
    // Keep the TAIL (newest) when over budget — same recency rule as the digest.
    let total = out.chars().count();
    if total > cap_chars {
        let tail: String = out.chars().skip(total - cap_chars).collect();
        return format!("{ELISION}{tail}");
    }
    out
}

/// The sessions OUTSIDE the digest's recency window (everything except the
/// most-recent `last_k`), rendered for the map pass, **newest-first** so a caller
/// taking the first N compresses the most relevant ones.
pub fn older_session_texts(
    sub: &Substrate,
    last_k: usize,
    msg_trunc: usize,
    cap_chars: usize,
    policy: &PrivacyPolicy,
) -> Vec<String> {
    let n = sub.sessions.len().saturating_sub(last_k);
    sub.sessions[..n]
        .iter()
        .rev() // newest-first among the older ones
        .map(|s| render_session_text(s, msg_trunc, cap_chars, policy))
        .collect()
}

/// Render a compact substrate digest. Recency is enforced at **message** granularity:
/// the most-recent `last_k` sessions are flattened into chronological lines, and when
/// over the char cap we drop from the FRONT (oldest) and keep the TAIL (newest). This
/// fixes the case where a single large session had its newest — most re-entry-relevant
/// — messages chopped by a tail-truncating overall cap (spec §4.2).
pub fn render_digest(sub: &Substrate, opts: &DigestOpts) -> String {
    let mut head = String::new();
    head.push_str(&format!("# SUBSTRATE DIGEST — {}\n", sub.name));
    head.push_str(&format!("path: {}\n", sub.location));
    let total = sub.sessions.len();
    let shown = opts.last_k.min(total);
    head.push_str(&format!(
        "sessions total: {total} (claude {}, codex {}); showing most-recent {shown}\n\n",
        sub.claude_n, sub.codex_n
    ));

    head.push_str("## GIT\n");
    match &sub.git {
        Some(g) => {
            head.push_str(&format!("branch: {}\nHEAD: {}\n", g.branch, g.head));
            head.push_str("status (porcelain, first 40):\n");
            if g.status_porcelain.is_empty() {
                head.push_str("  (clean)\n");
            } else {
                let filtered =
                    filter_paths_block(&g.status_porcelain, &opts.privacy, porcelain_path);
                head.push_str(&filtered);
            }
            head.push_str("\n\nrecent commits (last 30):\n");
            head.push_str(&g.recent_commits);
            head.push_str("\n\ndiffstat, last 14 days:\n");
            if g.diffstat_14d.is_empty() {
                head.push_str("  (none)");
            } else {
                let filtered = filter_paths_block(&g.diffstat_14d, &opts.privacy, numstat_path);
                head.push_str(&filtered);
            }
            head.push('\n');
        }
        None => head.push_str("(no git — state derived from sessions/files)\n"),
    }
    head.push_str(
        "\n## SESSIONS (recent, normalized — user/assistant only, tool noise stripped)\n",
    );

    // Flatten the most-recent K sessions into chronological lines (session header,
    // then each message line).
    let recent: Vec<&Session> = {
        let mut v: Vec<&Session> = sub.sessions.iter().rev().take(opts.last_k).collect();
        v.reverse(); // chronological: oldest..newest of the K
        v
    };
    let mut lines: Vec<String> = Vec::new();
    for s in &recent {
        lines.push(session_header(s));
        for m in &s.messages {
            if let Some(l) = render_msg_line(m, opts.msg_trunc, &opts.privacy) {
                lines.push(l);
            }
        }
    }

    // Drop from the front (oldest) until within budget, keeping the tail (newest).
    let budget = opts.cap_chars.saturating_sub(head.len() + ELISION.len());
    let mut total_len: usize = lines.iter().map(|l| l.len() + 1).sum();
    let mut start = 0;
    while start < lines.len() && total_len > budget {
        total_len -= lines[start].len() + 1;
        start += 1;
    }
    // Degenerate case (even the single newest line exceeds budget): keep it anyway,
    // so a re-entry digest is never empty.
    if start >= lines.len() && !lines.is_empty() {
        start = lines.len() - 1;
    }

    let mut out = head;
    if start > 0 {
        out.push_str(ELISION);
    }
    for l in &lines[start..] {
        out.push_str(l);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniproj_core::{
        Message, ProjectId, ProjectSource, ProjectSourceId, ProjectSourceKind, ProjectSourceStatus,
        Role, Session, Source,
    };

    #[test]
    fn capture_source_keeps_explicit_identity_separate_from_location() {
        let location =
            std::env::temp_dir().join(format!("omniproj-explicit-capture-{}", std::process::id()));
        std::fs::create_dir_all(&location).unwrap();
        let project_id = ProjectId::parse("deadbeefdeadbeef").unwrap();
        let source = ProjectSource {
            id: ProjectSourceId::parse("source-id").unwrap(),
            project_id: project_id.clone(),
            kind: ProjectSourceKind::GitRepo,
            location: location.to_string_lossy().into_owned(),
            is_primary: true,
            status: ProjectSourceStatus::Available,
            created_at: "2026-08-11T12:00:00Z".into(),
            last_observed_at: None,
            last_successful_refresh_at: None,
            last_error_category: None,
            revision: 0,
        };

        let captured = capture_source(&project_id, &source).unwrap();

        assert_eq!(captured.project_id, project_id);
        assert_eq!(
            captured.location,
            std::fs::canonicalize(&location).unwrap().to_string_lossy()
        );
        std::fs::remove_dir_all(location).ok();
    }

    fn big_session(n: usize) -> Session {
        let messages = (0..n)
            .map(|i| Message {
                idx: i as u64,
                role: if i % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                },
                text: format!("msg-{i:03} some representative content here"),
                ts: None,
            })
            .collect();
        Session {
            id: "s1".into(),
            source: Source::Claude,
            cwd: "/tmp/p".into(),
            started_at: None,
            ended_at: None,
            mtime: 100.0,
            messages,
        }
    }

    fn substrate(sessions: Vec<Session>) -> Substrate {
        let claude_n = sessions.len();
        Substrate {
            project_id: ProjectId::parse("project-capture-test").unwrap(),
            location: "/tmp/p".into(),
            name: "p".into(),
            git: None,
            sessions,
            claude_n,
            codex_n: 0,
        }
    }

    #[test]
    fn recency_keeps_newest_messages_in_one_big_session() {
        let sub = substrate(vec![big_session(100)]);
        let opts = DigestOpts {
            last_k: 4,
            msg_trunc: 1400,
            cap_chars: 1200,
            ..Default::default()
        };
        let out = render_digest(&sub, &opts);
        assert!(
            out.contains("msg-099"),
            "newest message must survive the cap"
        );
        assert!(!out.contains("msg-000"), "oldest message should be elided");
        assert!(
            out.contains("仅保留最近"),
            "elision marker present when trimmed"
        );
    }

    #[test]
    fn no_elision_when_it_all_fits() {
        let sub = substrate(vec![big_session(4)]);
        let out = render_digest(&sub, &Default::default());
        assert!(out.contains("msg-000") && out.contains("msg-003"));
        assert!(!out.contains("仅保留最近"));
    }

    #[test]
    fn older_sessions_are_newest_first_and_exclude_recent_window() {
        let mut s1 = big_session(2);
        s1.id = "old-1".into();
        s1.mtime = 10.0;
        let mut s2 = big_session(2);
        s2.id = "old-2".into();
        s2.mtime = 20.0;
        let mut s3 = big_session(2);
        s3.id = "recent".into();
        s3.mtime = 30.0;
        let sub = substrate(vec![s1, s2, s3]); // ascending mtime, like capture()
        let older = older_session_texts(&sub, 1, 1400, 10_000, &PrivacyPolicy::default()); // window = 1 newest
        assert_eq!(older.len(), 2);
        assert!(older[0].contains("old-2"), "newest of the older ones first");
        assert!(older[1].contains("old-1"));
        assert!(!older.iter().any(|t| t.contains("recent")));
    }

    #[test]
    fn session_text_keeps_tail_when_capped() {
        let s = big_session(60);
        let out = render_session_text(&s, 1400, 600, &PrivacyPolicy::default());
        assert!(out.contains("msg-059"), "newest line survives the cap");
        assert!(out.starts_with("…[较旧内容已省略"));
    }

    /// W1-1: a secret pasted into a session message must be masked in the digest
    /// (the outbound-to-LLM text), while ordinary content survives.
    #[test]
    fn digest_redacts_session_secrets_by_default() {
        let mut s = big_session(2);
        s.messages[0].text = "my key is sk-abcdef0123456789ABCDEF pls use it".into();
        let sub = substrate(vec![s]);
        let out = render_digest(&sub, &Default::default());
        assert!(
            !out.contains("sk-abcdef0123456789ABCDEF"),
            "raw secret must not appear in the outbound digest"
        );
        assert!(
            out.contains("«redacted:openai-key»"),
            "masked with a type marker"
        );
    }

    /// W1-1: `--no-redact` (permissive policy) leaves session text untouched.
    #[test]
    fn digest_no_redact_passes_secret_through() {
        let mut s = big_session(2);
        s.messages[0].text = "my key is sk-abcdef0123456789ABCDEF pls use it".into();
        let sub = substrate(vec![s]);
        let opts = DigestOpts {
            privacy: PrivacyPolicy::permissive(),
            ..Default::default()
        };
        let out = render_digest(&sub, &opts);
        assert!(out.contains("sk-abcdef0123456789ABCDEF"));
    }

    /// W1-1: deny-listed paths are stripped from the git status/diffstat blocks.
    #[test]
    fn digest_strips_deny_listed_git_paths() {
        let git = GitInfo {
            branch: "main".into(),
            head: "abc1234".into(),
            status_porcelain: " M src/main.rs\n?? .env.local\n M config/prod.key".into(),
            status_digest: "d".into(),
            recent_commits: "  2026-07-12 abc1234 wip".into(),
            diffstat_14d: "3\t1\tsrc/main.rs\n10\t0\t.env".into(),
            commit_hashes: vec![],
            file_paths: vec![],
        };
        let sub = Substrate {
            project_id: ProjectId::parse("project-capture-git").unwrap(),
            location: "/tmp/p".into(),
            name: "p".into(),
            git: Some(git),
            sessions: vec![],
            claude_n: 0,
            codex_n: 0,
        };
        let out = render_digest(&sub, &Default::default());
        assert!(out.contains("src/main.rs"), "normal path survives");
        assert!(
            !out.contains(".env.local"),
            "denied path dropped from status"
        );
        assert!(
            !out.contains("config/prod.key"),
            "denied *.key dropped from status"
        );
        assert!(!out.contains("\t.env"), "denied path dropped from diffstat");
        assert!(
            out.contains("«redacted: sensitive path"),
            "deny marker present"
        );
    }

    #[test]
    fn owner_cwd_parses_both_session_formats() {
        let dir = std::env::temp_dir().join(format!("omniproj-owner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let claude = dir.join("c.jsonl");
        std::fs::write(
            &claude,
            r#"{"type":"user","cwd":"/Users/x/proj","message":{"content":"hi"}}"#,
        )
        .unwrap();
        assert_eq!(session_owner_cwd(&claude).as_deref(), Some("/Users/x/proj"));

        let codex = dir.join("rollout-x.jsonl");
        std::fs::write(
            &codex,
            r#"{"type":"session_meta","timestamp":"2026-06-11T00:00:00Z","payload":{"cwd":"/Users/x/other","id":"s"}}"#,
        )
        .unwrap();
        assert_eq!(session_owner_cwd(&codex).as_deref(), Some("/Users/x/other"));

        let junk = dir.join("notes.txt");
        std::fs::write(&junk, "not a session").unwrap();
        assert_eq!(session_owner_cwd(&junk), None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
