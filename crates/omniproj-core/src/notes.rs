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

/// Tri-state task status (charter §7 细节决策1: open / **doing** / done). The middle
/// `Doing` state is encoded as `[/]` in the markdown checklist — a convention Obsidian
/// and the Tasks plugin already use; `[ ]` = Open, `[x]` = Done. Any renderer shows all
/// three as a checkbox, so the file stays human-readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Doing,
    Done,
}

impl TaskStatus {
    pub fn is_done(self) -> bool {
        matches!(self, TaskStatus::Done)
    }

    /// The checkbox glyph for `- [x]`-style rendering.
    fn checkbox(self) -> &'static str {
        match self {
            TaskStatus::Open => " ",
            TaskStatus::Doing => "/",
            TaskStatus::Done => "x",
        }
    }

    fn from_checkbox(c: &str) -> Option<Self> {
        match c {
            " " => Some(TaskStatus::Open),
            "/" => Some(TaskStatus::Doing),
            "x" | "X" => Some(TaskStatus::Done),
            _ => None,
        }
    }

    /// Stable lowercase token for IPC/serde and CLI (`open` / `doing` / `done`).
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Open => "open",
            TaskStatus::Doing => "doing",
            TaskStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(TaskStatus::Open),
            "doing" => Some(TaskStatus::Doing),
            "done" => Some(TaskStatus::Done),
            _ => None,
        }
    }
}

/// One actionable line in `next.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct NextItem {
    /// Short stable id (4+ hex). `None` for a hand-added line the tool hasn't stamped yet.
    pub id: Option<String>,
    pub text: String,
    pub status: TaskStatus,
    /// `?`-prefixed: the user flagged this as not-yet-thought-through.
    pub unclear: bool,
    /// Expected-completion date `YYYY-MM-DD`, stored in the trailing id comment as
    /// `due:<date>`. `None` when unset. Round-trips; invalid dates are dropped on parse.
    pub due: Option<String>,
    /// Attributed git commit SHAs (abbreviated), the *actual* side of planned-vs-actual
    /// (FR-R2). Many commits → one task. Stored in the id comment as `commits:h1,h2`;
    /// insertion order preserved. Non-hex tokens are dropped on parse.
    pub commits: Vec<String>,
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
        let open: Vec<_> = self.items().filter(|t| !t.status.is_done()).collect();
        let unclear = open.iter().filter(|t| t.unclear).count();
        (open.len(), unclear)
    }

    /// Append a new item (status `Open`), assigning it a fresh id unique within this
    /// document. Returns the id. The caller renders + writes.
    pub fn add(&mut self, text: &str, unclear: bool) -> String {
        let id = self.fresh_id(text);
        self.lines.push(Line::Task(NextItem {
            id: Some(id.clone()),
            text: text.trim().to_string(),
            status: TaskStatus::Open,
            unclear,
            due: None,
            commits: Vec::new(),
        }));
        id
    }

    /// Attribute a git commit (abbreviated SHA) to an item (FR-R2). No-op if already
    /// attributed. The SHA must be hex (4–40 chars). Returns true if the item was found
    /// and the attribution set is now non-redundant (added), false if not found or the
    /// SHA was invalid.
    pub fn attribute_commit(&mut self, id: &str, sha: &str) -> bool {
        let sha = sha.trim();
        if sha.len() < 4 || sha.len() > 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
        for l in &mut self.lines {
            if let Line::Task(t) = l {
                if t.id.as_deref() == Some(id) {
                    if !t.commits.iter().any(|c| c.eq_ignore_ascii_case(sha)) {
                        t.commits.push(sha.to_string());
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Remove a commit attribution from an item. Returns true if the item was found and
    /// the SHA was attributed.
    pub fn unattribute_commit(&mut self, id: &str, sha: &str) -> bool {
        for l in &mut self.lines {
            if let Line::Task(t) = l {
                if t.id.as_deref() == Some(id) {
                    let before = t.commits.len();
                    t.commits.retain(|c| !c.eq_ignore_ascii_case(sha));
                    return t.commits.len() != before;
                }
            }
        }
        false
    }

    /// Set an item's status by id. Returns true if found.
    pub fn set_status(&mut self, id: &str, status: TaskStatus) -> bool {
        for l in &mut self.lines {
            if let Line::Task(t) = l {
                if t.id.as_deref() == Some(id) {
                    t.status = status;
                    return true;
                }
            }
        }
        false
    }

    /// Mark an item done/undone by id (done → `Done`, undone → `Open`). Returns true
    /// if found. Kept as the binary shorthand over [`set_status`].
    pub fn set_done(&mut self, id: &str, done: bool) -> bool {
        self.set_status(
            id,
            if done {
                TaskStatus::Done
            } else {
                TaskStatus::Open
            },
        )
    }

    /// Set (or clear, with `None`) an item's expected-completion date by id. A `Some`
    /// value must be `YYYY-MM-DD`; malformed dates are rejected (returns `false`).
    /// Returns true if the item was found and updated.
    pub fn set_due(&mut self, id: &str, due: Option<String>) -> bool {
        if let Some(d) = &due {
            if !is_ymd(d) {
                return false;
            }
        }
        for l in &mut self.lines {
            if let Line::Task(t) = l {
                if t.id.as_deref() == Some(id) {
                    t.due = due;
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

/// Parse a single `- [ ] ...` / `- [/] ...` / `- [x] ...` line into a task, or None if
/// it isn't one. The trailing `<!--#<id> [due:<date>]-->` comment carries the stable id
/// and optional metadata; both are optional and tolerated in any renderer.
fn parse_task_line(raw: &str) -> Option<NextItem> {
    let trimmed = raw.trim_start();
    let rest = trimmed.strip_prefix("- [")?;
    let (mark, rest) = rest.split_at(rest.char_indices().nth(1)?.0.max(1));
    // rest now begins at the char after the checkbox char; expect "] "
    let status = TaskStatus::from_checkbox(mark)?;
    let mut body = rest.strip_prefix("]")?.trim_start();
    // Extract a trailing <!--#id [key:val ...]--> if present: first token is the hex id,
    // remaining space-separated tokens are metadata (currently `due:<YYYY-MM-DD>`).
    let mut id = None;
    let mut due = None;
    let mut commits = Vec::new();
    if let Some(open) = body.rfind("<!--#") {
        if let Some(close_rel) = body[open..].find("-->") {
            let inner = &body[open + 5..open + close_rel];
            let mut tokens = inner.split_whitespace();
            if let Some(first) = tokens.next() {
                if !first.is_empty() && first.chars().all(|c| c.is_ascii_hexdigit()) {
                    id = Some(first.to_string());
                    for kv in tokens {
                        if let Some(d) = kv.strip_prefix("due:") {
                            if is_ymd(d) {
                                due = Some(d.to_string());
                            }
                        } else if let Some(cs) = kv.strip_prefix("commits:") {
                            for h in cs.split(',') {
                                let h = h.trim();
                                if (4..=40).contains(&h.len())
                                    && h.chars().all(|c| c.is_ascii_hexdigit())
                                {
                                    commits.push(h.to_string());
                                }
                            }
                        }
                    }
                    body = body[..open].trim_end();
                }
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
        status,
        unclear,
        due,
        commits,
    })
}

fn render_task_line(t: &NextItem) -> String {
    let check = t.status.checkbox();
    let prefix = if t.unclear { "? " } else { "" };
    // The id comment also carries metadata (`due:`). Without an id we have nowhere stable
    // to hang metadata, so an id-less line renders bare (the tool assigns ids on add).
    let suffix = match &t.id {
        Some(id) => {
            let due = t
                .due
                .as_ref()
                .map(|d| format!(" due:{d}"))
                .unwrap_or_default();
            let commits = if t.commits.is_empty() {
                String::new()
            } else {
                format!(" commits:{}", t.commits.join(","))
            };
            format!(" <!--#{id}{due}{commits}-->")
        }
        None => String::new(),
    };
    format!("- [{check}] {prefix}{}{suffix}", t.text)
}

/// True iff `s` is a syntactically valid `YYYY-MM-DD` (digit/hyphen shape only — not a
/// calendar validity check; that's the caller's concern).
fn is_ymd(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
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
        assert!(items[0].status == TaskStatus::Open && !items[0].unclear);
        assert_eq!(items[1].status, TaskStatus::Done);
        assert!(items[2].unclear && items[2].status != TaskStatus::Done);
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
        assert_eq!(
            doc.items()
                .find(|t| t.id.as_deref() == Some("1111"))
                .unwrap()
                .status,
            TaskStatus::Done
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

    #[test]
    fn doing_status_parses_and_round_trips() {
        let doc = NextDoc::parse("- [/] mid-flight task <!--#a1b2-->\n");
        let item = doc.items().next().unwrap();
        assert_eq!(item.status, TaskStatus::Doing);
        // `[/]` survives a render round-trip.
        assert_eq!(doc.render(), "- [/] mid-flight task <!--#a1b2-->\n");
    }

    #[test]
    fn due_date_parses_and_round_trips() {
        let src = "- [ ] ship it <!--#a1b2 due:2026-08-15-->\n";
        let doc = NextDoc::parse(src);
        let item = doc.items().next().unwrap();
        assert_eq!(item.due.as_deref(), Some("2026-08-15"));
        assert_eq!(doc.render(), src);
    }

    #[test]
    fn malformed_due_is_dropped_but_id_kept() {
        let doc = NextDoc::parse("- [ ] x <!--#a1b2 due:notadate-->\n");
        let item = doc.items().next().unwrap();
        assert_eq!(item.id.as_deref(), Some("a1b2"));
        assert_eq!(item.due, None);
    }

    #[test]
    fn backward_compat_old_comment_without_due() {
        // A file written by an older OmniProj (no `due:`) parses unchanged.
        let src = "- [x] legacy done <!--#dead-->\n";
        let doc = NextDoc::parse(src);
        let item = doc.items().next().unwrap();
        assert_eq!(item.status, TaskStatus::Done);
        assert_eq!(item.due, None);
        assert_eq!(doc.render(), src);
    }

    #[test]
    fn set_status_and_set_due_by_id() {
        let mut doc = NextDoc::default();
        let id = doc.add("do the thing", false);
        assert!(doc.set_status(&id, TaskStatus::Doing));
        assert!(doc.set_due(&id, Some("2026-09-01".to_string())));
        let rendered = doc.render();
        assert!(rendered.contains(&format!("- [/] do the thing <!--#{id} due:2026-09-01-->")));
        // Round-trips back to the same in-memory state.
        let re = NextDoc::parse(&rendered);
        let item = re.items().next().unwrap();
        assert_eq!(item.status, TaskStatus::Doing);
        assert_eq!(item.due.as_deref(), Some("2026-09-01"));
    }

    #[test]
    fn set_due_rejects_bad_date_and_clears_with_none() {
        let mut doc = NextDoc::default();
        let id = doc.add("x", false);
        assert!(!doc.set_due(&id, Some("2026/09/01".to_string())));
        assert!(doc.set_due(&id, Some("2026-09-01".to_string())));
        assert!(doc.set_due(&id, None)); // clearing is always allowed
        assert_eq!(doc.items().next().unwrap().due, None);
    }

    #[test]
    fn attribute_commits_round_trips_and_dedupes() {
        let mut doc = NextDoc::default();
        let id = doc.add("land the sampler", false);
        assert!(doc.attribute_commit(&id, "abc1234"));
        assert!(doc.attribute_commit(&id, "def5678"));
        assert!(doc.attribute_commit(&id, "abc1234")); // dup is a no-op but still true
        assert_eq!(
            doc.items().next().unwrap().commits,
            vec!["abc1234", "def5678"]
        );

        let rendered = doc.render();
        assert!(rendered.contains("commits:abc1234,def5678"));
        // Round-trips back to the same attribution set + order.
        let re = NextDoc::parse(&rendered);
        assert_eq!(
            re.items().next().unwrap().commits,
            vec!["abc1234", "def5678"]
        );
    }

    #[test]
    fn attribute_rejects_non_hex_and_unattribute_works() {
        let mut doc = NextDoc::default();
        let id = doc.add("x", false);
        assert!(!doc.attribute_commit(&id, "zzz")); // not hex → rejected
        assert!(!doc.attribute_commit("nope", "abc1234")); // unknown id
        assert!(doc.attribute_commit(&id, "abc1234"));
        assert!(doc.unattribute_commit(&id, "ABC1234")); // case-insensitive match
        assert!(doc.items().next().unwrap().commits.is_empty());
        assert!(!doc.unattribute_commit(&id, "abc1234")); // already gone
    }

    #[test]
    fn commits_and_due_coexist_in_one_comment() {
        let src = "- [/] task <!--#a1b2 due:2026-08-20 commits:abc1234,def5678-->\n";
        let doc = NextDoc::parse(src);
        let it = doc.items().next().unwrap();
        assert_eq!(it.due.as_deref(), Some("2026-08-20"));
        assert_eq!(it.commits, vec!["abc1234", "def5678"]);
        assert_eq!(doc.render(), src);
    }
}
