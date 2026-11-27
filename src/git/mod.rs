pub mod graph;
pub mod patch;

use crate::cmd_log::{CommandEntry, run_cmd, run_cmd_with_env, run_cmd_with_stdin};
use anyhow::{Result, anyhow};
use chrono::{Local, TimeZone};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub use graph::{GraphRow, compute_graph};

static REBASE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    pub signature_status: char,
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
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_main: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleInfo {
    pub path: String,
    pub sha: String,
    pub status: char,
    pub describe: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoOpState {
    None,
    Merging,
    Rebasing { step: Option<usize>, total: Option<usize> },
    CherryPicking,
    Reverting,
    Bisecting,
}

#[derive(Clone, Debug)]
pub struct ReflogEntry {
    pub hash: String,
    pub short_hash: String,
    pub selector: String,
    pub action: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
pub struct BlameLine {
    pub line_no: usize,
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub timestamp: i64,
    pub summary: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct FileHistoryEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub timestamp: i64,
    pub subject: String,
    pub file_path: String,
}

#[derive(Clone, Debug)]
pub struct StashDetail {
    pub reference: String,
    pub message: String,
    pub files: Vec<FileChange>,
    pub patch: String,
}

#[derive(Clone, Debug)]
pub struct CompareDetail {
    pub base: String,
    pub head: String,
    pub files: Vec<FileChange>,
    pub patch: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebaseAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Drop,
}

impl RebaseAction {
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword => "reword",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pick => "Pick",
            Self::Reword => "Reword",
            Self::Squash => "Squash",
            Self::Fixup => "Fixup",
            Self::Drop => "Drop",
        }
    }

    pub fn cycle_next(self) -> Self {
        match self {
            Self::Pick => Self::Reword,
            Self::Reword => Self::Squash,
            Self::Squash => Self::Fixup,
            Self::Fixup => Self::Drop,
            Self::Drop => Self::Pick,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RebaseTodoItem {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub action: RebaseAction,
    pub reword_message: Option<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BisectAction {
    Good,
    Bad,
    Skip,
    Reset,
}

#[derive(Clone, Debug)]
pub struct RepoSnapshot {
    pub repo_name: String,
    pub head_branch: Option<String>,
    pub head_hash: Option<String>,
    pub github_nwo: Option<String>,
    pub op_state: RepoOpState,
    pub commits: Vec<Commit>,
    pub has_more_commits: bool,
    pub graph_rows: Vec<GraphRow>,
    pub local_branches: Vec<BranchInfo>,
    pub remote_branches: Vec<BranchInfo>,
    pub tags: Vec<TagInfo>,
    pub stashes: Vec<StashInfo>,
    pub worktrees: Vec<WorktreeInfo>,
    pub submodules: Vec<SubmoduleInfo>,
    pub remotes: Vec<RemoteInfo>,
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

    pub fn has_conflicts(&self) -> bool {
        self.unstaged_files
            .iter()
            .any(|f| f.kind == ChangeKind::Conflicted)
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

const COMMIT_LOG_FORMAT: &str = "%H%x1f%P%x1f%D%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%G?";

/// Loads the complete local repository snapshot for Git Out.
pub fn load_snapshot(
    repo_root: &Path,
    limit: usize,
    current_branch_only: bool,
) -> (Result<RepoSnapshot>, Vec<CommandEntry>) {
    let mut entries = Vec::new();

    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();

    // 1. Current HEAD hash + branch + git-dir for operation state
    let (head_hash_res, e1) = run_cmd("git", &["rev-parse", "HEAD"], repo_root);
    entries.push(e1);
    let head_hash = head_hash_res.ok().map(|s| s.trim().to_string());

    let (head_branch_res, e2) = run_cmd("git", &["symbolic-ref", "--short", "HEAD"], repo_root);
    entries.push(e2);
    let head_branch = head_branch_res.ok().map(|s| s.trim().to_string());

    let (git_dir_res, _) = run_cmd("git", &["rev-parse", "--git-dir"], repo_root);
    let op_state = git_dir_res
        .ok()
        .map(|raw| {
            let p = PathBuf::from(raw.trim());
            let abs = if p.is_absolute() {
                p
            } else {
                repo_root.join(p)
            };
            detect_op_state(&abs)
        })
        .unwrap_or(RepoOpState::None);

    // 2. Origin URL -> GitHub NWO (`owner/repo`)
    let (remote_res, e3) = run_cmd("git", &["remote", "get-url", "origin"], repo_root);
    entries.push(e3);
    let github_nwo = remote_res
        .ok()
        .and_then(|url| parse_github_nwo(url.trim()));

    // 3. Commits (fetch limit + 1 to detect whether more history exists)
    let fetch_limit = limit.saturating_add(1).to_string();
    let format_arg = format!("--format={COMMIT_LOG_FORMAT}");
    let log_args: Vec<&str> = if current_branch_only {
        vec!["log", "--topo-order", "-n", &fetch_limit, &format_arg]
    } else {
        vec!["log", "--all", "--topo-order", "-n", &fetch_limit, &format_arg]
    };
    let (log_res, e4) = run_cmd("git", &log_args, repo_root);
    entries.push(e4);

    let mut commits = match log_res {
        Ok(raw) => parse_commits(&raw, head_hash.as_deref()),
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("does not have any commits yet") || msg.contains("bad default revision") {
                Vec::new()
            } else {
                return (Err(err), entries);
            }
        }
    };
    let has_more_commits = commits.len() > limit;
    commits.truncate(limit);
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

    // 7. Worktrees, Submodules, Remotes
    let (wt_res, _) = run_cmd("git", &["worktree", "list", "--porcelain"], repo_root);
    let worktrees = wt_res
        .map(|raw| parse_worktrees(&raw))
        .unwrap_or_default();

    let (sm_res, _) = run_cmd("git", &["submodule", "status"], repo_root);
    let submodules = sm_res
        .map(|raw| parse_submodules(&raw))
        .unwrap_or_default();

    let (rm_res, _) = run_cmd("git", &["remote", "-v"], repo_root);
    let remotes = rm_res.map(|raw| parse_remotes(&raw)).unwrap_or_default();

    // 8. Working tree status
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
            op_state,
            commits,
            has_more_commits,
            graph_rows,
            local_branches,
            remote_branches,
            tags,
            stashes,
            worktrees,
            submodules,
            remotes,
            staged_files,
            unstaged_files,
        }),
        entries,
    )
}

fn detect_op_state(git_dir: &Path) -> RepoOpState {
    if git_dir.join("rebase-merge").is_dir() {
        let dir = git_dir.join("rebase-merge");
        let step = fs::read_to_string(dir.join("msgnum"))
            .ok()
            .and_then(|s| s.trim().parse().ok());
        let total = fs::read_to_string(dir.join("end"))
            .ok()
            .and_then(|s| s.trim().parse().ok());
        return RepoOpState::Rebasing { step, total };
    }
    if git_dir.join("rebase-apply").is_dir() {
        let dir = git_dir.join("rebase-apply");
        let step = fs::read_to_string(dir.join("next"))
            .ok()
            .and_then(|s| s.trim().parse().ok());
        let total = fs::read_to_string(dir.join("last"))
            .ok()
            .and_then(|s| s.trim().parse().ok());
        return RepoOpState::Rebasing { step, total };
    }
    if git_dir.join("MERGE_HEAD").exists() {
        return RepoOpState::Merging;
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return RepoOpState::CherryPicking;
    }
    if git_dir.join("REVERT_HEAD").exists() {
        return RepoOpState::Reverting;
    }
    if git_dir.join("BISECT_LOG").exists() {
        return RepoOpState::Bisecting;
    }
    RepoOpState::None
}

/// Loads full message, changed file list, and unified patch for a commit.
pub fn load_commit_detail(
    repo_root: &Path,
    hash: &str,
    ignore_whitespace: bool,
) -> (Result<CommitDetail>, Vec<CommandEntry>) {
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

    let files = parse_name_status_files(files_raw);

    let mut args = vec!["show", "--format=", "--patch", "--no-color"];
    if ignore_whitespace {
        args.push("-w");
    }
    args.push(hash);

    let (patch_res, e2) = run_cmd("git", &args, repo_root);
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
    ignore_whitespace: bool,
) -> (Result<String>, CommandEntry) {
    let mut args = vec!["show", "--format=", "--patch", "--no-color"];
    if ignore_whitespace {
        args.push("-w");
    }
    args.push(hash);
    args.push("--");
    args.push(file_path);
    run_cmd("git", &args, repo_root)
}

/// Loads unified diff for the working tree (either a specific `FileChange` or all changes).
pub fn load_worktree_diff(
    repo_root: &Path,
    selected: Option<&FileChange>,
    ignore_whitespace: bool,
) -> (Result<String>, Vec<CommandEntry>) {
    let mut entries = Vec::new();

    match selected {
        Some(file) if file.kind == ChangeKind::Untracked => {
            let patch = build_untracked_patch(repo_root, &file.path);
            (Ok(patch), entries)
        }
        Some(file) if file.staged => {
            let mut args = vec!["diff", "--cached", "--no-color"];
            if ignore_whitespace {
                args.push("-w");
            }
            args.push("--");
            args.push(&file.path);
            let (res, entry) = run_cmd("git", &args, repo_root);
            entries.push(entry);
            (res, entries)
        }
        Some(file) => {
            let mut args = vec!["diff", "--no-color"];
            if ignore_whitespace {
                args.push("-w");
            }
            args.push("--");
            args.push(&file.path);
            let (res, entry) = run_cmd("git", &args, repo_root);
            entries.push(entry);
            (res, entries)
        }
        None => {
            let mut combined = String::new();
            let mut staged_args = vec!["diff", "--cached", "--no-color"];
            if ignore_whitespace {
                staged_args.push("-w");
            }
            let (staged_res, e1) = run_cmd("git", &staged_args, repo_root);
            entries.push(e1);
            if let Ok(s) = staged_res
                && !s.trim().is_empty()
            {
                combined.push_str(&s);
                if !combined.ends_with('\n') {
                    combined.push('\n');
                }
            }
            let mut unstaged_args = vec!["diff", "--no-color"];
            if ignore_whitespace {
                unstaged_args.push("-w");
            }
            let (unstaged_res, e2) = run_cmd("git", &unstaged_args, repo_root);
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

fn parse_name_status_files(raw: &str) -> Vec<FileChange> {
    let mut files = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, '\t');
        let code = parts.next().unwrap_or("M");
        let path = parts
            .next()
            .unwrap_or("")
            .trim()
            .split('\t')
            .last()
            .unwrap_or("")
            .trim();
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
    files
}

// --- Mutating & Inspection Git commands ---

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

pub fn rename_branch(
    repo_root: &Path,
    old_name: &str,
    new_name: &str,
) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["branch", "-m", old_name, new_name], repo_root)
}

pub fn delete_branch(repo_root: &Path, name: &str, force: bool) -> (Result<String>, CommandEntry) {
    let flag = if force { "-D" } else { "-d" };
    run_cmd("git", &["branch", flag, name], repo_root)
}

pub fn delete_remote_branch(
    repo_root: &Path,
    remote_branch: &str,
) -> (Result<String>, CommandEntry) {
    if let Some((remote, branch)) = remote_branch.split_once('/') {
        run_cmd("git", &["push", remote, "--delete", branch], repo_root)
    } else {
        run_cmd("git", &["push", "origin", "--delete", remote_branch], repo_root)
    }
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

pub fn apply_patch_cached(
    repo_root: &Path,
    patch: &str,
    reverse: bool,
) -> (Result<String>, CommandEntry) {
    if reverse {
        run_cmd_with_stdin(
            "git",
            &["apply", "--cached", "--reverse", "--unidiff-zero", "-"],
            patch,
            repo_root,
        )
    } else {
        run_cmd_with_stdin(
            "git",
            &["apply", "--cached", "--unidiff-zero", "-"],
            patch,
            repo_root,
        )
    }
}

pub fn discard_patch(repo_root: &Path, patch: &str) -> (Result<String>, CommandEntry) {
    run_cmd_with_stdin(
        "git",
        &["apply", "--reverse", "--unidiff-zero", "-"],
        patch,
        repo_root,
    )
}

pub fn discard_all_unstaged(repo_root: &Path) -> (Result<String>, CommandEntry) {
    let has_tracked = run_cmd("git", &["ls-files"], repo_root)
        .0
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if has_tracked {
        let (res, entry) = run_cmd("git", &["checkout", "--", "."], repo_root);
        if res.is_err() {
            return (res, entry);
        }
    }
    run_cmd("git", &["clean", "-fd"], repo_root)
}

pub fn discard_all_changes(repo_root: &Path) -> (Result<String>, CommandEntry) {
    let has_head = run_cmd("git", &["rev-parse", "--verify", "HEAD"], repo_root)
        .0
        .is_ok();
    let (res, entry) = if has_head {
        run_cmd("git", &["reset", "--hard", "HEAD"], repo_root)
    } else {
        run_cmd(
            "git",
            &["rm", "-rf", "--cached", "--ignore-unmatch", "."],
            repo_root,
        )
    };
    if res.is_err() {
        return (res, entry);
    }
    run_cmd("git", &["clean", "-fd"], repo_root)
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
        run_cmd("git", &["checkout", "--", &file.path], repo_root)
    }
}

pub fn commit_with_opts(
    repo_root: &Path,
    message: &str,
    amend: bool,
    sign: bool,
) -> (Result<String>, CommandEntry) {
    let mut args = vec!["commit"];
    if amend {
        args.push("--amend");
    }
    if sign {
        args.push("-S");
    }
    args.push("-m");
    args.push(message);
    run_cmd("git", &args, repo_root)
}

pub fn read_head_commit_message(repo_root: &Path) -> (Result<(String, String)>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["log", "-1", "--format=%B", "HEAD"], repo_root);
    let parsed = res.map(|raw| split_commit_message(&raw));
    (parsed, entry)
}

pub fn split_commit_message(raw: &str) -> (String, String) {
    let trimmed = raw.trim();
    let mut lines = trimmed.lines();
    let summary = lines.next().unwrap_or("").trim().to_string();
    let rest = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    (summary, rest)
}

/// Soft-resets the latest commit (`HEAD~1`, or removes HEAD ref for an initial commit),
/// leaving its changes staged and returning the undone `(summary, body)`.
pub fn undo_last_commit(repo_root: &Path) -> (Result<(String, String)>, CommandEntry) {
    let (msg_res, msg_entry) = read_head_commit_message(repo_root);
    let (summary, body) = match msg_res {
        Ok(m) => m,
        Err(err) => return (Err(err), msg_entry),
    };

    let (has_parent_res, _) = run_cmd("git", &["rev-parse", "--verify", "HEAD~1"], repo_root);
    let (reset_res, reset_entry) = if has_parent_res.is_ok() {
        run_cmd("git", &["reset", "--soft", "HEAD~1"], repo_root)
    } else {
        run_cmd("git", &["update-ref", "-d", "HEAD"], repo_root)
    };

    (reset_res.map(|_| (summary, body)), reset_entry)
}

pub fn checkout_ours(repo_root: &Path, path: &str) -> (Result<String>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["checkout", "--ours", "--", path], repo_root);
    if res.is_ok() {
        return run_cmd("git", &["add", "--", path], repo_root);
    }
    (res, entry)
}

pub fn checkout_theirs(repo_root: &Path, path: &str) -> (Result<String>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["checkout", "--theirs", "--", path], repo_root);
    if res.is_ok() {
        return run_cmd("git", &["add", "--", path], repo_root);
    }
    (res, entry)
}

pub fn write_resolved_and_stage(
    repo_root: &Path,
    path: &str,
    content: &str,
) -> (Result<String>, CommandEntry) {
    let target = repo_root.join(path);
    if let Err(err) = fs::write(&target, content) {
        let entry = CommandEntry {
            program: "write".to_string(),
            args: vec![path.to_string()],
            duration_ms: 0,
            ok: false,
            stdout_preview: String::new(),
            stderr: err.to_string(),
            timestamp: Local::now(),
        };
        return (Err(err.into()), entry);
    }
    stage_file(repo_root, path)
}

pub fn fetch_all(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["fetch", "--all", "--prune", "--tags"], repo_root)
}

pub fn pull(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["pull"], repo_root)
}

pub fn pull_rebase(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["pull", "--rebase"], repo_root)
}

pub fn pull_ff_only(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["pull", "--ff-only"], repo_root)
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

pub fn push_force_with_lease(
    repo_root: &Path,
    current_branch: Option<&str>,
) -> (Result<String>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["push", "--force-with-lease"], repo_root);
    if res.is_err()
        && let Some(branch) = current_branch
    {
        let err_msg = res
            .as_ref()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        if err_msg.contains("has no upstream branch") {
            return run_cmd(
                "git",
                &["push", "--force-with-lease", "-u", "origin", branch],
                repo_root,
            );
        }
    }
    (res, entry)
}

pub fn stash_push_opts(
    repo_root: &Path,
    message: Option<&str>,
    include_untracked: bool,
    keep_index: bool,
) -> (Result<String>, CommandEntry) {
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("-u");
    }
    if keep_index {
        args.push("--keep-index");
    }
    let clean_msg = message.map(str::trim).filter(|m| !m.is_empty());
    if let Some(msg) = clean_msg {
        args.push("-m");
        args.push(msg);
    }
    run_cmd("git", &args, repo_root)
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

pub fn branch_from_stash(
    repo_root: &Path,
    name: &str,
    reference: &str,
) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["stash", "branch", name, reference], repo_root)
}

pub fn load_stash_detail(
    repo_root: &Path,
    reference: &str,
    message: &str,
) -> (Result<StashDetail>, Vec<CommandEntry>) {
    let mut entries = Vec::new();
    let (ns_res, e1) = run_cmd(
        "git",
        &["stash", "show", "--name-status", "--include-untracked", reference],
        repo_root,
    );
    entries.push(e1);
    let files = ns_res
        .map(|s| parse_name_status_files(&s))
        .unwrap_or_default();

    let (patch_res, e2) = run_cmd(
        "git",
        &["stash", "show", "-p", "--include-untracked", "--no-color", reference],
        repo_root,
    );
    entries.push(e2);
    let patch = patch_res.unwrap_or_default();

    (
        Ok(StashDetail {
            reference: reference.to_string(),
            message: message.to_string(),
            files,
            patch,
        }),
        entries,
    )
}

pub fn load_compare_detail(
    repo_root: &Path,
    base: &str,
    head: &str,
    ignore_whitespace: bool,
) -> (Result<CompareDetail>, Vec<CommandEntry>) {
    let mut entries = Vec::new();
    let range = format!("{base}..{head}");
    let (ns_res, e1) = run_cmd("git", &["diff", "--name-status", &range], repo_root);
    entries.push(e1);
    let files = match ns_res {
        Ok(raw) => parse_name_status_files(&raw),
        Err(err) => return (Err(err), entries),
    };

    let mut args = vec!["diff", "--no-color"];
    if ignore_whitespace {
        args.push("-w");
    }
    args.push(&range);
    let (patch_res, e2) = run_cmd("git", &args, repo_root);
    entries.push(e2);
    let patch = patch_res.unwrap_or_default();

    (
        Ok(CompareDetail {
            base: base.to_string(),
            head: head.to_string(),
            files,
            patch,
        }),
        entries,
    )
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

pub fn rebase_onto(repo_root: &Path, target: &str) -> (Result<String>, CommandEntry) {
    run_cmd_with_env("git", &["rebase", target], &[("GIT_EDITOR", "true")], repo_root)
}

pub fn load_rebase_commits(
    repo_root: &Path,
    base_hash: &str,
) -> (Result<Vec<RebaseTodoItem>>, CommandEntry) {
    let range = format!("{base_hash}..HEAD");
    let (res, entry) = run_cmd(
        "git",
        &["log", "--reverse", "--format=%H%x1f%s", &range],
        repo_root,
    );
    let parsed = res.map(|raw| {
        raw.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let (hash, subject) = trimmed.split_once('\x1f')?;
                let hash = hash.trim().to_string();
                let short_hash = hash.chars().take(7).collect();
                Some(RebaseTodoItem {
                    hash,
                    short_hash,
                    subject: subject.to_string(),
                    action: RebaseAction::Pick,
                    reword_message: None,
                })
            })
            .collect()
    });
    (parsed, entry)
}

/// Executes a non-interactive `git rebase -i <base_hash>` driven by `items`.
pub fn run_interactive_rebase(
    repo_root: &Path,
    base_hash: &str,
    items: &[RebaseTodoItem],
) -> (Result<String>, CommandEntry) {
    let uid = REBASE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_dir = std::env::temp_dir().join(format!(
        "gitout-rebase-{}-{uid}",
        std::process::id()
    ));

    let prep_res = (|| -> Result<(PathBuf, PathBuf)> {
        fs::create_dir_all(&tmp_dir)?;
        let todo_path = tmp_dir.join("todo.txt");
        let mut todo_buf = String::new();
        for item in items {
            todo_buf.push_str(item.action.keyword());
            todo_buf.push(' ');
            todo_buf.push_str(&item.hash);
            todo_buf.push(' ');
            todo_buf.push_str(&item.subject);
            todo_buf.push('\n');
        }
        fs::write(&todo_path, &todo_buf)?;

        let seq_script = tmp_dir.join("seq_editor.sh");
        let seq_body = format!("#!/bin/sh\ncp '{}' \"$1\"\n", shell_single_escape(&todo_path));
        fs::write(&seq_script, seq_body)?;
        fs::set_permissions(&seq_script, fs::Permissions::from_mode(0o700))?;

        let reword_msgs: Vec<String> = items
            .iter()
            .filter(|it| it.action == RebaseAction::Reword)
            .filter_map(|it| it.reword_message.as_ref().map(|m| m.trim().to_string()))
            .filter(|m| !m.is_empty())
            .collect();

        let idx_file = tmp_dir.join("msg_idx.txt");
        fs::write(&idx_file, "0")?;
        for (i, msg) in reword_msgs.iter().enumerate() {
            fs::write(tmp_dir.join(format!("msg_{i}.txt")), msg)?;
        }

        let msg_script = tmp_dir.join("msg_editor.sh");
        let msg_body = format!(
            "#!/bin/sh\nIDX_FILE='{}'\nN=$(cat \"$IDX_FILE\" 2>/dev/null || echo 0)\nMSG_FILE='{}'\"/msg_${{N}}.txt\"\nif [ -f \"$MSG_FILE\" ]; then\n  cp \"$MSG_FILE\" \"$1\"\n  echo $((N + 1)) > \"$IDX_FILE\"\nfi\nexit 0\n",
            shell_single_escape(&idx_file),
            shell_single_escape(&tmp_dir),
        );
        fs::write(&msg_script, msg_body)?;
        fs::set_permissions(&msg_script, fs::Permissions::from_mode(0o700))?;

        Ok((seq_script, msg_script))
    })();

    let out = match prep_res {
        Ok((seq_script, msg_script)) => {
            let seq_str = seq_script.to_string_lossy().to_string();
            let msg_str = msg_script.to_string_lossy().to_string();
            run_cmd_with_env(
                "git",
                &["rebase", "-i", base_hash],
                &[
                    ("GIT_SEQUENCE_EDITOR", &seq_str),
                    ("GIT_EDITOR", &msg_str),
                ],
                repo_root,
            )
        }
        Err(err) => {
            let entry = CommandEntry {
                program: "git".to_string(),
                args: vec!["rebase".to_string(), "-i".to_string(), base_hash.to_string()],
                duration_ms: 0,
                ok: false,
                stdout_preview: String::new(),
                stderr: err.to_string(),
                timestamp: Local::now(),
            };
            (Err(err), entry)
        }
    };

    let _ = fs::remove_dir_all(&tmp_dir);
    out
}

fn shell_single_escape(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "'\\''")
}

pub fn continue_op(repo_root: &Path, state: &RepoOpState) -> (Result<String>, CommandEntry) {
    let env = [("GIT_EDITOR", "true")];
    match state {
        RepoOpState::Merging => run_cmd_with_env("git", &["merge", "--continue"], &env, repo_root),
        RepoOpState::Rebasing { .. } => {
            run_cmd_with_env("git", &["rebase", "--continue"], &env, repo_root)
        }
        RepoOpState::CherryPicking => {
            run_cmd_with_env("git", &["cherry-pick", "--continue"], &env, repo_root)
        }
        RepoOpState::Reverting => {
            run_cmd_with_env("git", &["revert", "--continue"], &env, repo_root)
        }
        RepoOpState::Bisecting | RepoOpState::None => (
            Err(anyhow!("No active operation to continue")),
            CommandEntry {
                program: "git".to_string(),
                args: vec![],
                duration_ms: 0,
                ok: false,
                stdout_preview: String::new(),
                stderr: "No active operation".to_string(),
                timestamp: Local::now(),
            },
        ),
    }
}

pub fn skip_op(repo_root: &Path, state: &RepoOpState) -> (Result<String>, CommandEntry) {
    match state {
        RepoOpState::Rebasing { .. } => run_cmd("git", &["rebase", "--skip"], repo_root),
        RepoOpState::CherryPicking => run_cmd("git", &["cherry-pick", "--skip"], repo_root),
        RepoOpState::Reverting => run_cmd("git", &["revert", "--skip"], repo_root),
        _ => (
            Err(anyhow!("Skip is only valid for rebase/cherry-pick/revert")),
            CommandEntry {
                program: "git".to_string(),
                args: vec![],
                duration_ms: 0,
                ok: false,
                stdout_preview: String::new(),
                stderr: "Operation does not support --skip".to_string(),
                timestamp: Local::now(),
            },
        ),
    }
}

pub fn abort_op(repo_root: &Path, state: &RepoOpState) -> (Result<String>, CommandEntry) {
    match state {
        RepoOpState::Merging => run_cmd("git", &["merge", "--abort"], repo_root),
        RepoOpState::Rebasing { .. } => run_cmd("git", &["rebase", "--abort"], repo_root),
        RepoOpState::CherryPicking => run_cmd("git", &["cherry-pick", "--abort"], repo_root),
        RepoOpState::Reverting => run_cmd("git", &["revert", "--abort"], repo_root),
        RepoOpState::Bisecting => run_cmd("git", &["bisect", "reset"], repo_root),
        RepoOpState::None => (
            Err(anyhow!("No active operation to abort")),
            CommandEntry {
                program: "git".to_string(),
                args: vec![],
                duration_ms: 0,
                ok: false,
                stdout_preview: String::new(),
                stderr: "No active operation".to_string(),
                timestamp: Local::now(),
            },
        ),
    }
}

pub fn create_tag(
    repo_root: &Path,
    name: &str,
    target: &str,
    message: Option<&str>,
) -> (Result<String>, CommandEntry) {
    if let Some(msg) = message.map(str::trim).filter(|m| !m.is_empty()) {
        run_cmd("git", &["tag", "-a", name, target, "-m", msg], repo_root)
    } else {
        run_cmd("git", &["tag", name, target], repo_root)
    }
}

pub fn delete_tag(repo_root: &Path, name: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["tag", "-d", name], repo_root)
}

pub fn push_tag(repo_root: &Path, name: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["push", "origin", name], repo_root)
}

pub fn delete_remote_tag(repo_root: &Path, name: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["push", "origin", "--delete", name], repo_root)
}

pub fn load_reflog(repo_root: &Path, limit: usize) -> (Result<Vec<ReflogEntry>>, CommandEntry) {
    let lim = limit.to_string();
    let (res, entry) = run_cmd(
        "git",
        &["reflog", "-n", &lim, "--format=%H%x1f%gd%x1f%gs%x1f%at"],
        repo_root,
    );
    let parsed = res.map(|raw| {
        raw.lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\x1f').collect();
                if parts.len() < 4 {
                    return None;
                }
                let hash = parts[0].trim().to_string();
                let short_hash = hash.chars().take(7).collect();
                Some(ReflogEntry {
                    hash,
                    short_hash,
                    selector: parts[1].trim().to_string(),
                    action: parts[2].trim().to_string(),
                    timestamp: parts[3].trim().parse().unwrap_or(0),
                })
            })
            .collect()
    });
    (parsed, entry)
}

pub fn load_file_blame(
    repo_root: &Path,
    path: &str,
    rev: Option<&str>,
) -> (Result<Vec<BlameLine>>, CommandEntry) {
    let mut args = vec!["blame", "--line-porcelain"];
    if let Some(r) = rev {
        args.push(r);
    }
    args.push("--");
    args.push(path);

    let (res, entry) = run_cmd("git", &args, repo_root);
    let parsed = res.map(|raw| parse_blame_porcelain(&raw));
    (parsed, entry)
}

fn parse_blame_porcelain(raw: &str) -> Vec<BlameLine> {
    use std::collections::HashMap;
    struct Meta {
        author: String,
        timestamp: i64,
        summary: String,
    }

    let mut metas: HashMap<String, Meta> = HashMap::new();
    let mut out = Vec::new();
    let mut cur_hash = String::new();
    let mut cur_line_no = 0usize;
    let mut cur_author = String::new();
    let mut cur_ts = 0i64;
    let mut cur_summary = String::new();

    for line in raw.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            if let Some(prev) = metas.get(&cur_hash) {
                if cur_author.is_empty() {
                    cur_author = prev.author.clone();
                }
                if cur_ts == 0 {
                    cur_ts = prev.timestamp;
                }
                if cur_summary.is_empty() {
                    cur_summary = prev.summary.clone();
                }
            } else if !cur_hash.is_empty() {
                metas.insert(
                    cur_hash.clone(),
                    Meta {
                        author: cur_author.clone(),
                        timestamp: cur_ts,
                        summary: cur_summary.clone(),
                    },
                );
            }
            let short_hash = cur_hash.chars().take(7).collect();
            out.push(BlameLine {
                line_no: cur_line_no,
                hash: cur_hash.clone(),
                short_hash,
                author: cur_author.clone(),
                timestamp: cur_ts,
                summary: cur_summary.clone(),
                content: content.to_string(),
            });
            cur_author.clear();
            cur_ts = 0;
            cur_summary.clear();
            continue;
        }

        let mut parts = line.splitn(4, ' ');
        if let (Some(h), Some(_orig), Some(final_ln)) = (parts.next(), parts.next(), parts.next())
            && h.len() == 40
            && h.bytes().all(|b| b.is_ascii_hexdigit())
            && let Ok(ln) = final_ln.parse::<usize>()
        {
            cur_hash = h.to_string();
            cur_line_no = ln;
            if let Some(prev) = metas.get(&cur_hash) {
                cur_author = prev.author.clone();
                cur_ts = prev.timestamp;
                cur_summary = prev.summary.clone();
            }
            continue;
        }

        if let Some(a) = line.strip_prefix("author ") {
            cur_author = a.trim().to_string();
        } else if let Some(t) = line.strip_prefix("author-time ") {
            cur_ts = t.trim().parse().unwrap_or(0);
        } else if let Some(s) = line.strip_prefix("summary ") {
            cur_summary = s.to_string();
        }
    }

    out
}

pub fn load_file_history(
    repo_root: &Path,
    path: &str,
    limit: usize,
) -> (Result<Vec<FileHistoryEntry>>, CommandEntry) {
    let lim = limit.to_string();
    let (res, entry) = run_cmd(
        "git",
        &[
            "log",
            "--follow",
            "-n",
            &lim,
            "--name-only",
            "--format=%x1e%H%x1f%an%x1f%at%x1f%s",
            "--",
            path,
        ],
        repo_root,
    );
    let parsed = res.map(|raw| {
        let mut entries = Vec::new();
        for chunk in raw.split('\x1e') {
            let trimmed = chunk.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut lines = trimmed.lines();
            let header = lines.next().unwrap_or("");
            let parts: Vec<&str> = header.split('\x1f').collect();
            if parts.len() < 4 {
                continue;
            }
            let file_path = lines
                .find(|l| !l.trim().is_empty())
                .unwrap_or(path)
                .trim()
                .to_string();
            let hash = parts[0].trim().to_string();
            let short_hash = hash.chars().take(7).collect();
            entries.push(FileHistoryEntry {
                hash,
                short_hash,
                author: parts[1].trim().to_string(),
                timestamp: parts[2].trim().parse().unwrap_or(0),
                subject: parts[3].to_string(),
                file_path,
            });
        }
        entries
    });
    (parsed, entry)
}

pub fn list_tracked_files(repo_root: &Path) -> (Result<Vec<String>>, CommandEntry) {
    let (res, entry) = run_cmd("git", &["ls-files"], repo_root);
    let parsed = res.map(|raw| {
        raw.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    });
    (parsed, entry)
}

pub fn bisect_start(
    repo_root: &Path,
    bad_ref: &str,
    good_ref: &str,
) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["bisect", "start", bad_ref, good_ref], repo_root)
}

pub fn bisect_mark(repo_root: &Path, action: BisectAction) -> (Result<String>, CommandEntry) {
    let sub = match action {
        BisectAction::Good => "good",
        BisectAction::Bad => "bad",
        BisectAction::Skip => "skip",
        BisectAction::Reset => "reset",
    };
    run_cmd("git", &["bisect", sub], repo_root)
}

pub fn worktree_add(
    repo_root: &Path,
    path: &str,
    branch_or_ref: &str,
    create_branch: bool,
) -> (Result<String>, CommandEntry) {
    if create_branch {
        run_cmd(
            "git",
            &["worktree", "add", "-b", branch_or_ref, path],
            repo_root,
        )
    } else if branch_or_ref.trim().is_empty() {
        run_cmd("git", &["worktree", "add", path], repo_root)
    } else {
        run_cmd("git", &["worktree", "add", path, branch_or_ref], repo_root)
    }
}

pub fn worktree_remove(
    repo_root: &Path,
    path: &Path,
    force: bool,
) -> (Result<String>, CommandEntry) {
    let p_str = path.to_string_lossy().to_string();
    if force {
        run_cmd("git", &["worktree", "remove", "--force", &p_str], repo_root)
    } else {
        run_cmd("git", &["worktree", "remove", &p_str], repo_root)
    }
}

pub fn submodule_update(repo_root: &Path) -> (Result<String>, CommandEntry) {
    run_cmd(
        "git",
        &["submodule", "update", "--init", "--recursive"],
        repo_root,
    )
}

pub fn remote_add(repo_root: &Path, name: &str, url: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["remote", "add", name, url], repo_root)
}

pub fn remote_remove(repo_root: &Path, name: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["remote", "remove", name], repo_root)
}

pub fn search_pickaxe(
    repo_root: &Path,
    term: &str,
    limit: usize,
    head_hash: Option<&str>,
) -> (Result<Vec<Commit>>, CommandEntry) {
    let lim = limit.to_string();
    let format_arg = format!("--format={COMMIT_LOG_FORMAT}");
    let (res, entry) = run_cmd(
        "git",
        &["log", "--all", "-S", term, "-n", &lim, &format_arg],
        repo_root,
    );
    let parsed = res.map(|raw| parse_commits(&raw, head_hash));
    (parsed, entry)
}

pub fn format_commit_patch(repo_root: &Path, hash: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["format-patch", "-1", "--stdout", hash], repo_root)
}

pub fn apply_patch_file(repo_root: &Path, patch_path: &str) -> (Result<String>, CommandEntry) {
    run_cmd("git", &["apply", patch_path], repo_root)
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
        let signature_status = parts
            .get(7)
            .and_then(|s| s.trim().chars().next())
            .unwrap_or('N');
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
            signature_status,
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
        stashes.push(StashInfo { reference, message });
    }
    stashes
}

fn parse_worktrees(raw: &str) -> Vec<WorktreeInfo> {
    let mut list = Vec::new();
    for (idx, block) in raw.split("\n\n").enumerate() {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut path = PathBuf::new();
        let mut head = String::new();
        let mut branch = None;
        let mut is_bare = false;
        for line in trimmed.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = PathBuf::from(p.trim());
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                head = h.trim().chars().take(7).collect();
            } else if let Some(b) = line.strip_prefix("branch ") {
                let short = b
                    .trim()
                    .strip_prefix("refs/heads/")
                    .unwrap_or(b.trim())
                    .to_string();
                branch = Some(short);
            } else if line.trim() == "bare" {
                is_bare = true;
            }
        }
        if !path.as_os_str().is_empty() {
            list.push(WorktreeInfo {
                path,
                head,
                branch,
                is_bare,
                is_main: idx == 0,
            });
        }
    }
    list
}

fn parse_submodules(raw: &str) -> Vec<SubmoduleInfo> {
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.len() < 2 {
            continue;
        }
        let status = line.chars().next().unwrap_or(' ');
        let rest = line[1..].trim();
        let mut parts = rest.splitn(3, ' ');
        let sha = parts.next().unwrap_or("").chars().take(7).collect();
        let path = parts.next().unwrap_or("").to_string();
        let describe = parts
            .next()
            .unwrap_or("")
            .trim_matches(|c| c == '(' || c == ')')
            .to_string();
        if !path.is_empty() {
            out.push(SubmoduleInfo {
                path,
                sha,
                status,
                describe,
            });
        }
    }
    out
}

fn parse_remotes(raw: &str) -> Vec<RemoteInfo> {
    let mut out: Vec<RemoteInfo> = Vec::new();
    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(url) = parts.next() else {
            continue;
        };
        let kind = parts.next().unwrap_or("");
        if let Some(existing) = out.iter_mut().find(|r| r.name == name) {
            if kind.contains("push") {
                existing.push_url = url.to_string();
            } else {
                existing.fetch_url = url.to_string();
            }
        } else {
            let mut info = RemoteInfo {
                name: name.to_string(),
                fetch_url: String::new(),
                push_url: String::new(),
            };
            if kind.contains("push") {
                info.push_url = url.to_string();
            } else {
                info.fetch_url = url.to_string();
            }
            out.push(info);
        }
    }
    out
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

        if matches!(
            (x, y),
            ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
        ) {
            if seen_unstaged.insert(path.clone()) {
                unstaged.push(FileChange {
                    path,
                    staged: false,
                    kind: ChangeKind::Conflicted,
                });
            }
            continue;
        }

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

    fn git(dir: &Path, args: &[&str]) -> String {
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
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn init_repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Git Out Tester"]);
        git(root, &["config", "user.email", "tester@example.com"]);
        temp
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
    fn parses_conflict_status_codes_into_single_conflicted_entry() {
        let raw = "UU src/main.rs\nAA docs/a.md\nDU old.txt\n M clean.rs\n?? new.txt\n";
        let (staged, unstaged) = parse_status(raw);
        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 5);
        assert_eq!(unstaged[0].kind, ChangeKind::Conflicted);
        assert_eq!(unstaged[1].kind, ChangeKind::Conflicted);
        assert_eq!(unstaged[2].kind, ChangeKind::Conflicted);
        assert_eq!(unstaged[3].kind, ChangeKind::Modified);
        assert_eq!(unstaged[4].kind, ChangeKind::Untracked);
    }

    #[test]
    fn snapshot_and_stage_commit_workflow_in_temp_repo() {
        let temp = init_repo();
        let root = temp.path();
        git(
            root,
            &["remote", "add", "origin", "https://github.com/acme/demo.git"],
        );

        fs::write(root.join("README.md"), "# Demo\n").unwrap();
        git(root, &["add", "README.md"]);
        git(root, &["commit", "-m", "Initial commit"]);

        git(root, &["checkout", "-b", "feat/ui"]);
        fs::write(root.join("ui.txt"), "hello\n").unwrap();
        git(root, &["add", "ui.txt"]);
        git(root, &["commit", "-m", "Add UI"]);

        git(root, &["checkout", "main"]);
        git(root, &["merge", "--no-ff", "feat/ui", "-m", "Merge feat/ui"]);

        fs::write(root.join("notes.txt"), "wip note\n").unwrap();

        let (snap_res, _) = load_snapshot(root, 100, false);
        let snap = snap_res.expect("snapshot should load");
        assert_eq!(snap.head_branch.as_deref(), Some("main"));
        assert_eq!(snap.github_nwo.as_deref(), Some("acme/demo"));
        assert_eq!(snap.commits.len(), 3);
        assert_eq!(snap.graph_rows.len(), 3);
        assert_eq!(snap.remotes.len(), 1);
        assert_eq!(snap.worktrees.len(), 1);
        assert!(!snap.has_more_commits);
        assert!(snap.is_dirty());
        assert_eq!(snap.unstaged_files.len(), 1);
        assert_eq!(snap.unstaged_files[0].path, "notes.txt");
        assert_eq!(snap.unstaged_files[0].kind, ChangeKind::Untracked);

        let (res, _) = stage_file(root, "notes.txt");
        res.unwrap();
        let (res, _) = commit_with_opts(root, "Add notes", false, false);
        res.unwrap();

        let (snap2_res, _) = load_snapshot(root, 2, false);
        let snap2 = snap2_res.unwrap();
        assert_eq!(snap2.commits.len(), 2);
        assert!(snap2.has_more_commits);
        assert!(!snap2.is_dirty());
        assert_eq!(snap2.commits[0].subject, "Add notes");

        let (detail_res, _) = load_commit_detail(root, &snap2.commits[0].hash, false);
        let detail = detail_res.unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].path, "notes.txt");
        assert!(detail.patch.contains("+wip note"));
    }

    #[test]
    fn stages_and_unstages_single_hunk_and_selected_lines_via_git_apply() {
        let temp = init_repo();
        let root = temp.path();

        let base = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n";
        fs::write(root.join("nums.txt"), base).unwrap();
        git(root, &["add", "nums.txt"]);
        git(root, &["commit", "-m", "base"]);

        // Modify two well-separated regions (2 hunks)
        let edited = "1\ntwo_a\ntwo_b\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\nfourteen\n15\n";
        fs::write(root.join("nums.txt"), edited).unwrap();

        let (diff_res, _) = load_worktree_diff(
            root,
            Some(&FileChange {
                path: "nums.txt".into(),
                staged: false,
                kind: ChangeKind::Modified,
            }),
            false,
        );
        let parsed = patch::parse_unified_patch(&diff_res.unwrap());
        assert_eq!(parsed[0].hunks.len(), 2);

        // Stage ONLY Hunk 1 (second hunk: fourteen)
        let h1_patch = patch::build_hunk_patch(&parsed[0], 1).unwrap();
        apply_patch_cached(root, &h1_patch, false).0.unwrap();

        let staged_diff = git(root, &["diff", "--cached"]);
        assert!(staged_diff.contains("+fourteen"));
        assert!(!staged_diff.contains("two_a"));

        // Stage ONLY line +two_b from Hunk 0
        let (diff_res2, _) = load_worktree_diff(
            root,
            Some(&FileChange {
                path: "nums.txt".into(),
                staged: false,
                kind: ChangeKind::Modified,
            }),
            false,
        );
        let parsed2 = patch::parse_unified_patch(&diff_res2.unwrap());
        let line_patch =
            patch::build_selected_lines_patch(&parsed2[0], None, Some(&(3..=3)), false).unwrap();
        apply_patch_cached(root, &line_patch, false).0.unwrap();

        let staged_diff2 = git(root, &["diff", "--cached"]);
        assert!(staged_diff2.contains("+two_b"));
        assert!(!staged_diff2.contains("+two_a"));
        assert!(staged_diff2.contains("+fourteen"));

        // Unstage Hunk 1 (+fourteen)
        let (staged_res, _) = load_worktree_diff(
            root,
            Some(&FileChange {
                path: "nums.txt".into(),
                staged: true,
                kind: ChangeKind::Modified,
            }),
            false,
        );
        let parsed_staged = patch::parse_unified_patch(&staged_res.unwrap());
        assert_eq!(parsed_staged[0].hunks.len(), 2);
        let unstage_h1 = patch::build_hunk_patch(&parsed_staged[0], 1).unwrap();
        apply_patch_cached(root, &unstage_h1, true).0.unwrap();

        let staged_after = git(root, &["diff", "--cached"]);
        assert!(!staged_after.contains("+fourteen"));
        assert!(staged_after.contains("+two_b"));
    }

    #[test]
    fn interactive_rebase_squash_reword_drop_and_undo_last_commit() {
        let temp = init_repo();
        let root = temp.path();

        fs::write(root.join("f.txt"), "0\n").unwrap();
        git(root, &["add", "f.txt"]);
        git(root, &["commit", "-m", "c0"]);
        let base_hash = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        fs::write(root.join("f.txt"), "0\n1\n").unwrap();
        git(root, &["add", "f.txt"]);
        git(root, &["commit", "-m", "c1"]);

        fs::write(root.join("f.txt"), "0\n1\n2\n").unwrap();
        git(root, &["add", "f.txt"]);
        git(root, &["commit", "-m", "c2"]);

        fs::write(root.join("drop.txt"), "temp\n").unwrap();
        git(root, &["add", "drop.txt"]);
        git(root, &["commit", "-m", "c3_drop"]);

        let (items_res, _) = load_rebase_commits(root, &base_hash);
        let mut items = items_res.unwrap();
        assert_eq!(items.len(), 3);
        items[0].action = RebaseAction::Reword;
        items[0].reword_message = Some("reworded c1".to_string());
        items[1].action = RebaseAction::Fixup;
        items[2].action = RebaseAction::Drop;

        run_interactive_rebase(root, &base_hash, &items).0.unwrap();

        assert!(!root.join("drop.txt").exists());
        let (snap_res, _) = load_snapshot(root, 20, false);
        let snap = snap_res.unwrap();
        assert_eq!(snap.commits.len(), 2);
        assert_eq!(snap.commits[0].subject, "reworded c1");
        assert_eq!(snap.commits[1].subject, "c0");

        // Undo last commit preserves changes staged and returns previous message
        let (undone, _) = undo_last_commit(root);
        let (summary, _body) = undone.unwrap();
        assert_eq!(summary, "reworded c1");
        let (snap2, _) = load_snapshot(root, 20, false);
        let s2 = snap2.unwrap();
        assert_eq!(s2.commits.len(), 1);
        assert!(!s2.staged_files.is_empty());
    }

    #[test]
    fn blame_file_history_reflog_compare_pickaxe_and_conflict_detection() {
        let temp = init_repo();
        let root = temp.path();

        fs::write(root.join("code.rs"), "fn alpha() {}\n").unwrap();
        git(root, &["add", "code.rs"]);
        git(root, &["commit", "-m", "add alpha"]);
        let c1 = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        fs::write(root.join("code.rs"), "fn alpha() {}\nfn magic_needle() {}\n").unwrap();
        git(root, &["add", "code.rs"]);
        git(root, &["commit", "-m", "add magic needle"]);
        let c2 = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        // Blame
        let blame = load_file_blame(root, "code.rs", None).0.unwrap();
        assert_eq!(blame.len(), 2);
        assert_eq!(blame[0].hash, c1);
        assert_eq!(blame[1].hash, c2);

        // File history
        let hist = load_file_history(root, "code.rs", 10).0.unwrap();
        assert_eq!(hist.len(), 2);

        // Reflog
        let reflog = load_reflog(root, 10).0.unwrap();
        assert!(reflog.len() >= 2);

        // Compare
        let cmp = load_compare_detail(root, &c1, &c2, false).0.unwrap();
        assert_eq!(cmp.files.len(), 1);
        assert!(cmp.patch.contains("+fn magic_needle()"));

        // Pickaxe
        let pick = search_pickaxe(root, "magic_needle", 10, Some(&c2)).0.unwrap();
        assert_eq!(pick.len(), 1);
        assert_eq!(pick[0].hash, c2);

        // Tracked files
        let tracked = list_tracked_files(root).0.unwrap();
        assert_eq!(tracked, vec!["code.rs".to_string()]);

        // Create merge conflict -> detect RepoOpState::Merging
        git(root, &["checkout", "-b", "side", &c1]);
        fs::write(root.join("code.rs"), "fn alpha_side() {}\n").unwrap();
        git(root, &["add", "code.rs"]);
        git(root, &["commit", "-m", "side edit"]);
        git(root, &["checkout", "main"]);
        let _ = Command::new("git")
            .args(["merge", "side"])
            .current_dir(root)
            .output();

        let snap = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap.op_state, RepoOpState::Merging);
        assert!(snap.has_conflicts());

        abort_op(root, &snap.op_state).0.unwrap();
        let snap_clean = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap_clean.op_state, RepoOpState::None);
    }

    #[test]
    fn tags_stashes_worktrees_and_bisect_workflows() {
        let temp = init_repo();
        let root = temp.path();

        fs::write(root.join("a.txt"), "v1\n").unwrap();
        git(root, &["add", "a.txt"]);
        git(root, &["commit", "-m", "v1"]);
        let c1 = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        fs::write(root.join("a.txt"), "v2\n").unwrap();
        git(root, &["add", "a.txt"]);
        git(root, &["commit", "-m", "v2"]);
        let c2 = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        // Tags
        create_tag(root, "v0.1.0", &c1, Some("First tag")).0.unwrap();
        let snap = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap.tags.len(), 1);
        assert_eq!(snap.tags[0].name, "v0.1.0");
        delete_tag(root, "v0.1.0").0.unwrap();

        // Stash push + detail + branch from stash
        fs::write(root.join("wip.txt"), "stashed\n").unwrap();
        stash_push_opts(root, Some("my wip"), true, false).0.unwrap();
        let detail = load_stash_detail(root, "stash@{0}", "my wip").0.unwrap();
        assert!(detail.patch.contains("+stashed"));
        branch_from_stash(root, "from-stash", "stash@{0}").0.unwrap();
        git(root, &["checkout", "main"]);

        // Worktrees
        let wt_parent = TempDir::new().unwrap();
        let wt_dir = wt_parent.path().join("wt");
        let wt_str = wt_dir.to_string_lossy().to_string();
        worktree_add(root, &wt_str, "wt-branch", true).0.unwrap();
        let snap_wt = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap_wt.worktrees.len(), 2);
        worktree_remove(root, &wt_dir, false).0.unwrap();

        // Bisect state detection + reset
        bisect_start(root, &c2, &c1).0.unwrap();
        let snap_bs = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap_bs.op_state, RepoOpState::Bisecting);
        bisect_mark(root, BisectAction::Reset).0.unwrap();
        let snap_after = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap_after.op_state, RepoOpState::None);
    }

    #[test]
    fn discards_unstaged_and_all_changes_in_temp_repo() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        run_cmd("git", &["init", "-b", "main"], root).0.unwrap();
        run_cmd("git", &["config", "user.name", "Test"], root).0.unwrap();
        run_cmd("git", &["config", "user.email", "test@example.com"], root).0.unwrap();

        std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
        std::fs::write(root.join("staged_mod.txt"), "one\n").unwrap();
        stage_file(root, "tracked.txt").0.unwrap();
        stage_file(root, "staged_mod.txt").0.unwrap();
        run_cmd("git", &["commit", "-m", "init"], root).0.unwrap();

        // Create both staged and unstaged changes + an untracked file
        std::fs::write(root.join("staged_mod.txt"), "two\n").unwrap();
        stage_file(root, "staged_mod.txt").0.unwrap();
        std::fs::write(root.join("tracked.txt"), "dirty\n").unwrap();
        std::fs::write(root.join("untracked.txt"), "temp\n").unwrap();

        discard_all_unstaged(root).0.unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
            "base\n"
        );
        assert!(!root.join("untracked.txt").exists());
        // Staged change should remain intact after discard_all_unstaged
        assert_eq!(
            std::fs::read_to_string(root.join("staged_mod.txt")).unwrap(),
            "two\n"
        );
        let snap_mid = load_snapshot(root, 20, false).0.unwrap();
        assert_eq!(snap_mid.staged_files.len(), 1);
        assert!(snap_mid.unstaged_files.is_empty());

        // Now add another unstaged + untracked change and discard EVERYTHING
        std::fs::write(root.join("tracked.txt"), "dirty2\n").unwrap();
        std::fs::write(root.join("untracked2.txt"), "temp2\n").unwrap();
        discard_all_changes(root).0.unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
            "base\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("staged_mod.txt")).unwrap(),
            "one\n"
        );
        assert!(!root.join("untracked2.txt").exists());
        let snap_clean = load_snapshot(root, 20, false).0.unwrap();
        assert!(!snap_clean.is_dirty());
    }
}
