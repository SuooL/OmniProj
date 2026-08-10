//! Git substrate (spec §4.2). v1 shells out to the `git` binary (simplest, most correct);
//! `gix` can replace this behind the same API later. Git is OPTIONAL — `collect` returns
//! `None` for non-git projects, and the pipeline falls back to session/fs signals (spec §5).

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub branch: String,
    pub head: String,
    pub status_porcelain: String,
    /// Stable digest of the full porcelain status, including unstaged, staged and
    /// untracked files. This is the dirty-worktree half of the refresh fingerprint.
    pub status_digest: String,
    pub recent_commits: String,
    pub diffstat_14d: String,
    /// Full 40-char SHAs (last 50) — the verify-gate whitelist (spec §4.7/§5.2).
    pub commit_hashes: Vec<String>,
    /// Repo-relative tracked + porcelain paths — the path-verify whitelist (spec §5.2).
    pub file_paths: Vec<String>,
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
        || git(path, &["rev-parse", "--is-inside-work-tree"])
            .map(|s| s.trim() == "true")
            .unwrap_or(false)
}

pub fn collect(path: &Path) -> Option<GitInfo> {
    if !is_git_repo(path) {
        return None;
    }
    let branch = git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let head = git(path, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let status = git(path, &["status", "--porcelain"]).unwrap_or_default();
    let status_digest = omniproj_core::content_hash(&status);
    // cap porcelain to first 40 lines
    let status_porcelain = status.lines().take(40).collect::<Vec<_>>().join("\n");
    let recent_commits = git(
        path,
        &["log", "-30", "--date=short", "--pretty=  %ad %h %s"],
    )
    .unwrap_or_default();
    let diffstat_14d = git(
        path,
        &["log", "--since=14.days", "--numstat", "--pretty=tformat:"],
    )
    .unwrap_or_default();
    let diffstat_14d = diffstat_14d.chars().take(3000).collect::<String>();
    let commit_hashes = git(path, &["log", "-n", "50", "--pretty=%H"])
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect();
    // Path whitelist for the verify gate (spec §5.2): tracked files + anything in
    // porcelain (untracked-but-present files sessions may reference). Capped — a
    // briefing citing a path outside the first 5000 tracked files is vanishingly
    // rare, and the gate only annotates.
    let mut file_paths: Vec<String> = git(path, &["ls-files"])
        .unwrap_or_default()
        .lines()
        .take(5000)
        .map(|s| s.to_string())
        .collect();
    for line in status.lines() {
        // porcelain v1: XY <path> (renames: "old -> new")
        if line.len() > 3 {
            let p = &line[3..];
            let p = p.split(" -> ").last().unwrap_or(p).trim();
            if !p.is_empty() {
                file_paths.push(p.trim_matches('"').trim_end_matches('/').to_string());
            }
        }
    }
    file_paths.sort();
    file_paths.dedup();

    Some(GitInfo {
        branch,
        head,
        status_porcelain,
        status_digest,
        recent_commits,
        diffstat_14d,
        commit_hashes,
        file_paths,
    })
}

/// One commit for the Record-layer timeline (FR-R2 planned-vs-actual). Structured
/// (unlike the free-text `recent_commits` digest) so the UI can render a real timeline
/// and attach `task ↔ commit` attributions.
#[derive(Debug, Clone)]
pub struct CommitEntry {
    /// Full 40-char SHA.
    pub hash: String,
    /// Abbreviated SHA as git prints it (`%h`).
    pub short: String,
    /// Author date, `YYYY-MM-DD`.
    pub date: String,
    pub author: String,
    pub subject: String,
}

/// Recent commits (newest first, up to `limit`) as structured entries — the *actual*
/// line the user attributes tasks against (FR-R2). Empty for non-git dirs. A `0x1f`
/// field separator keeps subjects with spaces/tabs intact.
pub fn commit_log(path: &Path, limit: usize) -> Vec<CommitEntry> {
    if limit == 0 || !is_git_repo(path) {
        return Vec::new();
    }
    let n = format!("-n{limit}");
    let out = git(
        path,
        &[
            "log",
            &n,
            "--date=short",
            "--pretty=format:%H%x1f%h%x1f%ad%x1f%an%x1f%s",
        ],
    )
    .unwrap_or_default();
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\u{1f}');
            let hash = f.next()?.to_string();
            if hash.is_empty() {
                return None;
            }
            Some(CommitEntry {
                short: f.next()?.to_string(),
                date: f.next()?.to_string(),
                author: f.next()?.to_string(),
                subject: f.next().unwrap_or("").to_string(),
                hash,
            })
        })
        .collect()
}

/// One commit for the branch-aware flow graph (FR-R2 深化, M4): carries parent SHAs and
/// ref decorations so the UI can lay out lanes/merges — the git flow graph is the canvas
/// task↔commit reconciliation happens on (charter §3, NOT a general history browser).
#[derive(Debug, Clone)]
pub struct GraphCommit {
    pub hash: String,
    pub short: String,
    /// Full parent SHAs (2+ = a merge; 0 = a root).
    pub parents: Vec<String>,
    /// Ref labels pointing here (branch names, `HEAD`, tags) — remotes dropped as noise.
    pub refs: Vec<String>,
    pub date: String,
    pub author: String,
    pub subject: String,
}

/// Clean `%D` ref decoration into display labels: unwrap `HEAD -> x`, keep `tag: x` as `x`,
/// drop `origin/*` remotes (dedupe with their local branch).
fn parse_refs(d: &str) -> Vec<String> {
    let mut out = Vec::new();
    for r in d.split(',') {
        let r = r.trim();
        if r.is_empty() {
            continue;
        }
        if r == "HEAD" {
            out.push("HEAD".to_string());
        } else if let Some(b) = r.strip_prefix("HEAD -> ") {
            // Detached-less checkout: HEAD and the branch it points at both sit here.
            out.push("HEAD".to_string());
            if !b.starts_with("origin/") {
                out.push(b.to_string());
            }
        } else if let Some(t) = r.strip_prefix("tag: ") {
            out.push(t.to_string());
        } else if !r.starts_with("origin/") {
            out.push(r.to_string());
        }
    }
    out
}

/// Recent commits (newest first, up to `limit`) with parent + ref data for the flow graph.
/// Merges are included (they're the graph). Empty for non-git dirs.
pub fn commit_graph(path: &Path, limit: usize) -> Vec<GraphCommit> {
    if limit == 0 || !is_git_repo(path) {
        return Vec::new();
    }
    let n = format!("-n{limit}");
    let out = git(
        path,
        &[
            "log",
            &n,
            "--date=short",
            "--pretty=format:%H%x1f%h%x1f%P%x1f%D%x1f%ad%x1f%an%x1f%s",
        ],
    )
    .unwrap_or_default();
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\u{1f}');
            let hash = f.next()?.to_string();
            if hash.is_empty() {
                return None;
            }
            let short = f.next()?.to_string();
            let parents = f
                .next()?
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let refs = parse_refs(f.next()?);
            let date = f.next()?.to_string();
            let author = f.next()?.to_string();
            let subject = f.next().unwrap_or("").to_string();
            Some(GraphCommit {
                hash,
                short,
                parents,
                refs,
                date,
                author,
                subject,
            })
        })
        .collect()
}

/// Weekly commit histogram for the last `n_weeks` (oldest → newest), for the portfolio
/// sparkline (cockpit). Each bucket is a raw count — a neutral activity fact, NOT a
/// score or ranking (charter §5 原则3). `now_epoch` is passed in so this stays
/// clock-free at the seam and testable. Empty vec of zeros when not a git repo.
pub fn commit_weeks(path: &Path, n_weeks: usize, now_epoch: i64) -> Vec<u32> {
    let mut weeks = vec![0u32; n_weeks];
    if n_weeks == 0 || !is_git_repo(path) {
        return weeks;
    }
    let week = 7 * 86_400i64;
    let since = now_epoch - (n_weeks as i64) * week;
    let out = git(
        path,
        &[
            "log",
            &format!("--since={since}"),
            "--pretty=%ct",
            "--no-merges",
        ],
    )
    .unwrap_or_default();
    for line in out.lines() {
        if let Ok(ct) = line.trim().parse::<i64>() {
            // bucket index from oldest(0) → newest(n_weeks-1)
            let age_weeks = ((now_epoch - ct) / week).max(0) as usize;
            if age_weeks < n_weeks {
                weeks[n_weeks - 1 - age_weeks] += 1;
            }
        }
    }
    weeks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique tempdir per call (no tempfile dep; tests run in parallel threads of one
    /// process, so process id alone isn't unique). Caller removes it when done.
    fn unique_tmpdir(tag: &str) -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "omniproj-git-test-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Run `git` in `dir`, panicking on failure (test-only helper).
    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git must be on PATH for capture tests");
        assert!(status.success(), "git {args:?} failed in {dir:?}");
    }

    /// Init a hermetic repo with a deterministic identity + main branch. Neutralize
    /// any global git config that would break the assertions: gpg signing (no key on
    /// CI/contributor boxes → commit fails) and a global excludesFile (could ignore
    /// `scratch.tmp` → porcelain-path assertion fails).
    fn init_repo(dir: &Path) {
        run_git(dir, &["init", "-q", "-b", "main"]);
        run_git(dir, &["config", "user.name", "omniproj-test"]);
        run_git(dir, &["config", "user.email", "test@local"]);
        run_git(dir, &["config", "commit.gpgsign", "false"]);
        run_git(dir, &["config", "core.excludesFile", "/dev/null"]);
    }

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn non_git_dir_returns_none() {
        let dir = unique_tmpdir("nongit");
        assert!(collect(&dir).is_none(), "plain dir has no git substrate");
        assert!(!is_git_repo(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_reports_real_git_facts() {
        let dir = unique_tmpdir("facts");
        init_repo(&dir);
        write(&dir, "src/main.rs", "fn main() {}\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "first commit"]);
        write(&dir, "README.md", "# hello\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "second commit"]);
        // A tracked-but-modified file + an untracked file for the porcelain path check.
        write(&dir, "src/main.rs", "fn main() { /* edit */ }\n");
        write(&dir, "scratch.tmp", "not tracked\n");

        let info = collect(&dir).expect("git repo must yield GitInfo");

        // branch
        assert_eq!(info.branch, "main");

        // head is a short SHA (7..=40 hex chars)
        assert!(
            (7..=40).contains(&info.head.len()) && info.head.chars().all(|c| c.is_ascii_hexdigit()),
            "head should be a short SHA, got {:?}",
            info.head
        );

        // commit_hashes: exactly 2 full 40-char SHAs, newest first, and HEAD is a prefix.
        assert_eq!(info.commit_hashes.len(), 2, "two commits recorded");
        for h in &info.commit_hashes {
            assert_eq!(h.len(), 40, "full 40-char SHA in whitelist: {h}");
            assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert!(
            info.commit_hashes[0].starts_with(&info.head),
            "newest full SHA {} must start with short HEAD {}",
            info.commit_hashes[0],
            info.head
        );

        // file_paths whitelist: tracked files + the untracked porcelain path.
        assert!(info.file_paths.contains(&"src/main.rs".to_string()));
        assert!(info.file_paths.contains(&"README.md".to_string()));
        assert!(
            info.file_paths.contains(&"scratch.tmp".to_string()),
            "untracked porcelain path is whitelisted: {:?}",
            info.file_paths
        );

        // porcelain reflects the modification + the untracked file; digest is stable + 16 hex.
        assert!(info.status_porcelain.contains("src/main.rs"));
        assert!(info.status_porcelain.contains("scratch.tmp"));
        assert_eq!(info.status_digest.len(), 16);
        assert_eq!(
            info.status_digest,
            omniproj_core::content_hash(&git(&dir, &["status", "--porcelain"]).unwrap()),
            "digest is the hash of the full porcelain status"
        );

        // recent_commits carries both subjects.
        assert!(info.recent_commits.contains("first commit"));
        assert!(info.recent_commits.contains("second commit"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clean_worktree_has_empty_status() {
        let dir = unique_tmpdir("clean");
        init_repo(&dir);
        write(&dir, "a.txt", "a\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "only commit"]);

        let info = collect(&dir).expect("git repo");
        assert!(
            info.status_porcelain.is_empty(),
            "clean worktree, empty porcelain"
        );
        assert_eq!(info.commit_hashes.len(), 1);
        assert!(info.file_paths.contains(&"a.txt".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_weeks_buckets_recent_commits_into_newest_slot() {
        let dir = unique_tmpdir("weeks");
        init_repo(&dir);
        write(&dir, "a.txt", "a\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "c1"]);
        run_git(&dir, &["commit", "-q", "--allow-empty", "-m", "c2"]);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let weeks = commit_weeks(&dir, 16, now);
        assert_eq!(weeks.len(), 16);
        // Both commits were made just now → the newest (last) bucket holds them.
        assert_eq!(
            *weeks.last().unwrap(),
            2,
            "both fresh commits in newest week"
        );
        assert_eq!(weeks[..15].iter().sum::<u32>(), 0, "older weeks empty");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_log_returns_structured_entries_newest_first() {
        let dir = unique_tmpdir("log");
        init_repo(&dir);
        write(&dir, "a.txt", "a\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "first commit"]);
        write(&dir, "b.txt", "b\n");
        run_git(&dir, &["add", "-A"]);
        run_git(
            &dir,
            &["commit", "-q", "-m", "second: with a spaced subject"],
        );

        let log = commit_log(&dir, 10);
        assert_eq!(log.len(), 2, "two commits");
        // Newest first.
        assert_eq!(log[0].subject, "second: with a spaced subject");
        assert_eq!(log[1].subject, "first commit");
        // Structured fields are well-formed.
        assert_eq!(log[0].hash.len(), 40);
        assert!(log[0].hash.starts_with(&log[0].short));
        assert_eq!(log[0].author, "omniproj-test");
        assert_eq!(log[0].date.len(), 10, "YYYY-MM-DD");

        // `limit` caps the result.
        assert_eq!(commit_log(&dir, 1).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_log_is_empty_for_non_git_dir() {
        let dir = unique_tmpdir("logempty");
        assert!(commit_log(&dir, 10).is_empty());
        assert!(commit_log(&dir, 0).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_graph_captures_parents_refs_and_merges() {
        let dir = unique_tmpdir("graph");
        init_repo(&dir);
        write(&dir, "a.txt", "a\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "root"]);
        // branch, commit, merge back with a merge commit (--no-ff).
        run_git(&dir, &["checkout", "-q", "-b", "feature"]);
        write(&dir, "b.txt", "b\n");
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "feature work"]);
        run_git(&dir, &["checkout", "-q", "main"]);
        run_git(
            &dir,
            &["merge", "-q", "--no-ff", "feature", "-m", "merge feature"],
        );

        let g = commit_graph(&dir, 20);
        assert_eq!(g.len(), 3, "root + feature + merge");
        // Newest first: the merge commit has two parents and HEAD/main refs.
        let merge = &g[0];
        assert_eq!(merge.subject, "merge feature");
        assert_eq!(merge.parents.len(), 2, "merge has two parents");
        assert!(merge.refs.contains(&"HEAD".to_string()));
        assert!(merge.refs.contains(&"main".to_string()));
        // The feature branch's ref decorates its tip.
        assert!(g.iter().any(|c| c.refs.contains(&"feature".to_string())));
        // The root commit has no parents (log order isn't guaranteed root-last).
        let root = g
            .iter()
            .find(|c| c.subject == "root")
            .expect("root commit present");
        assert!(root.parents.is_empty(), "root has no parent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_graph_is_empty_for_non_git_dir() {
        let dir = unique_tmpdir("graphempty");
        assert!(commit_graph(&dir, 20).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_weeks_is_all_zero_for_non_git_dir() {
        let dir = unique_tmpdir("nogit");
        assert_eq!(commit_weeks(&dir, 16, 1_000_000_000), vec![0u32; 16]);
        assert!(commit_weeks(&dir, 0, 1_000_000_000).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
