pub mod graph;

use crate::cmd_log::{CommandEntry, run_cmd};
use anyhow::Result;
use chrono::{Local, TimeZone};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub use graph::{GraphRow, compute_graph};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefKind {
    Head,
    LocalBranch,
    RemoteBranch,
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitRef {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Clone, Debug)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub refs: Vec<GitRef>,
    pub author: String,
    pub author_email: String,
    pub timestamp: i64,
    pub subject: String,
    pub is_head: bool,
}

#[derive(Clone, Debug)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub target_hash: String,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug)]
pub struct TagInfo {
    pub name: String,
    pub target_hash: String,
}

#[derive(Clone, Debug)]
pub struct StashInfo {
    pub reference: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

impl ChangeKind {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::Untracked => "?",
            Self::Conflicted => "U",
        }
    }

    fn from_status_char(ch: char) -> Self {
        match ch {
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            '?' => Self::Untracked,
            'U' => Self::Conflicted,
            _ => Self::Modified,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub staged: bool,
    pub kind: ChangeKind,
}

#[derive(Clone, Debug)]
pub struct CommitDetail {
    pub hash: String,
    pub body: String,
    pub files: Vec<FileChange>,
    pub patch: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepoSnapshot {
    pub repo_name: String,
    pub head_branch: Option<String>,
    pub head_hash: Option<String>,
    pub github_nwo: Option<String>,
    pub commits: Vec<Commit>,
    pub graph_rows: Vec<GraphRow>,
    pub local_branches: Vec<BranchInfo>,
    pub remote_branches: Vec<BranchInfo>,
    pub tags: Vec<TagInfo>,
    pub stashes: Vec<StashInfo>,
    pub staged_files: Vec<FileChange>,
    pub unstaged_files: Vec<FileChange>,
}

impl RepoSnapshot {
    pub fn head_ahead_behind(&self) -> (usize, usize) {
        self.local_branches
            .iter()
            .find(|b| b.is_head)
            .map(|b| (b.ahead, b.behind))
            .unwrap_or((0, 0))
    }

    pub fn is_dirty(&self) -> bool {
        !self.staged_files.is_empty() || !self.unstaged_files.is_empty()
    }
}

/// Resolves the root of the git repository containing `path`.
pub fn resolve_repo_root(path: &Path) -> Result<PathBuf> {
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let (res, _) = run_cmd("git", &["rev-parse", "--show-toplevel"], target);
    let out = res?;
    Ok(PathBuf::from(out.trim()))
}

/// Loads the complete local repository snapshot for Git Out.
pub fn load_snapshot(repo_root: &Path, limit: usize) -> (Result<RepoSnapshot>, Vec<CommandEntry>) {
    let mut entries = Vec::new();

    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();

    // 1. Current HEAD hash + branch
    let (head_hash_res, e1) = run_cmd("git", &["rev-parse", "HEAD"], repo_root);
    entries.push(e1);
    let head_hash = head_hash_res.ok().map(|s| s.trim().to_string());

    let (head_branch_res, e2) = run_cmd("git", &["symbolic-ref", "--short", "HEAD"], repo_root);
    entries.push(e2);
    let head_branch = head_branch_res.ok().map(|s| s.trim().to_string());

    // 2. Origin URL -> GitHub NWO (`owner/repo`)
    let (remote_res, e3) = run_cmd("git", &["remote", "get-url", "origin"], repo_root);
    entries.push(e3);
    let github_nwo = remote_res
        .ok()
        .and_then(|url| parse_github_nwo(url.trim()));

    // 3. Commits across all refs in topological order
    let limit_str = limit.to_string();
    let log_args = [
        "log",
        "--all",
        "--topo-order",
        "-n",
        &limit_str,
        "--format=%H%x1f%P%x1f%D%x1f%an%x1f%ae%x1f%at%x1f%s",
    ];
    let (log_res, e4) = run_cmd("git", &log_args, repo_root);
    entries.push(e4);

    let commits = match log_res {
        Ok(raw) => parse_commits(&raw, head_hash.as_deref()),
        Err(err) => {
            // Empty repo (no commits yet) returns an error on `git log`; handle gracefully.
            let msg = err.to_string();
            if msg.contains("does not have any commits yet") || msg.contains("bad default revision") {
                Vec::new()
            } else {
                return (Err(err), entries);
            }
        }
    };
    let graph_rows = compute_graph(&commits);

    // 4. Branches (local & remote)
    let (refs_res, e5) = run_cmd(
        "git",
        &[
            "for-each-ref",
            "--format=%(refname)%09%(HEAD)%09%(objectname)%09%(upstream:track)",
            "refs/heads",
            "refs/remotes",
        ],
        repo_root,
    );
    entries.push(e5);
    let (local_branches, remote_branches) = refs_res
        .map(|raw| parse_branches(&raw))
        .unwrap_or_default();

    // 5. Tags
    let (tags_res, e6) = run_cmd(
        "git",
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname:short)%09%(*objectname)%09%(objectname)",
            "refs/tags",
        ],
        repo_root,
    );
    entries.push(e6);
    let tags = tags_res.map(|raw| parse_tags(&raw)).unwrap_or_default();

    // 6. Stashes
    let (stash_res, e7) = run_cmd("git", &["stash", "list", "--format=%gd%x1f%gs"], repo_root);
    entries.push(e7);
    let stashes = stash_res.map(|raw| parse_stashes(&raw)).unwrap_or_default();

    // 7. Working tree status
    let (status_res, e8) = run_cmd("git", &["status", "--porcelain=v1", "-uall"], repo_root);
    entries.push(e8);
    let (staged_files, unstaged_files) = status_res
        .map(|raw| parse_status(&raw))
        .unwrap_or_default();

    (
        Ok(RepoSnapshot {
            repo_name,
            head_branch,
            head_hash,
            github_nwo,
            commits,
            graph_rows,
            local_branches,
            remote_branches,
            tags,
            stashes,
            staged_files,
            unstaged_files,
        }),
        entries,
    )
}

/// Loads full message, changed file list, and unified patch for a commit.
pub fn load_commit_detail(repo_root: &Path, hash: &str) -> (Result<CommitDetail>, Vec<CommandEntry>) {
    let mut entries = Vec::new();

    let (meta_res, e1) = run_cmd(
        "git",
        &["show", "--no-renames", "--name-status", "--format=%B%x1e", hash],
        repo_root,
    );
    entries.push(e1);
    let meta_raw = match meta_res {
        Ok(s) => s,
        Err(err) => return (Err(err), entries),
    };

    let (body, files_raw) = meta_raw
        .split_once('\x1e')
        .map(|(b, f)| (b.trim().to_string(), f))
        .unwrap_or_else(|| (meta_raw.trim().to_string(), ""));

    let mut files = Vec::new();
    for line in files_raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, '\t');
        let code = parts.next().unwrap_or("M");
        let path = parts.next().unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let kind = ChangeKind::from_status_char(code.chars().next().unwrap_or('M'));
        files.push(FileChange {
            path: path.to_string(),
            staged: false,
            kind,
        });
    }

    let (patch_res, e2) = run_cmd(
        "git",
        &["show", "--format=", "--patch", "--no-color", hash],
        repo_root,
    );
    entries.push(e2);
    let patch = patch_res.unwrap_or_default();

    (
        Ok(CommitDetail {
            hash: hash.to_string(),
            body,
            files,
            patch,
        }),
        entries,
    )
}

/// Loads unified patch for a single commit file.
pub fn load_commit_file_patch(
    repo_root: &Path,
    hash: &str,
    file_path: &str,
) -> (Result<String>, CommandEntry) {
    run_cmd(
        "git",
        &["show", "--format=", "--patch", "--no-color", hash, "--", file_path],
        repo_root,
    )
}

/// Loads unified diff for the working tree (either a specific `FileChange` or all changes).
pub fn load_worktree_diff(
    repo_root: &Path,
    selected: Option<&FileChange>,
) -> (Result<String>, Vec<CommandEntry>) {
    let mut entries = Vec::new();

    match selected {
        Some(file) if file.kind == ChangeKind::Untracked => {
            let patch = build_untracked_patch(repo_root, &file.path);
            (Ok(patch), entries)
        }
        Some(file) if file.staged => {
            let (res, entry) = run_cmd(
                "git",
                &["diff", "--cached", "--no-color", "--", &file.path],
                repo_root,
            );
            entries.push(entry);
            (res, entries)
        }
        Some(file) => {
            let (res, entry) = run_cmd(
                "git",
                &["diff", "--no-color", "--", &file.path],
                repo_root,
            );
            entries.push(entry);
            (res, entries)
        }
        None => {
            let mut combined = String::new();
            let (staged_res, e1) = run_cmd("git", &["diff", "--cached", "--no-color"], repo_root);
            entries.push(e1);
            if let Ok(s) = staged_res
                && !s.trim().is_empty()
            {
                combined.push_str(&s);
                if !combined.ends_with('\n') {
                    combined.push('\n');
                }
            }
            let (unstaged_res, e2) = run_cmd("git", &["diff", "--no-color"], repo_root);
            entries.push(e2);
            if let Ok(u) = unstaged_res
                && !u.trim().is_empty()
            {
                combined.push_str(&u);
            }
            (Ok(combined), entries)
        }
    }
}

fn build_untracked_patch(repo_root: &Path, rel_path: &str) -> String {
    let full_path = repo_root.join(rel_path);
    let Ok(bytes) = fs::read(&full_path) else {
        return String::new();
    };
    const MAX_PREVIEW_BYTES: usize = 200 * 1024;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return format!(
            "diff --git a/{rel_path} b/{rel_path}\nnew file mode 100644\nBinary file ({} bytes)\n",
            bytes.len()
        );
    }
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    let slice = if truncated {
        &bytes[..MAX_PREVIEW_BYTES]
    } else {
        &bytes[..]
    };
    let text = String::from_utf8_lossy(slice);
    let lines: Vec<&str> = text.lines().collect();
    let count = lines.len().max(1);

    let mut out = format!(
        "diff --git a/{rel_path} b/{rel_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{rel_path}\n@@ -0,0 +1,{count} @@\n"
    );
    for line in lines {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    if truncated {
        out.push_str("+... (truncated preview at 200 KB)\n");
    }
    out
}

// --- Mutating Git commands ---

pub fn checkout(repo_root: &Path, target: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["checkout", target], repo_root)
}

pub fn create_branch(
    repo_root: &Path,
    name: &str,
    start_point: Option<&str>,
    checkout_after: bool,
) -> (Result<String>, CommandEntry) {
    let mut args = if checkout_after {
        vec!["checkout", "-b", name]
    } else {
        vec!["branch", name]
    };
    if let Some(start) = start_point.filter(|s| !s.trim().is_empty()) {
        args.push(start);
    }
    run_cmd("git", &args, repo_root)
}

pub fn delete_branch(repo_root: &Path, name: &str, force: bool) -> (Result<String>, CommandEntry) {
    let flag = if force { "-D" } else { "-d" };
    run_cmd("git", &["branch", flag, name], repo_root)
}

pub fn stage_file(repo_root: &Path, path: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["add", "--", path], repo_root)
}

pub fn stage_all(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["add", "-A"], repo_root)
}

pub fn unstage_file(repo_root: &Path, path: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["restore", "--staged", "--", path], repo_root)
}

pub fn unstage_all(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["reset", "HEAD", "--"], repo_root)
}

pub fn discard_file(repo_root: &Path, file: &FileChange) -> (Result<String>, CommandEntry) {
    if file.kind == ChangeKind::Untracked {
        let target = repo_root.join(&file.path);
        let res = if target.is_dir() {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        let entry = CommandEntry {
            program: "rm".to_string(),
            args: vec![file.path.clone()],
            duration_ms: 0,
            ok: res.is_ok(),
            stdout_preview: String::new(),
            stderr: res
                .as_ref()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default(),
            timestamp: Local::now(),
        };
        (
            res.map(|()| format!("Removed {}", file.path))
                .map_err(Into::into),
            entry,
        )
    } else {
        run_cmd(
            "git",
            &["checkout", "--", &file.path],
            repo_root,
        )
    }
}

pub fn commit(repo_root: &Path, message: &str, amend: bool) -> (Result<String>, CommandEntry) {
    if amend {
        run_cmd("git", &["commit", "--amend", "-m", message], repo_root)
    } else {
        run_cmd("git", &["commit", "-m", message], repo_root)
    }
}

pub fn fetch_all(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["fetch", "--all", "--prune", "--tags"], repo_root)
}

pub fn pull(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["pull"], repo_root)
}

pub fn push(repo_root: &Path, current_branch: Option<&str>) -> (Result<String>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["push"], repo_root);
    if res.is_err()
        && let Some(branch) = current_branch
    {
        let err_msg = res
            .as_ref()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        if err_msg.contains("has no upstream branch") {
            return run_cmd("git", &["push", "-u", "origin", branch], repo_root);
        }
    }
    (res, entry)
}

pub fn stash_push(repo_root: &Path, message: Option<&str>) -> (Result<String>, CommandEntry) {
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(msg) => run_cmd("git", &["stash", "push", "-u", "-m", msg], repo_root),
        None => run_cmd("git", &["stash", "push", "-u"], repo_root),
    }
}

pub fn stash_pop(repo_root: &Path, reference: Option<&str>) -> (Result<String>, CommandEntry) {
    match reference {
        Some(r) => run_cmd("git", &["stash", "pop", r], repo_root),
        None => run_cmd("git", &["stash", "pop"], repo_root),
    }
}

pub fn stash_apply(repo_root: &Path, reference: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["stash", "apply", reference], repo_root)
}

pub fn stash_drop(repo_root: &Path, reference: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["stash", "drop", reference], repo_root)
}

pub fn cherry_pick(repo_root: &Path, hash: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["cherry-pick", hash], repo_root)
}

pub fn revert_commit(repo_root: &Path, hash: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["revert", "--no-edit", hash], repo_root)
}

pub fn reset_to(repo_root: &Path, hash: &str, mode: ResetMode) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["reset", mode.flag(), hash], repo_root)
}

pub fn merge_branch(repo_root: &Path, branch: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["merge", branch], repo_root)
}

// --- Parsing helpers ---

pub fn parse_github_nwo(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        return validate_nwo(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        return validate_nwo(rest);
    }
    if let Some(pos) = trimmed.find("github.com/") {
        let rest = &trimmed[pos + "github.com/".len()..];
        return validate_nwo(rest);
    }
    None
}

fn validate_nwo(rest: &str) -> Option<String> {
    let mut parts = rest.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn parse_commits(raw: &str, head_hash: Option<&str>) -> Vec<Commit> {
    let mut commits = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() < 7 {
            continue;
        }
        let hash = parts[0].trim().to_string();
        let short_hash: String = hash.chars().take(7).collect();
        let parents = parts[1]
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        let refs = parse_ref_decorations(parts[2]);
        let author = parts[3].trim().to_string();
        let author_email = parts[4].trim().to_string();
        let timestamp = parts[5].trim().parse::<i64>().unwrap_or(0);
        let subject = parts[6].to_string();
        let is_head = head_hash.is_some_and(|h| h == hash)
            || refs.iter().any(|r| r.kind == RefKind::Head);

        commits.push(Commit {
            hash,
            short_hash,
            parents,
            refs,
            author,
            author_email,
            timestamp,
            subject,
            is_head,
        });
    }
    commits
}

fn parse_ref_decorations(raw: &str) -> Vec<GitRef> {
    let mut refs = Vec::new();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return refs;
    }

    for part in trimmed.split(", ") {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        if let Some(branch) = item.strip_prefix("HEAD -> ") {
            refs.push(GitRef {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
            });
            refs.push(GitRef {
                name: branch.to_string(),
                kind: RefKind::LocalBranch,
            });
        } else if item == "HEAD" {
            refs.push(GitRef {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
            });
        } else if let Some(tag) = item.strip_prefix("tag: ") {
            refs.push(GitRef {
                name: tag.to_string(),
                kind: RefKind::Tag,
            });
        } else if item.ends_with("/HEAD") {
            continue;
        } else if item.contains('/')
            && !item.starts_with("feature/")
            && !item.starts_with("fix/")
            && !item.starts_with("chore/")
            && !item.starts_with("feat/")
            && !item.starts_with("docs/")
            && !item.starts_with("refactor/")
        {
            // Keep remote-looking branches distinct when ambiguous, while sidebar tracks exact refs.
            let kind = if item.starts_with("origin/") || item.starts_with("upstream/") {
                RefKind::RemoteBranch
            } else {
                RefKind::LocalBranch
            };
            refs.push(GitRef {
                name: item.to_string(),
                kind,
            });
        } else {
            refs.push(GitRef {
                name: item.to_string(),
                kind: RefKind::LocalBranch,
            });
        }
    }

    refs
}

fn parse_branches(raw: &str) -> (Vec<BranchInfo>, Vec<BranchInfo>) {
    let mut local = Vec::new();
    let mut remote = Vec::new();

    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let full_ref = cols[0].trim();
        let is_head = cols[1].trim() == "*";
        let target_hash = cols[2].trim().to_string();
        let track = cols.get(3).map(|s| s.trim()).unwrap_or("");
        let (ahead, behind) = parse_track_counts(track);

        if let Some(name) = full_ref.strip_prefix("refs/heads/") {
            local.push(BranchInfo {
                name: name.to_string(),
                is_head,
                target_hash,
                ahead,
                behind,
            });
        } else if let Some(name) = full_ref.strip_prefix("refs/remotes/") {
            if name.ends_with("/HEAD") {
                continue;
            }
            remote.push(BranchInfo {
                name: name.to_string(),
                is_head: false,
                target_hash,
                ahead: 0,
                behind: 0,
            });
        }
    }

    local.sort_by(|a, b| b.is_head.cmp(&a.is_head).then_with(|| a.name.cmp(&b.name)));
    remote.sort_by(|a, b| a.name.cmp(&b.name));
    (local, remote)
}

fn parse_track_counts(track: &str) -> (usize, usize) {
    let mut ahead = 0usize;
    let mut behind = 0usize;
    let inner = track.trim().trim_start_matches('[').trim_end_matches(']');
    for segment in inner.split(',') {
        let seg = segment.trim();
        if let Some(n) = seg.strip_prefix("ahead ") {
            ahead = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = seg.strip_prefix("behind ") {
            behind = n.trim().parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

fn parse_tags(raw: &str) -> Vec<TagInfo> {
    let mut tags = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let name = cols[0].trim().to_string();
        let peeled = cols[1].trim();
        let target = if peeled.is_empty() {
            cols[2].trim()
        } else {
            peeled
        };
        tags.push(TagInfo {
            name,
            target_hash: target.to_string(),
        });
    }
    tags
}

fn parse_stashes(raw: &str) -> Vec<StashInfo> {
    let mut stashes = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (reference, message) = line
            .split_once('\x1f')
            .map(|(r, m)| (r.trim().to_string(), m.trim().to_string()))
            .unwrap_or_else(|| (format!("stash@{{{idx}}}"), line.trim().to_string()));
        stashes.push(StashInfo {
            reference,
            message,
        });
    }
    stashes
}

fn parse_status(raw: &str) -> (Vec<FileChange>, Vec<FileChange>) {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut seen_staged = HashSet::new();
    let mut seen_unstaged = HashSet::new();

    for line in raw.lines() {
        if line.len() < 4 {
            continue;
        }
        let mut chars = line.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let rest = &line[3..];
        let path = rest
            .split(" -> ")
            .last()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"')
            .to_string();

        if x == '?' && y == '?' {
            if seen_unstaged.insert(path.clone()) {
                unstaged.push(FileChange {
                    path,
                    staged: false,
                    kind: ChangeKind::Untracked,
                });
            }
            continue;
        }

        if x != ' ' && x != '?' && seen_staged.insert(path.clone()) {
            staged.push(FileChange {
                path: path.clone(),
                staged: true,
                kind: ChangeKind::from_status_char(x),
            });
        }

        if y != ' ' && seen_unstaged.insert(path.clone()) {
            unstaged.push(FileChange {
                path,
                staged: false,
                kind: ChangeKind::from_status_char(y),
            });
        }
    }

    (staged, unstaged)
}

/// Formats a Unix timestamp as compact relative time (`just now`, `4m ago`, `2h ago`, `3d ago`, `Nov 20`).
pub fn format_relative_time(timestamp: i64) -> String {
    if timestamp <= 0 {
        return String::new();
    }
    let now = Local::now().timestamp();
    let diff = now - timestamp;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 86400 * 14 {
        format!("{}d ago", diff / 86400)
    } else if let Some(dt) = Local.timestamp_opt(timestamp, 0).single() {
        dt.format("%b %d").to_string()
    } else {
        String::new()
    }
}

pub fn format_full_date(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git failed to spawn");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn parses_github_remote_urls() {
        assert_eq!(
            parse_github_nwo("git@github.com:vovakaravov/git-out.git"),
            Some("vovakaravov/git-out".to_string())
        );
        assert_eq!(
            parse_github_nwo("https://github.com/vovakaravov/git-out.git"),
            Some("vovakaravov/git-out".to_string())
        );
        assert_eq!(
            parse_github_nwo("https://github.com/vovakaravov/git-out/"),
            Some("vovakaravov/git-out".to_string())
        );
        assert_eq!(
            parse_github_nwo("ssh://git@github.com/octocat/Hello-World.git"),
            Some("octocat/Hello-World".to_string())
        );
        assert_eq!(
            parse_github_nwo("https://gitlab.com/user/repo.git"),
            None
        );
    }

    #[test]
    fn snapshot_and_stage_commit_workflow_in_temp_repo() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Git Out Tester"]);
        git(root, &["config", "user.email", "tester@example.com"]);
        git(
            root,
            &["remote", "add", "origin", "https://github.com/acme/demo.git"],
        );

        fs::write(root.join("README.md"), "# Demo\n").unwrap();
        git(root, &["add", "README.md"]);
        git(root, &["commit", "-m", "Initial commit"]);

        // Create a feature branch and merge it back with --no-ff
        git(root, &["checkout", "-b", "feat/ui"]);
        fs::write(root.join("ui.txt"), "hello\n").unwrap();
        git(root, &["add", "ui.txt"]);
        git(root, &["commit", "-m", "Add UI"]);

        git(root, &["checkout", "main"]);
        git(root, &["merge", "--no-ff", "feat/ui", "-m", "Merge feat/ui"]);

        // Leave one untracked file
        fs::write(root.join("notes.txt"), "wip note\n").unwrap();

        let (snap_res, _) = load_snapshot(root, 100);
        let snap = snap_res.expect("snapshot should load");
        assert_eq!(snap.head_branch.as_deref(), Some("main"));
        assert_eq!(snap.github_nwo.as_deref(), Some("acme/demo"));
        assert_eq!(snap.commits.len(), 3);
        assert_eq!(snap.graph_rows.len(), 3);
        assert!(snap.is_dirty());
        assert_eq!(snap.unstaged_files.len(), 1);
        assert_eq!(snap.unstaged_files[0].path, "notes.txt");
        assert_eq!(snap.unstaged_files[0].kind, ChangeKind::Untracked);

        // Stage & commit through our functions
        let (res, _) = stage_file(root, "notes.txt");
        res.unwrap();
        let (res, _) = commit(root, "Add notes", false);
        res.unwrap();

        let (snap2_res, _) = load_snapshot(root, 100);
        let snap2 = snap2_res.unwrap();
        assert_eq!(snap2.commits.len(), 4);
        assert!(!snap2.is_dirty());
        assert_eq!(snap2.commits[0].subject, "Add notes");

        // Load commit detail for Add notes
        let (detail_res, _) = load_commit_detail(root, &snap2.commits[0].hash);
        let detail = detail_res.unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].path, "notes.txt");
        assert!(detail.patch.contains("+wip note"));
    }
}
