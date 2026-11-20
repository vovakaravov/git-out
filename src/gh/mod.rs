use crate::cmd_log::{CommandEntry, run_cmd};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    Success,
    Failure,
    Pending,
    None,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GhUser {
    pub login: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GhLabel {
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusCheckItem {
    #[serde(rename = "__typename")]
    pub kind: String,
    pub name: String,
    pub context: String,
    pub status: String,
    pub conclusion: String,
    pub state: String,
    pub details_url: String,
    pub target_url: String,
}

impl StatusCheckItem {
    pub fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else if !self.context.is_empty() {
            &self.context
        } else {
            "Check"
        }
    }

    pub fn ci_state(&self) -> CiState {
        let conc = self.conclusion.to_uppercase();
        let st = self.status.to_uppercase();
        let state = self.state.to_uppercase();

        if conc == "FAILURE"
            || conc == "TIMED_OUT"
            || conc == "CANCELLED"
            || conc == "ACTION_REQUIRED"
            || conc == "STARTUP_FAILURE"
            || state == "FAILURE"
            || state == "ERROR"
        {
            CiState::Failure
        } else if st == "IN_PROGRESS"
            || st == "QUEUED"
            || st == "PENDING"
            || st == "WAITING"
            || state == "PENDING"
        {
            CiState::Pending
        } else if conc == "SUCCESS" || conc == "NEUTRAL" || conc == "SKIPPED" || state == "SUCCESS"
        {
            CiState::Success
        } else {
            CiState::Pending
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GhComment {
    pub author: GhUser,
    pub body: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GhReview {
    pub author: GhUser,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GhPrFile {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub head_ref_name: String,
    pub base_ref_name: String,
    pub author: GhUser,
    pub review_decision: String,
    pub mergeable: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub additions: usize,
    pub deletions: usize,
    pub changed_files: usize,
    pub labels: Vec<GhLabel>,
    pub status_check_rollup: Vec<StatusCheckItem>,
}

impl PullRequest {
    pub fn ci_state(&self) -> CiState {
        if self.status_check_rollup.is_empty() {
            return CiState::None;
        }
        let mut any_pending = false;
        for item in &self.status_check_rollup {
            match item.ci_state() {
                CiState::Failure => return CiState::Failure,
                CiState::Pending => any_pending = true,
                CiState::Success | CiState::None => {}
            }
        }
        if any_pending {
            CiState::Pending
        } else {
            CiState::Success
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrDetail {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub head_ref_name: String,
    pub base_ref_name: String,
    pub body: String,
    pub author: GhUser,
    pub review_decision: String,
    pub mergeable: String,
    pub url: String,
    pub additions: usize,
    pub deletions: usize,
    pub changed_files: usize,
    pub labels: Vec<GhLabel>,
    pub status_check_rollup: Vec<StatusCheckItem>,
    pub comments: Vec<GhComment>,
    pub reviews: Vec<GhReview>,
    pub files: Vec<GhPrFile>,
    #[serde(skip)]
    pub diff_patch: String,
}

impl PrDetail {
    pub fn ci_state(&self) -> CiState {
        if self.status_check_rollup.is_empty() {
            return CiState::None;
        }
        let mut any_pending = false;
        for item in &self.status_check_rollup {
            match item.ci_state() {
                CiState::Failure => return CiState::Failure,
                CiState::Pending => any_pending = true,
                CiState::Success | CiState::None => {}
            }
        }
        if any_pending {
            CiState::Pending
        } else {
            CiState::Success
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowRun {
    pub database_id: u64,
    pub number: u64,
    pub display_title: String,
    pub name: String,
    pub workflow_name: String,
    pub head_branch: String,
    pub head_sha: String,
    pub event: String,
    pub status: String,
    pub conclusion: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

impl WorkflowRun {
    pub fn ci_state(&self) -> CiState {
        let conc = self.conclusion.to_lowercase();
        let st = self.status.to_lowercase();
        if conc == "failure"
            || conc == "timed_out"
            || conc == "cancelled"
            || conc == "startup_failure"
        {
            CiState::Failure
        } else if st == "in_progress" || st == "queued" || st == "waiting" || st == "requested" {
            CiState::Pending
        } else if conc == "success" || conc == "neutral" || conc == "skipped" {
            CiState::Success
        } else if st == "completed" {
            CiState::Success
        } else {
            CiState::Pending
        }
    }

    pub fn title_or_workflow(&self) -> &str {
        if !self.display_title.is_empty() {
            &self.display_title
        } else if !self.workflow_name.is_empty() {
            &self.workflow_name
        } else {
            &self.name
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowStep {
    pub name: String,
    pub status: String,
    pub conclusion: String,
    pub number: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowJob {
    pub database_id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: String,
    pub steps: Vec<WorkflowStep>,
}

impl WorkflowJob {
    pub fn ci_state(&self) -> CiState {
        let conc = self.conclusion.to_lowercase();
        let st = self.status.to_lowercase();
        if conc == "failure" || conc == "timed_out" || conc == "cancelled" {
            CiState::Failure
        } else if st == "in_progress" || st == "queued" {
            CiState::Pending
        } else if conc == "success" || conc == "skipped" || conc == "neutral" {
            CiState::Success
        } else {
            CiState::Pending
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunDetail {
    pub database_id: u64,
    pub number: u64,
    pub display_title: String,
    pub workflow_name: String,
    pub head_branch: String,
    pub head_sha: String,
    pub event: String,
    pub status: String,
    pub conclusion: String,
    pub url: String,
    pub jobs: Vec<WorkflowJob>,
    #[serde(skip)]
    pub failed_logs: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: GhUser,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<GhUser>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueDetail {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: String,
    pub author: GhUser,
    pub created_at: String,
    pub url: String,
    pub labels: Vec<GhLabel>,
    pub comments: Vec<GhComment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeStrategy {
    Squash,
    Merge,
    Rebase,
}

impl MergeStrategy {
    pub fn flag(self) -> &'static str {
        match self {
            Self::Squash => "--squash",
            Self::Merge => "--merge",
            Self::Rebase => "--rebase",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    Approve,
    RequestChanges,
    Comment,
}

impl ReviewAction {
    pub fn flag(self) -> &'static str {
        match self {
            Self::Approve => "--approve",
            Self::RequestChanges => "--request-changes",
            Self::Comment => "--comment",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approved",
            Self::RequestChanges => "Requested changes",
            Self::Comment => "Commented",
        }
    }
}

// --- gh CLI operations ---

pub fn list_prs(cwd: &Path, state: &str) -> (Result<Vec<PullRequest>>, CommandEntry) {
    let fields = "number,title,state,isDraft,headRefName,baseRefName,author,reviewDecision,mergeable,createdAt,updatedAt,url,additions,deletions,changedFiles,labels,statusCheckRollup";
    let (res, entry) = run_cmd(
        "gh",
        &["pr", "list", "--state", state, "--limit", "50", "--json", fields],
        cwd,
    );
    let parsed = res.and_then(|json| {
        serde_json::from_str::<Vec<PullRequest>>(&json).context("Failed to parse `gh pr list` JSON")
    });
    (parsed, entry)
}

pub fn view_pr_detail(cwd: &Path, number: u64) -> (Result<PrDetail>, Vec<CommandEntry>) {
    let mut entries = Vec::new();
    let num = number.to_string();
    let fields = "number,title,state,isDraft,headRefName,baseRefName,body,author,reviewDecision,mergeable,url,additions,deletions,changedFiles,labels,statusCheckRollup,comments,reviews,files";
    let (res, e1) = run_cmd("gh", &["pr", "view", &num, "--json", fields], cwd);
    entries.push(e1);

    let mut detail = match res.and_then(|json| {
        serde_json::from_str::<PrDetail>(&json).context("Failed to parse `gh pr view` JSON")
    }) {
        Ok(d) => d,
        Err(err) => return (Err(err), entries),
    };

    let (diff_res, e2) = run_cmd("gh", &["pr", "diff", &num, "--color", "never"], cwd);
    entries.push(e2);
    if let Ok(patch) = diff_res {
        detail.diff_patch = patch;
    }

    (Ok(detail), entries)
}

pub fn checkout_pr(cwd: &Path, number: u64) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["pr", "checkout", &num], cwd)
}

pub fn create_pr(
    cwd: &Path,
    title: &str,
    body: &str,
    base: Option<&str>,
    draft: bool,
) -> (Result<String>, CommandEntry) {
    let mut args = vec!["pr", "create", "--title", title, "--body", body];
    if let Some(b) = base.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--base");
        args.push(b);
    }
    if draft {
        args.push("--draft");
    }
    run_cmd("gh", &args, cwd)
}

pub fn merge_pr(cwd: &Path, number: u64, strategy: MergeStrategy) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["pr", "merge", &num, strategy.flag()], cwd)
}

pub fn comment_pr(cwd: &Path, number: u64, body: &str) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["pr", "comment", &num, "--body", body], cwd)
}

pub fn review_pr(
    cwd: &Path,
    number: u64,
    action: ReviewAction,
    body: &str,
) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    let trimmed = body.trim();
    let mut args = vec!["pr", "review", &num, action.flag()];
    if !trimmed.is_empty() {
        args.push("--body");
        args.push(trimmed);
    }
    run_cmd("gh", &args, cwd)
}

pub fn ready_pr(cwd: &Path, number: u64) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["pr", "ready", &num], cwd)
}

pub fn open_pr_web(cwd: &Path, number: u64) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["pr", "view", &num, "--web"], cwd)
}

pub fn list_runs(cwd: &Path) -> (Result<Vec<WorkflowRun>>, CommandEntry) {
    let fields = "databaseId,number,displayTitle,name,workflowName,headBranch,headSha,event,status,conclusion,createdAt,updatedAt,url";
    let (res, entry) = run_cmd(
        "gh",
        &["run", "list", "--limit", "40", "--json", fields],
        cwd,
    );
    let parsed = res.and_then(|json| {
        serde_json::from_str::<Vec<WorkflowRun>>(&json)
            .context("Failed to parse `gh run list` JSON")
    });
    (parsed, entry)
}

pub fn view_run_detail(cwd: &Path, id: u64) -> (Result<RunDetail>, Vec<CommandEntry>) {
    let mut entries = Vec::new();
    let id_str = id.to_string();
    let fields = "databaseId,number,displayTitle,workflowName,headBranch,headSha,event,status,conclusion,url,jobs";
    let (res, e1) = run_cmd("gh", &["run", "view", &id_str, "--json", fields], cwd);
    entries.push(e1);

    let mut detail = match res.and_then(|json| {
        serde_json::from_str::<RunDetail>(&json).context("Failed to parse `gh run view` JSON")
    }) {
        Ok(d) => d,
        Err(err) => return (Err(err), entries),
    };

    let (log_res, e2) = run_cmd("gh", &["run", "view", &id_str, "--log-failed"], cwd);
    entries.push(e2);
    detail.failed_logs = match log_res {
        Ok(s) if !s.trim().is_empty() => s,
        _ => {
            // If no failed logs (e.g. passing run), fetch concise job log summary or note.
            String::new()
        }
    };

    (Ok(detail), entries)
}

pub fn rerun_failed(cwd: &Path, id: u64) -> (Result<String>, CommandEntry) {
    let id_str = id.to_string();
    run_cmd("gh", &["run", "rerun", &id_str, "--failed"], cwd)
}

pub fn open_run_web(cwd: &Path, id: u64) -> (Result<String>, CommandEntry) {
    let id_str = id.to_string();
    run_cmd("gh", &["run", "view", &id_str, "--web"], cwd)
}

pub fn list_issues(cwd: &Path, state: &str) -> (Result<Vec<Issue>>, CommandEntry) {
    let fields = "number,title,state,author,createdAt,updatedAt,url,labels,assignees";
    let (res, entry) = run_cmd(
        "gh",
        &["issue", "list", "--state", state, "--limit", "50", "--json", fields],
        cwd,
    );
    let parsed = res.and_then(|json| {
        serde_json::from_str::<Vec<Issue>>(&json).context("Failed to parse `gh issue list` JSON")
    });
    (parsed, entry)
}

pub fn view_issue_detail(cwd: &Path, number: u64) -> (Result<IssueDetail>, CommandEntry) {
    let num = number.to_string();
    let fields = "number,title,state,body,author,createdAt,url,labels,comments";
    let (res, entry) = run_cmd("gh", &["issue", "view", &num, "--json", fields], cwd);
    let parsed = res.and_then(|json| {
        serde_json::from_str::<IssueDetail>(&json)
            .context("Failed to parse `gh issue view` JSON")
    });
    (parsed, entry)
}

pub fn create_issue(cwd: &Path, title: &str, body: &str) -> (Result<String>, CommandEntry) {
    run_cmd("gh", &["issue", "create", "--title", title, "--body", body], cwd)
}

/// Attempts `gh issue develop <number> --checkout`, falling back to a local slugged branch if offline/unavailable.
pub fn develop_issue(
    cwd: &Path,
    number: u64,
    title: &str,
) -> (Result<String>, Vec<CommandEntry>) {
    let mut entries = Vec::new();
    let num = number.to_string();
    let (res, e1) = run_cmd("gh", &["issue", "develop", &num, "--checkout"], cwd);
    entries.push(e1);
    if res.is_ok() {
        return (res, entries);
    }

    let branch_name = slugify_issue_branch(number, title);
    let (fallback_res, e2) = run_cmd("git", &["checkout", "-b", &branch_name], cwd);
    entries.push(e2);
    (fallback_res, entries)
}

pub fn open_issue_web(cwd: &Path, number: u64) -> (Result<String>, CommandEntry) {
    let num = number.to_string();
    run_cmd("gh", &["issue", "view", &num, "--web"], cwd)
}

pub fn open_repo_web(cwd: &Path) -> (Result<String>, CommandEntry) {
    run_cmd("gh", &["repo", "view", "--web"], cwd)
}

pub fn slugify_issue_branch(number: u64, title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
        if slug.len() >= 36 {
            break;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        format!("issue-{number}")
    } else {
        format!("issue-{number}-{trimmed}")
    }
}

/// Formats ISO-8601 GitHub timestamps (`2026-11-20T01:23:45Z`) cleanly into `YYYY-MM-DD` or relative time.
pub fn short_iso_date(iso: &str) -> String {
    if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        iso.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_gh_pr_list_with_check_rollup() {
        let json = r#"[
          {
            "number": 42,
            "title": "Add GPU commit graph renderer",
            "state": "OPEN",
            "isDraft": false,
            "headRefName": "feat/graph",
            "baseRefName": "main",
            "author": { "login": "vovakaravov" },
            "reviewDecision": "APPROVED",
            "mergeable": "MERGEABLE",
            "additions": 320,
            "deletions": 14,
            "changedFiles": 5,
            "labels": [{ "name": "enhancement", "color": "a2eeef", "description": "" }],
            "statusCheckRollup": [
              { "__typename": "CheckRun", "name": "cargo test", "status": "COMPLETED", "conclusion": "SUCCESS" },
              { "__typename": "StatusContext", "context": "ci/lint", "state": "FAILURE" }
            ]
          }
        ]"#;

        let prs: Vec<PullRequest> = serde_json::from_str(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].head_ref_name, "feat/graph");
        assert_eq!(prs[0].ci_state(), CiState::Failure);
    }

    #[test]
    fn deserializes_gh_run_list_and_issues() {
        let runs_json = r#"[
          {
            "databaseId": 987654,
            "number": 12,
            "displayTitle": "Fix graph edge curve",
            "workflowName": "CI",
            "headBranch": "main",
            "headSha": "abcdef1",
            "event": "push",
            "status": "completed",
            "conclusion": "success"
          }
        ]"#;
        let runs: Vec<WorkflowRun> = serde_json::from_str(runs_json).unwrap();
        assert_eq!(runs[0].ci_state(), CiState::Success);
        assert_eq!(runs[0].database_id, 987654);

        let issues_json = r#"[
          {
            "number": 7,
            "title": "Support dark mode diff split toggle",
            "state": "OPEN",
            "author": { "login": "vovakaravov" },
            "labels": [{ "name": "ui", "color": "1f6feb", "description": "UI work" }]
          }
        ]"#;
        let issues: Vec<Issue> = serde_json::from_str(issues_json).unwrap();
        assert_eq!(issues[0].number, 7);
        assert_eq!(
            slugify_issue_branch(issues[0].number, &issues[0].title),
            "issue-7-support-dark-mode-diff-split-toggle"
        );
    }

    #[test]
    fn review_action_flags_and_labels_match_gh_cli() {
        assert_eq!(ReviewAction::Approve.flag(), "--approve");
        assert_eq!(ReviewAction::RequestChanges.flag(), "--request-changes");
        assert_eq!(ReviewAction::Comment.flag(), "--comment");
        assert_eq!(ReviewAction::Approve.label(), "Approved");
        assert_eq!(ReviewAction::RequestChanges.label(), "Requested changes");
        assert_eq!(ReviewAction::Comment.label(), "Commented");
    }
}
