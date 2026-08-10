//! Claude Code transcript parser (spec §4.2).
//! Files: `~/.claude/projects/<encoded-cwd>/<sessionId>.jsonl`, flat typed lines.
//! Each message line carries `cwd` + `gitBranch`; content is string OR a block array.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use omniproj_core::{Message, Role, Session, Source};
use serde_json::Value;

fn claude_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("projects"))
}

fn mtime_secs(path: &Path) -> f64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Peek the first non-null `cwd` in a transcript (used to associate it with a project).
pub(crate) fn first_cwd(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().take(40).map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if let Some(c) = v.get("cwd").and_then(Value::as_str) {
                if !c.is_empty() {
                    return Some(c.to_string());
                }
            }
        }
    }
    None
}

/// Extract flattened text from a Claude `message.content` (string or block array).
fn content_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| {
                if b.get("type").and_then(Value::as_str) == Some("text") {
                    b.get("text").and_then(Value::as_str).map(str::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn parse_file(path: &Path) -> Option<Session> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut cwd = String::new();
    let mut started_at: Option<String> = None;
    let mut ended_at: Option<String> = None;
    let mut idx = 0u64;

    for line in reader.lines().map_while(Result::ok) {
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // fail-soft per line (spec §4.2)
        };
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        if ty != "user" && ty != "assistant" {
            continue;
        }
        if cwd.is_empty() {
            if let Some(c) = v.get("cwd").and_then(Value::as_str) {
                cwd = c.to_string();
            }
        }
        let ts = v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        if started_at.is_none() {
            started_at = ts.clone();
        }
        if ts.is_some() {
            ended_at = ts.clone();
        }
        let content = v.get("message").and_then(|m| m.get("content"));
        let text = content.map(content_text).unwrap_or_default();
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let role = if ty == "user" {
            Role::User
        } else {
            Role::Assistant
        };
        messages.push(Message {
            idx,
            role,
            text,
            ts,
        });
        idx += 1;
    }

    if messages.is_empty() {
        return None;
    }
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    Some(Session {
        id,
        source: Source::Claude,
        cwd,
        started_at,
        ended_at,
        mtime: mtime_secs(path),
        messages,
    })
}

/// All Claude sessions whose cwd matches `target_cwd`.
pub fn sessions_for_cwd(target_cwd: &str) -> Vec<Session> {
    let Some(root) = claude_root() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let Ok(dirs) = fs::read_dir(&root) else {
        return out;
    };
    for dir in dirs.flatten() {
        let p = dir.path();
        if !p.is_dir() {
            continue;
        }
        // collect *.jsonl
        let Ok(files) = fs::read_dir(&p) else {
            continue;
        };
        let jsonls: Vec<PathBuf> = files
            .flatten()
            .map(|e| e.path())
            .filter(|f| f.extension().and_then(|e| e.to_str()) == Some("jsonl"))
            .collect();
        if jsonls.is_empty() {
            continue;
        }
        // dir maps to one cwd: peek the first jsonl that yields a cwd
        let dir_cwd = jsonls.iter().find_map(|f| first_cwd(f));
        if dir_cwd.as_deref() != Some(target_cwd) {
            continue;
        }
        for f in jsonls {
            if let Some(s) = parse_file(&f) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique file path in a tempdir (no tempfile dep; hermetic — never touches the
    /// user's real ~/.claude). Caller removes the parent dir.
    fn tmp_jsonl(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "omniproj-claude-test-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("abc123.jsonl")
    }

    /// Two message shapes Claude actually emits: user content as a bare string, and
    /// assistant content as a `[{type:text}]` block array (spec §4.2). Interleaved
    /// with a non-conversation `summary` line that must be skipped.
    const SAMPLE: &str = concat!(
        r#"{"type":"summary","summary":"noise line, not a message"}"#,
        "\n",
        r#"{"type":"user","cwd":"/Users/x/proj","timestamp":"2026-06-01T10:00:00Z","message":{"content":"how do I run the tests?"}}"#,
        "\n",
        r#"{"type":"assistant","cwd":"/Users/x/proj","timestamp":"2026-06-01T10:00:05Z","message":{"content":[{"type":"text","text":"run cargo test"},{"type":"tool_use","name":"bash"}]}}"#,
        "\n",
        r#"{"type":"user","cwd":"/Users/x/proj","timestamp":"2026-06-01T10:01:00Z","message":{"content":"  "}}"#,
        "\n",
    );

    #[test]
    fn parses_claude_jsonl_to_unified_session() {
        let path = tmp_jsonl("parse");
        std::fs::write(&path, SAMPLE).unwrap();

        let s = parse_file(&path).expect("sample yields a session");
        assert_eq!(s.source, Source::Claude);
        assert_eq!(s.cwd, "/Users/x/proj");
        assert_eq!(s.id, "abc123", "id is the file stem");
        // summary line skipped; empty-whitespace user message dropped → 2 real messages.
        assert_eq!(s.messages.len(), 2);
        assert_eq!(s.messages[0].role, Role::User);
        assert_eq!(s.messages[0].text, "how do I run the tests?");
        assert_eq!(s.messages[0].idx, 0);
        assert_eq!(s.messages[1].role, Role::Assistant);
        // block array flattened to text-only content (tool_use dropped).
        assert_eq!(s.messages[1].text, "run cargo test");
        assert_eq!(s.messages[1].idx, 1);
        assert_eq!(s.started_at.as_deref(), Some("2026-06-01T10:00:00Z"));
        // ended_at tracks the last *timestamped* line, even one whose text was empty
        // (the parser advances ended_at before the empty-text skip).
        assert_eq!(s.ended_at.as_deref(), Some("2026-06-01T10:01:00Z"));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn first_cwd_peeks_the_transcript() {
        let path = tmp_jsonl("cwd");
        std::fs::write(&path, SAMPLE).unwrap();
        assert_eq!(first_cwd(&path).as_deref(), Some("/Users/x/proj"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn malformed_lines_fail_soft() {
        let path = tmp_jsonl("softfail");
        std::fs::write(
            &path,
            concat!(
                "this is not json at all\n",
                r#"{"type":"user","cwd":"/p","message":{"content":"still parsed"}}"#,
                "\n",
                "{ broken json\n",
            ),
        )
        .unwrap();
        let s = parse_file(&path).expect("good line survives bad ones");
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0].text, "still parsed");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn empty_transcript_yields_none() {
        let path = tmp_jsonl("empty");
        std::fs::write(&path, r#"{"type":"summary","summary":"x"}"#).unwrap();
        assert!(
            parse_file(&path).is_none(),
            "no user/assistant → no session"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
