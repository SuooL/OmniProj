//! `notes/next.md` — the user's per-project next-action list (cockpit P1).
//!
//! This is USER ground truth (charter §5 原则4): AI never writes it, only the user
//! does — via `omniproj note` / the dashboard, or by hand in any editor. The on-disk
//! form is therefore a **plain GitHub-flavored markdown checklist**, readable and
//! editable without OmniProj:
//!
//! ```text
//! # Next — <project>
//!
//! - [ ] wire the adaptive sampler into render::integrate <!--#a3f1-->
//! - [ ] ? should the caustics regression go in CI <!--#b7c2-->
//! - [x] fix the denoiser fixed-samples assumption <!--#c9d0-->
//! ```
//!
//! Two conventions on top of standard markdown, both survive any renderer:
//! - a leading `?` marks an item as **未成形** (thought not yet clear) — the hook
//!   for a later `omniproj clarify` pass; it is one character and needs no tooling.
//! - a trailing `<!--#id-->` HTML comment (invisible in every markdown viewer) is a
//!   stable id so a clarification discussion stays attached even after the text is
//!   edited. The parser tolerates its absence — a hand-added item just has no id
//!   until the next tool write assigns one.
//!
//! The document round-trips: non-checklist lines (headings, blank lines, prose) are
//! preserved verbatim so hand-editing is never clobbered.

use std::path::PathBuf;

use crate::paths::{content_hash, notes_dir};

/// `~/.omniproj/projects/<hash>/notes/next.md`.
pub fn next_path(hash: &str) -> PathBuf {
    notes_dir(hash).join("next.md")
}

/// One actionable line in `next.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct NextItem {
    /// Short stable id (4+ hex). `None` for a hand-added line the tool hasn't stamped yet.
    pub id: Option<String>,
    pub text: String,
    pub done: bool,
    /// `?`-prefixed: the user flagged this as not-yet-thought-through.
    pub unclear: bool,
}

/// A parsed `next.md`. `lines` preserves the full document (tasks + raw passthrough)
/// so a read-modify-write never loses hand-authored headings or prose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NextDoc {
    lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq)]
enum Line {
    Task(NextItem),
    Raw(String),
}

impl NextDoc {
    /// Parse markdown text. Never fails: anything that isn't a checklist item is kept
    /// as a raw passthrough line.
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        for raw in text.lines() {
            match parse_task_line(raw) {
                Some(item) => lines.push(Line::Task(item)),
                None => lines.push(Line::Raw(raw.to_string())),
            }
        }
        NextDoc { lines }
    }

    /// Load from disk. A missing file is an empty document (not an error).
    pub fn load(hash: &str) -> Self {
        match std::fs::read_to_string(next_path(hash)) {
            Ok(text) => Self::parse(&text),
            Err(_) => NextDoc::default(),
        }
    }

    /// All task lines in document order (skips headings/prose).
    pub fn items(&self) -> impl Iterator<Item = &NextItem> {
        self.lines.iter().filter_map(|l| match l {
            Line::Task(t) => Some(t),
            Line::Raw(_) => None,
        })
    }

    /// Count of not-yet-done items, and of those the count still marked unclear.
    /// (open, unclear) — the two numbers the portfolio card shows.
    pub fn counts(&self) -> (usize, usize) {
        let open: Vec<_> = self.items().filter(|t| !t.done).collect();
        let unclear = open.iter().filter(|t| t.unclear).count();
        (open.len(), unclear)
    }

    /// Append a new item, assigning it a fresh id unique within this document.
    /// Returns the id. The caller renders + writes.
    pub fn add(&mut self, text: &str, unclear: bool) -> String {
        let id = self.fresh_id(text);
        self.lines.push(Line::Task(NextItem {
            id: Some(id.clone()),
            text: text.trim().to_string(),
            done: false,
            unclear,
        }));
        id
    }

    /// Mark an item done/undone by id. Returns true if found.
    pub fn set_done(&mut self, id: &str, done: bool) -> bool {
        for l in &mut self.lines {
            if let Line::Task(t) = l {
                if t.id.as_deref() == Some(id) {
                    t.done = done;
                    return true;
                }
            }
        }
        false
    }

    /// Remove an item by id. Returns true if found.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.lines.len();
        self.lines
            .retain(|l| !matches!(l, Line::Task(t) if t.id.as_deref() == Some(id)));
        self.lines.len() != before
    }

    /// Render back to markdown. Round-trips `parse`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            match l {
                Line::Task(t) => out.push_str(&render_task_line(t)),
                Line::Raw(r) => out.push_str(r),
            }
            out.push('\n');
        }
        out
    }

    /// Write to `next.md` (creating `notes/`). The document is user ground truth, so
    /// callers must only invoke this for user-initiated edits, never from distill.
    pub fn save(&self, hash: &str) -> std::io::Result<()> {
        let path = next_path(hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render())
    }

    /// A 4-hex id derived from the text, salted until unique within this document.
    /// Pure (no clock/rng) so it's testable and stable for the same input.
    fn fresh_id(&self, text: &str) -> String {
        let existing: std::collections::HashSet<&str> =
            self.items().filter_map(|t| t.id.as_deref()).collect();
        for salt in 0u32.. {
            let cand = &content_hash(&format!("{text}\u{0}{salt}"))[..4];
            if !existing.contains(cand) {
                return cand.to_string();
            }
        }
        unreachable!("4-hex space exhausted")
    }
}

/// Parse a single `- [ ] ...` / `- [x] ...` line into a task, or None if it isn't one.
fn parse_task_line(raw: &str) -> Option<NextItem> {
    let trimmed = raw.trim_start();
    let rest = trimmed.strip_prefix("- [")?;
    let (mark, rest) = rest.split_at(rest.char_indices().nth(1)?.0.max(1));
    // rest now begins at the char after the checkbox char; expect "] "
    let done = match mark {
        " " => false,
        "x" | "X" => true,
        _ => return None,
    };
    let mut body = rest.strip_prefix("]")?.trim_start();
    // Extract a trailing <!--#id--> if present.
    let mut id = None;
    if let Some(open) = body.rfind("<!--#") {
        if let Some(close_rel) = body[open..].find("-->") {
            let inner = &body[open + 5..open + close_rel];
            if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_hexdigit()) {
                id = Some(inner.to_string());
                body = body[..open].trim_end();
            }
        }
    }
    // Leading `?` marks unclear.
    let (unclear, text) = match body.strip_prefix('?') {
        Some(after) => (true, after.trim_start()),
        None => (false, body),
    };
    Some(NextItem {
        id,
        text: text.to_string(),
        done,
        unclear,
    })
}

fn render_task_line(t: &NextItem) -> String {
    let check = if t.done { "x" } else { " " };
    let prefix = if t.unclear { "? " } else { "" };
    let suffix = match &t.id {
        Some(id) => format!(" <!--#{id}-->"),
        None => String::new(),
    };
    format!("- [{check}] {prefix}{}{suffix}", t.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_done_and_unclear() {
        let doc = NextDoc::parse(
            "# Next\n\n- [ ] plain open <!--#a1b2-->\n- [x] done item <!--#c3d4-->\n- [ ] ? fuzzy one <!--#e5f6-->\n",
        );
        let items: Vec<_> = doc.items().cloned().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].text, "plain open");
        assert!(!items[0].done && !items[0].unclear);
        assert!(items[1].done);
        assert!(items[2].unclear && !items[2].done);
        assert_eq!(items[2].id.as_deref(), Some("e5f6"));
    }

    #[test]
    fn round_trips_headings_and_prose() {
        let src = "# Next — foo\n\nSome prose the user wrote.\n\n- [ ] a task <!--#dead-->\n";
        let doc = NextDoc::parse(src);
        assert_eq!(doc.render(), src);
    }

    #[test]
    fn hand_added_item_without_id_is_tolerated() {
        let doc = NextDoc::parse("- [ ] no id here\n");
        let items: Vec<_> = doc.items().cloned().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, None);
        assert_eq!(items[0].text, "no id here");
    }

    #[test]
    fn add_assigns_unique_ids() {
        let mut doc = NextDoc::default();
        let id1 = doc.add("first", false);
        let id2 = doc.add("first", false); // identical text must still get a distinct id
        assert_ne!(id1, id2);
        assert_eq!(doc.items().count(), 2);
        assert_eq!(id1.len(), 4);
    }

    #[test]
    fn counts_open_and_unclear() {
        let doc = NextDoc::parse(
            "- [ ] a <!--#1111-->\n- [x] b <!--#2222-->\n- [ ] ? c <!--#3333-->\n- [ ] ? d <!--#4444-->\n",
        );
        // 3 open (a, c, d); 2 of them unclear (c, d). done item b excluded.
        assert_eq!(doc.counts(), (3, 2));
    }

    #[test]
    fn set_done_and_remove_by_id() {
        let mut doc = NextDoc::parse("- [ ] a <!--#1111-->\n- [ ] b <!--#2222-->\n");
        assert!(doc.set_done("1111", true));
        assert!(
            doc.items()
                .find(|t| t.id.as_deref() == Some("1111"))
                .unwrap()
                .done
        );
        assert!(!doc.set_done("nope", true));
        assert!(doc.remove("2222"));
        assert_eq!(doc.items().count(), 1);
        assert!(!doc.remove("2222"));
    }

    #[test]
    fn unclear_and_id_survive_round_trip_after_mutation() {
        let mut doc = NextDoc::default();
        let id = doc.add("think about X", true);
        let rendered = doc.render();
        assert!(rendered.contains("- [ ] ? think about X <!--#"));
        let reparsed = NextDoc::parse(&rendered);
        let item = reparsed.items().next().unwrap();
        assert!(item.unclear);
        assert_eq!(item.id.as_deref(), Some(id.as_str()));
    }
}
