//! omniproj-index — the derived retrieval layer (spec §4.6, review P1#6).
//!
//! FTS5 over the RAW normalized session text — NOT over distilled summaries. This
//! is the Hermes lesson recorded in spec §4.6: summarize-before-index made live
//! sessions unsearchable, cost money per query, and confabulated; indexing raw text
//! is ~ms and $0. The index is derived state: it lives in the project's `cache/`
//! (gitignored), is rebuilt whenever sessions changed, and can be deleted freely.
//!
//! Tokenizer: `trigram` — substring matching that works for CJK (the unicode61
//! default can't segment Chinese). Queries shorter than 3 chars won't match;
//! acceptable for a recall tool.

use anyhow::{Context, Result};
use omniproj_core::{ProjectId, Session};
use rusqlite::Connection;
use std::path::Path;

/// One search hit, newest-relevance first.
#[derive(Debug)]
pub struct Hit {
    pub session_id: String,
    pub source: String,
    pub role: String,
    /// Session mtime (epoch secs) — when the conversation last moved.
    pub mtime: f64,
    /// Contextual snippet with `«»` around matched spans.
    pub snippet: String,
}

pub fn index_path(project_id: &ProjectId) -> std::path::PathBuf {
    omniproj_core::cache_dir_for(project_id).join("index.sqlite")
}

fn open(project_id: &ProjectId) -> Result<Connection> {
    let path = index_path(project_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).with_context(|| format!("open index {}", path.display()))?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS msg USING fts5(
            text, session_id UNINDEXED, source UNINDEXED, role UNINDEXED,
            mtime UNINDEXED, tokenize='trigram');
         CREATE TABLE IF NOT EXISTS idx_meta(key TEXT PRIMARY KEY, value TEXT);",
    )?;
    Ok(conn)
}

/// The substrate's change signature: rebuild only when it moves. (Full rebuild —
/// sessions are small and parse in ms; incremental bookkeeping isn't worth it yet.)
fn signature(sessions: &[Session]) -> String {
    let max_mtime = sessions.iter().map(|s| s.mtime).fold(0.0f64, f64::max);
    format!("{}:{max_mtime}", sessions.len())
}

/// Ensure the project's index reflects the captured substrate. Returns true when a
/// rebuild happened. Disposable contract: corruption → delete the file and re-run.
pub fn ensure_index_for(project_id: &ProjectId, sessions: &[Session]) -> Result<bool> {
    let conn = open(project_id)?;
    let sig = signature(sessions);
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM idx_meta WHERE key='signature'",
            [],
            |r| r.get(0),
        )
        .ok();
    if stored.as_deref() == Some(sig.as_str()) {
        return Ok(false);
    }
    conn.execute("DELETE FROM msg", [])?;
    {
        let mut ins = conn.prepare(
            "INSERT INTO msg(text, session_id, source, role, mtime) VALUES (?1,?2,?3,?4,?5)",
        )?;
        for s in sessions {
            for m in &s.messages {
                let role = match m.role {
                    omniproj_core::Role::User => "user",
                    omniproj_core::Role::Assistant => "assistant",
                    _ => continue,
                };
                if m.text.trim().is_empty() {
                    continue;
                }
                ins.execute(rusqlite::params![
                    m.text,
                    s.id,
                    s.source.as_str(),
                    role,
                    s.mtime
                ])?;
            }
        }
    }
    conn.execute(
        "INSERT INTO idx_meta(key, value) VALUES ('signature', ?1)
         ON CONFLICT(key) DO UPDATE SET value=?1",
        [&sig],
    )?;
    Ok(true)
}

/// FTS5 search over a project's indexed sessions, best-match first (bm25).
pub fn search_for(project_id: &ProjectId, query: &str, limit: usize) -> Result<Vec<Hit>> {
    let conn = open(project_id)?;
    let mut stmt = conn.prepare(
        "SELECT session_id, source, role, mtime,
                snippet(msg, 0, '«', '»', '…', 16)
         FROM msg WHERE msg MATCH ?1 ORDER BY rank LIMIT ?2",
    )?;
    // Quote the user query as one FTS5 string literal: recall search wants literal
    // matching, not exposing query-syntax operators (AND/OR/NEAR/columns).
    let quoted = format!("\"{}\"", query.replace('"', "\"\""));
    let rows = stmt.query_map(rusqlite::params![quoted, limit as i64], |r| {
        Ok(Hit {
            session_id: r.get(0)?,
            source: r.get(1)?,
            role: r.get(2)?,
            mtime: r.get(3)?,
            snippet: r.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("decode index search result")
}

/// Legacy wrapper for callers which still hold a path-derived substrate.
#[deprecated(note = "use ensure_index_for with a permanent ProjectId")]
pub fn ensure_index(sub: &omniproj_capture::Substrate) -> Result<bool> {
    ensure_index_for(&sub.project_id, &sub.sessions)
}

/// Legacy wrapper for path-derived IDs.
#[deprecated(note = "use search_for with a permanent ProjectId")]
pub fn search(hash: &str, query: &str, limit: usize) -> Result<Vec<Hit>> {
    let project_id =
        ProjectId::parse(hash).context("legacy search hash is not a valid project id")?;
    search_for(&project_id, query, limit)
}

/// Legacy path-based capture, indexing, and search wrapper.
#[deprecated(note = "resolve a registered ProjectSource, then use ensure_index_for and search_for")]
pub fn search_project(dir: &Path, query: &str, limit: usize) -> Result<Vec<Hit>> {
    #[allow(deprecated)]
    let sub = omniproj_capture::capture(dir)?;
    ensure_index_for(&sub.project_id, &sub.sessions)?;
    search_for(&sub.project_id, query, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniproj_core::{Message, ProjectId, Role, Session, Source};

    fn test_sessions(msgs: &[(&str, f64)]) -> Vec<Session> {
        msgs.iter()
            .enumerate()
            .map(|(i, (text, mtime))| Session {
                id: format!("s{i}"),
                source: Source::Claude,
                cwd: "/tmp/p".into(),
                started_at: None,
                ended_at: None,
                mtime: *mtime,
                messages: vec![Message {
                    idx: 0,
                    role: Role::User,
                    text: text.to_string(),
                    ts: None,
                }],
            })
            .collect()
    }

    /// Serialization lock for the process-global `OMNIPROJ_HOME` env var: every test
    /// here repoints it, so two of them running concurrently would race.
    /// Poison-tolerant — a panicking test must not wedge the rest of the suite.
    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A throwaway `~/.omniproj` for one test. Indexing writes a real sqlite file under
    /// `omniproj_core::cache_dir()`, which derives from `OMNIPROJ_HOME` (default `~/.omniproj`),
    /// so without this the suite would litter the *user's* real store with per-test
    /// project dirs. Drop restores the env and removes the temp store.
    struct TempHome(std::path::PathBuf);

    impl TempHome {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("omniproj-index-{}-{tag}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("OMNIPROJ_HOME", &dir);
            Self(dir)
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            std::env::remove_var("OMNIPROJ_HOME");
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn index_and_search_cjk_and_latin() {
        let _g = env_guard();
        let _home = TempHome::new("cjk");
        let project_id = ProjectId::parse("project-index-cjk").unwrap();
        let sessions = test_sessions(&[
            ("我们决定存储层使用 SQLite 而不是 Postgres", 10.0),
            ("the daemon uses a staleness floor", 20.0),
        ]);
        assert!(
            ensure_index_for(&project_id, &sessions).unwrap(),
            "first build indexes"
        );

        let hits = search_for(&project_id, "SQLite", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("«SQLite»"));

        let hits = search_for(&project_id, "存储层", 10).unwrap();
        assert_eq!(hits.len(), 1, "trigram tokenizer must match CJK");

        let hits = search_for(&project_id, "staleness floor", 10).unwrap();
        assert_eq!(hits.len(), 1);

        let hits = search_for(&project_id, "nonexistent-term", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn rebuild_only_when_substrate_changes() {
        let _g = env_guard();
        let _home = TempHome::new("rebuild");
        let project_id = ProjectId::parse("project-index-rebuild").unwrap();
        let sessions = test_sessions(&[("hello world", 10.0)]);
        assert!(ensure_index_for(&project_id, &sessions).unwrap());
        assert!(
            !ensure_index_for(&project_id, &sessions).unwrap(),
            "same signature → no rebuild"
        );

        let sessions2 = test_sessions(&[("hello world", 10.0), ("new session", 30.0)]);
        assert!(
            ensure_index_for(&project_id, &sessions2).unwrap(),
            "new session → rebuild"
        );
        assert_eq!(search_for(&project_id, "new session", 10).unwrap().len(), 1);
    }

    #[test]
    fn query_operators_are_treated_literally() {
        let _g = env_guard();
        let _home = TempHome::new("operators");
        let project_id = ProjectId::parse("project-index-operators").unwrap();
        let sessions = test_sessions(&[("plain text only here", 10.0)]);
        ensure_index_for(&project_id, &sessions).unwrap();
        // would be a syntax error / column filter if not quoted
        assert!(search_for(&project_id, "text AND here", 10)
            .unwrap()
            .is_empty());
        assert!(search_for(&project_id, "role:user", 10).unwrap().is_empty());
    }

    /// Regression guard for the pollution bug: indexing must land under `OMNIPROJ_HOME`,
    /// never in the user's real `~/.omniproj`.
    #[test]
    fn index_stays_inside_temp_home() {
        let _g = env_guard();
        let home = TempHome::new("scoped");
        let project_id = ProjectId::parse("project-index-scoped").unwrap();
        let sessions = test_sessions(&[("hello world", 10.0)]);
        ensure_index_for(&project_id, &sessions).unwrap();
        let path = index_path(&project_id);
        assert!(
            path.starts_with(&home.0),
            "index escaped the sandbox: {}",
            path.display()
        );
        assert!(path.exists(), "index file was not written");
    }
}
