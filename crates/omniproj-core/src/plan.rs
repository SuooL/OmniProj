//! `plan.md` — the project's plan / decision log (Record layer, M4). Append-only, a light
//! ADR: entries record what you decided, **including "decided NOT to do X"** (status
//! `abandoned`, marked not deleted — superseded style, charter §4a/§7). User ground truth;
//! the AI never writes it. Human-readable markdown that round-trips:
//!
//! ```text
//! # Plan — <project>
//!
//! ## 2026-08-10 — Chose stratified over Halton sequences <!--#a1b2 status:done commit:3f062b1-->
//! Better cache locality on the tiled backend.
//!
//! ## 2026-08-09 — Decided NOT to add a plugin system <!--#c3d4 status:abandoned-->
//! Scope creep; the provider abstraction already covers the real need.
//! ```

use std::path::PathBuf;

use crate::paths::{content_hash, project_dir};

/// `~/.omniproj/projects/<hash>/plan.md`.
pub fn plan_path(hash: &str) -> PathBuf {
    project_dir(hash).join("plan.md")
}

/// Decision status (charter §7 ADR): the middle two track work; `Abandoned` is the
/// "decided not to / superseded" marker that is never deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStatus {
    Planned,
    Doing,
    Done,
    Abandoned,
}

impl PlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlanStatus::Planned => "planned",
            PlanStatus::Doing => "doing",
            PlanStatus::Done => "done",
            PlanStatus::Abandoned => "abandoned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "planned" => Some(PlanStatus::Planned),
            "doing" => Some(PlanStatus::Doing),
            "done" => Some(PlanStatus::Done),
            "abandoned" => Some(PlanStatus::Abandoned),
            _ => None,
        }
    }
}

/// One plan/decision entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanEntry {
    pub id: Option<String>,
    /// `YYYY-MM-DD` (supplied by the caller — core stays clock-free).
    pub date: String,
    pub title: String,
    pub status: PlanStatus,
    /// Optional linked commit (abbreviated SHA) — the *actual* this decision landed as.
    pub commit: Option<String>,
    /// Free-text rationale (may span lines).
    pub body: String,
}

/// A parsed `plan.md`: a raw preamble (anything before the first entry, e.g. a title) plus
/// the entries in document order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanDoc {
    preamble: String,
    entries: Vec<PlanEntry>,
}

impl PlanDoc {
    pub fn parse(text: &str) -> Self {
        let mut preamble = String::new();
        let mut entries: Vec<PlanEntry> = Vec::new();
        let mut body = String::new();
        let mut in_entry = false;
        for line in text.lines() {
            if let Some(entry) = parse_heading(line) {
                if let Some(last) = entries.last_mut() {
                    last.body = body.trim().to_string();
                }
                body.clear();
                entries.push(entry);
                in_entry = true;
            } else if in_entry {
                body.push_str(line);
                body.push('\n');
            } else {
                preamble.push_str(line);
                preamble.push('\n');
            }
        }
        if let Some(last) = entries.last_mut() {
            last.body = body.trim().to_string();
        }
        PlanDoc {
            preamble: preamble.trim().to_string(),
            entries,
        }
    }

    pub fn load(hash: &str) -> Self {
        match std::fs::read_to_string(plan_path(hash)) {
            Ok(text) => Self::parse(&text),
            Err(_) => PlanDoc::default(),
        }
    }

    pub fn entries(&self) -> &[PlanEntry] {
        &self.entries
    }

    /// Append a new decision (status `Planned`); returns its id. `date` is `YYYY-MM-DD`.
    pub fn add(&mut self, date: &str, title: &str, body: &str) -> String {
        let id = self.fresh_id(title);
        self.entries.push(PlanEntry {
            id: Some(id.clone()),
            date: date.trim().to_string(),
            title: title.trim().to_string(),
            status: PlanStatus::Planned,
            commit: None,
            body: body.trim().to_string(),
        });
        id
    }

    /// Set an entry's status by id (e.g. mark a decision `abandoned` — never delete it).
    pub fn set_status(&mut self, id: &str, status: PlanStatus) -> bool {
        for e in &mut self.entries {
            if e.id.as_deref() == Some(id) {
                e.status = status;
                return true;
            }
        }
        false
    }

    /// Link (or clear) the commit an entry landed as. `Some` must be hex (4–40).
    pub fn set_commit(&mut self, id: &str, commit: Option<String>) -> bool {
        if let Some(c) = &commit {
            let c = c.trim();
            if !(4..=40).contains(&c.len()) || !c.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return false;
            }
        }
        for e in &mut self.entries {
            if e.id.as_deref() == Some(id) {
                e.commit = commit.map(|c| c.trim().to_string());
                return true;
            }
        }
        false
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.preamble.is_empty() {
            out.push_str(&self.preamble);
            out.push_str("\n\n");
        }
        for e in &self.entries {
            out.push_str(&render_heading(e));
            out.push('\n');
            if !e.body.is_empty() {
                out.push_str(&e.body);
                out.push('\n');
            }
            out.push('\n');
        }
        let s = out.trim_end();
        if s.is_empty() {
            String::new()
        } else {
            format!("{s}\n")
        }
    }

    pub fn save(&self, hash: &str) -> std::io::Result<()> {
        let path = plan_path(hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render())
    }

    fn fresh_id(&self, seed: &str) -> String {
        let existing: std::collections::HashSet<&str> = self
            .entries
            .iter()
            .filter_map(|e| e.id.as_deref())
            .collect();
        for salt in 0u32.. {
            let cand = &content_hash(&format!("{seed}\u{0}{salt}"))[..4];
            if !existing.contains(cand) {
                return cand.to_string();
            }
        }
        unreachable!("4-hex space exhausted")
    }
}

/// Parse a `## <date> — <title> <!--#id status:X commit:Y-->` heading into an entry (body
/// empty; the caller fills it). Any `## ` line is an entry; a missing comment → no id,
/// status `Planned`.
fn parse_heading(line: &str) -> Option<PlanEntry> {
    let mut visible = line.strip_prefix("## ")?.trim().to_string();
    let mut id = None;
    let mut status = PlanStatus::Planned;
    let mut commit = None;
    if let Some(open) = visible.rfind("<!--#") {
        if let Some(close_rel) = visible[open..].find("-->") {
            let inner = visible[open + 5..open + close_rel].to_string();
            let mut tokens = inner.split_whitespace();
            if let Some(first) = tokens.next() {
                if first.chars().all(|c| c.is_ascii_hexdigit()) && !first.is_empty() {
                    id = Some(first.to_string());
                    for kv in tokens {
                        if let Some(s) = kv.strip_prefix("status:") {
                            if let Some(st) = PlanStatus::parse(s) {
                                status = st;
                            }
                        } else if let Some(c) = kv.strip_prefix("commit:") {
                            if !c.is_empty() {
                                commit = Some(c.to_string());
                            }
                        }
                    }
                    visible = visible[..open].trim().to_string();
                }
            }
        }
    }
    let (date, title) = match visible.split_once(" — ") {
        Some((d, t)) => (d.trim().to_string(), t.trim().to_string()),
        None => (String::new(), visible),
    };
    Some(PlanEntry {
        id,
        date,
        title,
        status,
        commit,
        body: String::new(),
    })
}

fn render_heading(e: &PlanEntry) -> String {
    let head = if e.date.is_empty() {
        format!("## {}", e.title)
    } else {
        format!("## {} — {}", e.date, e.title)
    };
    let meta = match &e.id {
        Some(id) => {
            let commit = e
                .commit
                .as_ref()
                .map(|c| format!(" commit:{c}"))
                .unwrap_or_default();
            format!(" <!--#{id} status:{}{commit}-->", e.status.as_str())
        }
        None => String::new(),
    };
    format!("{head}{meta}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_render_parse_round_trips() {
        let mut doc = PlanDoc::default();
        let id = doc.add(
            "2026-08-10",
            "Chose stratified sampling",
            "Better cache locality.",
        );
        let rendered = doc.render();
        assert!(rendered.contains(&format!(
            "## 2026-08-10 — Chose stratified sampling <!--#{id} status:planned-->"
        )));
        assert!(rendered.contains("Better cache locality."));
        let re = PlanDoc::parse(&rendered);
        assert_eq!(re.entries(), doc.entries());
    }

    #[test]
    fn abandoned_decision_is_marked_not_removed() {
        let mut doc = PlanDoc::default();
        let id = doc.add("2026-08-10", "Add a plugin system", "extensibility");
        assert!(doc.set_status(&id, PlanStatus::Abandoned));
        assert_eq!(doc.entries().len(), 1, "abandoned entries are kept");
        assert_eq!(doc.entries()[0].status, PlanStatus::Abandoned);
        assert!(doc.render().contains("status:abandoned"));
    }

    #[test]
    fn set_commit_validates_and_round_trips() {
        let mut doc = PlanDoc::default();
        let id = doc.add("2026-08-10", "Ship the sampler", "");
        assert!(!doc.set_commit(&id, Some("nothex".into())));
        assert!(doc.set_commit(&id, Some("3f062b1".into())));
        let re = PlanDoc::parse(&doc.render());
        assert_eq!(re.entries()[0].commit.as_deref(), Some("3f062b1"));
    }

    #[test]
    fn preamble_and_multiline_body_preserved() {
        let src = "# Plan — photon-tracer\n\n## 2026-08-10 — A decision <!--#a1b2 status:done-->\nline one\nline two\n";
        let doc = PlanDoc::parse(src);
        assert_eq!(doc.entries().len(), 1);
        assert_eq!(doc.entries()[0].body, "line one\nline two");
        assert!(doc.render().starts_with("# Plan — photon-tracer"));
    }
}
