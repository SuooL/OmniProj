//! Normalized session model (spec §4.2): lossy-but-searchable core + raw escape hatch.
//! Both Claude Code and Codex transcripts normalize into these types; source-specific
//! quirks are absorbed in each parser (omniproj-capture), downstream only sees `Session`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Claude,
    Codex,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Claude => "claude",
            Source::Codex => "codex",
        }
    }
}

/// Roles normalized across sources; `Tool` is first-class, `Other` is the escape hatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Linear order index; we do NOT reconstruct the Claude `parentUuid` tree here.
    pub idx: u64,
    pub role: Role,
    /// Flattened plain text (tool/thinking noise stripped for the v1 thin loop).
    pub text: String,
    /// ISO-8601 timestamp if the source provided one.
    pub ts: Option<String>,
}

/// One normalized session (a single transcript file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Native session id (Claude `sessionId` / Codex thread uuid) or the file stem.
    pub id: String,
    pub source: Source,
    /// cwd the session ran in — used to associate a session with a project.
    pub cwd: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// File mtime (epoch seconds) — used for recency-first capping.
    pub mtime: f64,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn user_assistant_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| matches!(m.role, Role::User | Role::Assistant))
            .count()
    }
}
