//! Codex rollout parser (spec §4.2).
//! Files: `~/.codex/sessions/YYYY/MM/DD/rollout-<ISO>-<uuid>.jsonl`.
//! Each line is `{type, timestamp, payload}`. cwd lives in `session_meta` (first line);
//! messages are `response_item` with `payload.type == "message"`.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::UNIX_EPOCH;

use omniproj_core::{Message, Role, Session, Source};
use serde_json::Value;
use walkdir::WalkDir;

fn mtime_secs(path: &Path) -> f64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// cwd from the first-line `session_meta` payload.
pub(crate) fn meta_cwd(path: &Path) -> Option<(String, Option<String>, String)> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: Value = serde_json::from_str(&line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("session_meta") {
        return None;
    }
    let payload = v.get("payload")?;
    let cwd = payload.get("cwd").and_then(Value::as_str)?.to_string();
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
    let started = v
        .get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some((cwd, started, id))
}

fn content_text(content: &Value) -> String {
    match content {
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match b.get("type").and_then(Value::as_str) {
                Some("input_text") | Some("output_text") => {
                    b.get("text").and_then(Value::as_str).map(str::to_string)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn parse_file(path: &Path, cwd: String, started_at: Option<String>, id: String) -> Option<Session> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut ended_at: Option<String> = None;
    let mut idx = 0u64;

    for line in reader.lines().map_while(Result::ok) {
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = v.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let role = match payload.get("role").and_then(Value::as_str) {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => continue, // skip developer/system in the thin loop
        };
        let text = payload.get("content").map(content_text).unwrap_or_default();
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let ts = v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        if ts.is_some() {
            ended_at = ts.clone();
        }
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
    Some(Session {
        id,
        source: Source::Codex,
        cwd,
        started_at,
        ended_at,
        mtime: mtime_secs(path),
        messages,
    })
}

/// All Codex sessions whose `session_meta.cwd` matches `target_cwd`.
pub fn sessions_for_cwd(target_cwd: &str) -> Vec<Session> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let root = home.join(".codex").join("sessions");
    if !root.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
            continue;
        }
        let Some((cwd, started, id)) = meta_cwd(p) else {
            continue;
        };
        if cwd != target_cwd {
            continue;
        }
        if let Some(s) = parse_file(p, cwd, started, id) {
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique rollout file path (hermetic — never touches the user's real ~/.codex).
    fn tmp_rollout(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "omniproj-codex-test-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("rollout-2026-06-01T10-00-00-uuid.jsonl")
    }

    /// Codex rollout shape (spec §4.2): first line is `session_meta` (carries cwd),
    /// messages are `response_item` with `payload.type == "message"` and content
    /// blocks of `input_text` (user) / `output_text` (assistant). Non-message
    /// response_items (function_call) and turn_context lines must be skipped.
    const SAMPLE: &str = concat!(
        r#"{"type":"session_meta","timestamp":"2026-06-01T10:00:00Z","payload":{"cwd":"/Users/x/codexproj","id":"sess-42"}}"#,
        "\n",
        r#"{"type":"turn_context","timestamp":"2026-06-01T10:00:01Z","payload":{"cwd":"/Users/x/codexproj"}}"#,
        "\n",
        r#"{"type":"response_item","timestamp":"2026-06-01T10:00:02Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add a test please"}]}}"#,
        "\n",
        r#"{"type":"response_item","timestamp":"2026-06-01T10:00:07Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done, added it"}]}}"#,
        "\n",
        r#"{"type":"response_item","timestamp":"2026-06-01T10:00:08Z","payload":{"type":"function_call","name":"bash","arguments":"{}"}}"#,
        "\n",
    );

    #[test]
    fn meta_cwd_reads_session_meta() {
        let path = tmp_rollout("meta");
        std::fs::write(&path, SAMPLE).unwrap();
        let (cwd, started, id) = meta_cwd(&path).expect("session_meta first line");
        assert_eq!(cwd, "/Users/x/codexproj");
        assert_eq!(started.as_deref(), Some("2026-06-01T10:00:00Z"));
        assert_eq!(id, "sess-42");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn parses_codex_rollout_to_unified_session() {
        let path = tmp_rollout("parse");
        std::fs::write(&path, SAMPLE).unwrap();
        let (cwd, started, id) = meta_cwd(&path).unwrap();

        let s = parse_file(&path, cwd, started, id).expect("sample yields a session");
        assert_eq!(s.source, Source::Codex);
        assert_eq!(s.cwd, "/Users/x/codexproj");
        assert_eq!(s.id, "sess-42");
        // turn_context + function_call skipped → 2 real messages.
        assert_eq!(s.messages.len(), 2);
        assert_eq!(s.messages[0].role, Role::User);
        assert_eq!(s.messages[0].text, "add a test please");
        assert_eq!(s.messages[0].idx, 0);
        assert_eq!(s.messages[1].role, Role::Assistant);
        assert_eq!(s.messages[1].text, "done, added it");
        assert_eq!(s.messages[1].idx, 1);
        assert_eq!(s.ended_at.as_deref(), Some("2026-06-01T10:00:07Z"));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn non_session_meta_first_line_is_none() {
        let path = tmp_rollout("notmeta");
        std::fs::write(
            &path,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[]}}"#,
        )
        .unwrap();
        assert!(meta_cwd(&path).is_none(), "first line isn't session_meta");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
