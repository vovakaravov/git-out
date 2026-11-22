pub mod cmd_drawer;
pub mod dialogs;
pub mod gh_actions;
pub mod gh_issues;
pub mod gh_prs;
pub mod graph_canvas;
pub mod graph_view;
pub mod inspector;
pub mod sidebar;
pub mod toolbar;

use crate::cmd_log::CommandEntry;
use crate::config::AppConfig;
use crate::gh::{
    self, GhLabel, Issue, IssueDetail, MergeStrategy, PrDetail, PullRequest, ReviewAction,
    RunDetail, WorkflowRun,
};
use crate::git::{self, CommitDetail, FileChange, RepoSnapshot, ResetMode};
use crate::theme::{contrast_fg_for_hex, gh_colors, hex_color, parse_gh_label_hex};
use crate::watcher::RepoWatcher;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::diff::{DiffFile, DiffMode, DiffParseOptions, DiffState};
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, v_flex};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement, Render, ScrollStrategy,
    Styled, UniformListScrollHandle, Window, actions, div, prelude::FluentBuilder as _, px,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

actions!(
    git_out,
    [
        RefreshAll,
        OpenRepoModal,
        NewRepoTab,
        CloseRepoTab,
        NextRepoTab,
        SwitchTab1,
        SwitchTab2,
        SwitchTab3,
        SwitchTab4,
        ToggleCmdLog
    ]
);

pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-r", RefreshAll, None),
        KeyBinding::new("f5", RefreshAll, None),
        KeyBinding::new("ctrl-o", OpenRepoModal, None),
        KeyBinding::new("ctrl-t", NewRepoTab, None),
        KeyBinding::new("ctrl-w", CloseRepoTab, None),
        KeyBinding::new("ctrl-tab", NextRepoTab, None),
        KeyBinding::new("ctrl-1", SwitchTab1, None),
        KeyBinding::new("ctrl-2", SwitchTab2, None),
        KeyBinding::new("ctrl-3", SwitchTab3, None),
        KeyBinding::new("ctrl-4", SwitchTab4, None),
        KeyBinding::new("ctrl-`", ToggleCmdLog, None),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTab {
    Graph,
    PullRequests,
    Actions,
    Issues,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteGitOp {
    Fetch,
    Pull,
    Push,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Wip,
    Commit(String),
}

const MAX_CMD_LOG: usize = 200;
const LOG_COMMIT_LIMIT: usize = 500;

#[derive(Clone)]
pub struct CachedRepoState {
    pub snapshot: Option<RepoSnapshot>,
    pub active_tab: ActiveTab,
    pub selection: Selection,
    pub gh_error: Option<String>,
    pub prs: Vec<PullRequest>,
    pub selected_pr_number: Option<u64>,
    pub selected_pr_detail: Option<PrDetail>,
    pub runs: Vec<WorkflowRun>,
    pub selected_run_id: Option<u64>,
    pub selected_run_detail: Option<RunDetail>,
    pub issues: Vec<Issue>,
    pub selected_issue_number: Option<u64>,
    pub selected_issue_detail: Option<IssueDetail>,
}

pub struct GitOutApp {
    pub window_handle: AnyWindowHandle,
    pub focus_handle: FocusHandle,
    pub config: AppConfig,

    pub open_repos: Vec<PathBuf>,
    pub repo_cache: HashMap<PathBuf, CachedRepoState>,
    pub repo_generation: u64,

    pub repo_root: Option<PathBuf>,
    pub snapshot: Option<RepoSnapshot>,
    pub watcher: Option<RepoWatcher>,

    pub active_tab: ActiveTab,
    pub selection: Selection,
    pub selected_worktree_file: Option<FileChange>,
    pub selected_commit_file: Option<String>,
    pub commit_detail: Option<CommitDetail>,

    pub diff_mode: DiffMode,
    pub inspector_diff_state: Entity<DiffState>,
    pub inspector_raw_patch: String,

    pub search_input: Entity<InputState>,
    pub commit_summary_input: Entity<InputState>,
    pub commit_body_input: Entity<TextareaState>,
    pub amend_commit: bool,

    pub graph_scroll: UniformListScrollHandle,

    // Sidebar section toggles
    pub sec_local_open: bool,
    pub sec_remotes_open: bool,
    pub sec_tags_open: bool,
    pub sec_stashes_open: bool,
    pub sec_prs_open: bool,

    // GitHub (`gh`) state
    pub gh_error: Option<String>,
    pub prs: Vec<PullRequest>,
    pub pr_state_filter: String,
    pub selected_pr_number: Option<u64>,
    pub selected_pr_detail: Option<PrDetail>,
    pub pr_show_files: bool,
    pub pr_diff_state: Entity<DiffState>,
    pub pr_comment_input: Entity<TextareaState>,

    pub runs: Vec<WorkflowRun>,
    pub selected_run_id: Option<u64>,
    pub selected_run_detail: Option<RunDetail>,

    pub issues: Vec<Issue>,
    pub issue_state_filter: String,
    pub selected_issue_number: Option<u64>,
    pub selected_issue_detail: Option<IssueDetail>,

    // Busy & command log
    pub busy_git: bool,
    pub active_remote_op: Option<RemoteGitOp>,
    #[allow(dead_code)]
    pub busy_gh: bool,
    pub show_cmd_log: bool,
    pub cmd_log: Vec<CommandEntry>,
}

impl Focusable for GitOutApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl GitOutApp {
    pub fn new(initial_repo: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = AppConfig::load();

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter commits, branches, SHAs…"));
        cx.observe(&search_input, |_, _, cx| cx.notify()).detach();

        let commit_summary_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Commit summary (required)…"));
        cx.observe(&commit_summary_input, |_, _, cx| cx.notify())
            .detach();

        let commit_body_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("Extended commit description (optional)…")
        });

        let pr_comment_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("Leave a comment (Markdown)…")
        });
        cx.observe(&pr_comment_input, |_, _, cx| cx.notify())
            .detach();

        let inspector_diff_state = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Unified));
        let pr_diff_state = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Unified));

        let restored_repos: Vec<PathBuf> = config
            .open_repos
            .iter()
            .filter(|p| p.is_dir())
            .filter_map(|p| git::resolve_repo_root(p).ok())
            .fold(Vec::new(), |mut acc, p| {
                let canon = fs::canonicalize(&p).unwrap_or(p);
                if !acc.contains(&canon) {
                    acc.push(canon);
                }
                acc
            });

        let mut app = Self {
            window_handle: window.window_handle(),
            focus_handle: cx.focus_handle(),
            config,
            open_repos: restored_repos,
            repo_cache: HashMap::new(),
            repo_generation: 0,
            repo_root: None,
            snapshot: None,
            watcher: None,
            active_tab: ActiveTab::Graph,
            selection: Selection::Wip,
            selected_worktree_file: None,
            selected_commit_file: None,
            commit_detail: None,
            diff_mode: DiffMode::Unified,
            inspector_diff_state,
            inspector_raw_patch: String::new(),
            search_input,
            commit_summary_input,
            commit_body_input,
            amend_commit: false,
            graph_scroll: UniformListScrollHandle::new(),
            sec_local_open: true,
            sec_remotes_open: true,
            sec_tags_open: false,
            sec_stashes_open: true,
            sec_prs_open: true,
            gh_error: None,
            prs: Vec::new(),
            pr_state_filter: "open".to_string(),
            selected_pr_number: None,
            selected_pr_detail: None,
            pr_show_files: false,
            pr_diff_state,
            pr_comment_input,
            runs: Vec::new(),
            selected_run_id: None,
            selected_run_detail: None,
            issues: Vec::new(),
            issue_state_filter: "open".to_string(),
            selected_issue_number: None,
            selected_issue_detail: None,
            busy_git: false,
            active_remote_op: None,
            busy_gh: false,
            show_cmd_log: false,
            cmd_log: Vec::new(),
        };

        // Resolve initial repository (CLI arg -> cwd -> config.last_repo -> first restored tab).
        let candidate = initial_repo
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|d| git::resolve_repo_root(&d).ok())
            })
            .or_else(|| app.config.last_repo.clone())
            .or_else(|| app.open_repos.first().cloned());

        if let Some(path) = candidate {
            app.open_repository(path, cx);
        }

        // Start lightweight .git watcher poller so external terminal / agent actions auto-refresh.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let should_refresh = this
                    .update(cx, |app, _| {
                        app.watcher
                            .as_mut()
                            .is_some_and(RepoWatcher::poll_changed)
                    })
                    .unwrap_or(false);
                if should_refresh {
                    let _ = this.update(cx, |app, cx| {
                        if !app.busy_git {
                            app.refresh_snapshot_only(cx);
                        }
                    });
                }
            }
        })
        .detach();

        app
    }

    pub fn push_cmds(&mut self, entries: Vec<CommandEntry>) {
        for e in entries.into_iter().rev() {
            self.cmd_log.insert(0, e);
        }
        self.cmd_log.truncate(MAX_CMD_LOG);
    }

    pub fn push_cmd(&mut self, entry: CommandEntry) {
        self.cmd_log.insert(0, entry);
        self.cmd_log.truncate(MAX_CMD_LOG);
    }

    pub fn toast_ok(&self, msg: impl Into<String>, cx: &mut App) {
        let text = msg.into();
        let _ = self.window_handle.update(cx, |_, window, cx| {
            window.push_notification(Notification::success(text), cx);
        });
    }

    pub fn toast_err(&self, msg: impl Into<String>, cx: &mut App) {
        let text = msg.into();
        let _ = self.window_handle.update(cx, |_, window, cx| {
            window.push_notification(Notification::error(text), cx);
        });
    }

    fn stash_active_repo_cache(&mut self) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        self.repo_cache.insert(
            root,
            CachedRepoState {
                snapshot: self.snapshot.clone(),
                active_tab: self.active_tab,
                selection: self.selection.clone(),
                gh_error: self.gh_error.clone(),
                prs: self.prs.clone(),
                selected_pr_number: self.selected_pr_number,
                selected_pr_detail: self.selected_pr_detail.clone(),
                runs: self.runs.clone(),
                selected_run_id: self.selected_run_id,
                selected_run_detail: self.selected_run_detail.clone(),
                issues: self.issues.clone(),
                selected_issue_number: self.selected_issue_number,
                selected_issue_detail: self.selected_issue_detail.clone(),
            },
        );
    }

    fn clear_active_repo_state(&mut self, cx: &mut Context<Self>) {
        self.repo_generation = self.repo_generation.wrapping_add(1);
        self.repo_root = None;
        self.snapshot = None;
        self.watcher = None;
        self.active_tab = ActiveTab::Graph;
        self.selection = Selection::Wip;
        self.selected_worktree_file = None;
        self.selected_commit_file = None;
        self.commit_detail = None;
        self.inspector_raw_patch.clear();
        self.inspector_diff_state
            .update(cx, |s, cx| s.set_files([], cx));
        self.gh_error = None;
        self.prs.clear();
        self.runs.clear();
        self.issues.clear();
        self.selected_pr_number = None;
        self.selected_pr_detail = None;
        self.pr_diff_state
            .update(cx, |s, cx| s.set_files([], cx));
        self.selected_run_id = None;
        self.selected_run_detail = None;
        self.selected_issue_number = None;
        self.selected_issue_detail = None;
        self.busy_git = false;
        self.active_remote_op = None;
        self.busy_gh = false;
    }

    fn activate_repo(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if self.repo_root.as_ref() == Some(&root) {
            return;
        }
        self.stash_active_repo_cache();
        self.repo_generation = self.repo_generation.wrapping_add(1);
        self.busy_git = false;
        self.active_remote_op = None;
        self.busy_gh = false;
        self.selected_worktree_file = None;
        self.selected_commit_file = None;
        self.commit_detail = None;
        self.inspector_raw_patch.clear();
        self.inspector_diff_state
            .update(cx, |s, cx| s.set_files([], cx));

        self.watcher = RepoWatcher::start(&root).ok();
        self.repo_root = Some(root.clone());

        if let Some(cached) = self.repo_cache.get(&root).cloned() {
            self.snapshot = cached.snapshot;
            self.active_tab = cached.active_tab;
            self.selection = cached.selection;
            self.gh_error = cached.gh_error;
            self.prs = cached.prs;
            self.selected_pr_number = cached.selected_pr_number;
            self.selected_pr_detail = cached.selected_pr_detail;
            self.runs = cached.runs;
            self.selected_run_id = cached.selected_run_id;
            self.selected_run_detail = cached.selected_run_detail;
            self.issues = cached.issues;
            self.selected_issue_number = cached.selected_issue_number;
            self.selected_issue_detail = cached.selected_issue_detail;
            let pr_files = self
                .selected_pr_detail
                .as_ref()
                .map(|d| parse_diff_lenient(&d.diff_patch))
                .unwrap_or_default();
            self.pr_diff_state
                .update(cx, |s, cx| s.set_files(pr_files, cx));
        } else {
            self.snapshot = None;
            self.active_tab = ActiveTab::Graph;
            self.selection = Selection::Wip;
            self.gh_error = None;
            self.prs.clear();
            self.runs.clear();
            self.issues.clear();
            self.selected_pr_number = None;
            self.selected_pr_detail = None;
            self.pr_diff_state
                .update(cx, |s, cx| s.set_files([], cx));
            self.selected_run_id = None;
            self.selected_run_detail = None;
            self.selected_issue_number = None;
            self.selected_issue_detail = None;
        }

        self.config
            .save_open_repos(&self.open_repos, self.repo_root.as_deref());
        self.refresh_all(cx);
        cx.notify();
    }

    pub fn open_repository(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match git::resolve_repo_root(&path) {
            Ok(root) => {
                let canonical = fs::canonicalize(&root).unwrap_or(root);
                if !self.open_repos.contains(&canonical) {
                    self.open_repos.push(canonical.clone());
                }
                self.config.record_repo(&canonical);
                if self.repo_root.as_ref() == Some(&canonical) {
                    self.config
                        .save_open_repos(&self.open_repos, self.repo_root.as_deref());
                    self.refresh_all(cx);
                } else {
                    self.activate_repo(canonical, cx);
                }
            }
            Err(err) => {
                self.toast_err(format!("Not a git repository: {err}"), cx);
            }
        }
    }

    pub fn switch_to_repo_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(target) = self.open_repos.get(idx).cloned() {
            self.activate_repo(target, cx);
        }
    }

    pub fn next_repo_tab(&mut self, cx: &mut Context<Self>) {
        if self.open_repos.len() <= 1 {
            return;
        }
        let cur = self
            .repo_root
            .as_ref()
            .and_then(|r| self.open_repos.iter().position(|p| p == r))
            .unwrap_or(0);
        let next = (cur + 1) % self.open_repos.len();
        self.switch_to_repo_tab(next, cx);
    }

    pub fn close_repo_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.open_repos.len() {
            return;
        }
        let removed = self.open_repos.remove(idx);
        self.repo_cache.remove(&removed);

        if self.repo_root.as_ref() == Some(&removed) {
            self.clear_active_repo_state(cx);
            if !self.open_repos.is_empty() {
                let next_idx = idx.min(self.open_repos.len() - 1);
                let next_repo = self.open_repos[next_idx].clone();
                self.activate_repo(next_repo, cx);
                return;
            }
        }
        self.config
            .save_open_repos(&self.open_repos, self.repo_root.as_deref());
        cx.notify();
    }

    pub fn reorder_repo_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if reorder_vec(&mut self.open_repos, from, to) {
            self.config
                .save_open_repos(&self.open_repos, self.repo_root.as_deref());
            cx.notify();
        }
    }

    pub fn close_current_repo_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(cur_idx) = self
            .repo_root
            .as_ref()
            .and_then(|r| self.open_repos.iter().position(|p| p == r))
        {
            self.close_repo_tab(cur_idx, cx);
        }
    }

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        self.refresh_snapshot_inner(true, cx);
    }

    pub fn refresh_snapshot_only(&mut self, cx: &mut Context<Self>) {
        self.refresh_snapshot_inner(false, cx);
    }

    fn refresh_snapshot_inner(&mut self, also_gh: bool, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_git = true;

        cx.spawn(async move |this, cx| {
            let (snap_res, entries) = cx
                .background_executor()
                .spawn(async move { git::load_snapshot(&root, LOG_COMMIT_LIMIT) })
                .await;

            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_git = false;
                match snap_res {
                    Ok(snap) => {
                        let dirty = snap.is_dirty();
                        let has_github = snap.github_nwo.is_some();
                        let first_hash = snap.commits.first().map(|c| c.hash.clone());
                        app.snapshot = Some(snap);

                        // Refresh active selection details
                        match app.selection.clone() {
                            Selection::Wip => {
                                if !dirty && matches!(app.selected_worktree_file, Some(_)) {
                                    app.selected_worktree_file = None;
                                }
                                app.load_worktree_diff(cx);
                            }
                            Selection::Commit(h) => {
                                let exists = app
                                    .snapshot
                                    .as_ref()
                                    .is_some_and(|s| s.commits.iter().any(|c| c.hash == h));
                                if exists {
                                    app.load_commit_detail(h, cx);
                                } else if let Some(fh) = first_hash {
                                    app.selection = Selection::Commit(fh.clone());
                                    app.load_commit_detail(fh, cx);
                                }
                            }
                        }

                        if also_gh && has_github {
                            app.refresh_gh_all(cx);
                        }
                    }
                    Err(err) => {
                        app.toast_err(format!("Failed to load repo: {err}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn switch_tab(&mut self, tab: ActiveTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        let has_github = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.github_nwo.is_some());
        if has_github {
            match tab {
                ActiveTab::PullRequests if self.prs.is_empty() => self.refresh_gh_prs(cx),
                ActiveTab::Actions if self.runs.is_empty() => self.refresh_gh_runs(cx),
                ActiveTab::Issues if self.issues.is_empty() => self.refresh_gh_issues(cx),
                _ => {}
            }
        }
        cx.notify();
    }

    pub fn jump_to_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        let Some(snap) = self.snapshot.as_ref() else {
            return;
        };
        if let Some(pos) = snap.commits.iter().position(|c| c.hash == hash) {
            self.active_tab = ActiveTab::Graph;
            self.graph_scroll.scroll_to_item(pos, ScrollStrategy::Center);
            self.select_commit(hash.to_string(), cx);
        }
    }

    pub fn select_wip(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::Wip;
        self.selected_worktree_file = None;
        self.load_worktree_diff(cx);
        cx.notify();
    }

    pub fn select_worktree_file(&mut self, file: Option<FileChange>, cx: &mut Context<Self>) {
        self.selected_worktree_file = file;
        self.load_worktree_diff(cx);
        cx.notify();
    }

    pub fn select_commit(&mut self, hash: String, cx: &mut Context<Self>) {
        self.selection = Selection::Commit(hash.clone());
        self.selected_commit_file = None;
        self.load_commit_detail(hash, cx);
        cx.notify();
    }

    pub fn select_commit_file(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.selected_commit_file = path.clone();
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let Selection::Commit(hash) = self.selection.clone() else {
            return;
        };

        if let Some(file_path) = path {
            let repo_gen = self.repo_generation;
            cx.spawn(async move |this, cx| {
                let (res, entry) = cx
                    .background_executor()
                    .spawn(async move { git::load_commit_file_patch(&root, &hash, &file_path) })
                    .await;
                let _ = this.update(cx, |app, cx| {
                    app.push_cmd(entry);
                    if app.repo_generation != repo_gen {
                        return;
                    }
                    if let Ok(patch) = res {
                        app.apply_inspector_patch(patch, cx);
                    }
                    cx.notify();
                });
            })
            .detach();
        } else if let Some(detail) = self.commit_detail.as_ref() {
            let patch = detail.patch.clone();
            self.apply_inspector_patch(patch, cx);
        }
    }

    fn load_commit_detail(&mut self, hash: String, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        cx.spawn(async move |this, cx| {
            let (res, entries) = cx
                .background_executor()
                .spawn(async move { git::load_commit_detail(&root, &hash) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                if app.repo_generation != repo_gen {
                    return;
                }
                match res {
                    Ok(detail) => {
                        let patch = detail.patch.clone();
                        app.commit_detail = Some(detail);
                        app.apply_inspector_patch(patch, cx);
                    }
                    Err(err) => {
                        app.toast_err(format!("Failed to load commit diff: {err}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_worktree_diff(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        let selected = self.selected_worktree_file.clone();
        cx.spawn(async move |this, cx| {
            let (res, entries) = cx
                .background_executor()
                .spawn(async move { git::load_worktree_diff(&root, selected.as_ref()) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                if app.repo_generation != repo_gen {
                    return;
                }
                if let Ok(patch) = res {
                    app.apply_inspector_patch(patch, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_inspector_patch(&mut self, patch: String, cx: &mut Context<Self>) {
        self.inspector_raw_patch = patch.clone();
        let files = parse_diff_lenient(&patch);
        self.inspector_diff_state
            .update(cx, |state, cx| state.set_files(files, cx));
    }

    pub fn set_diff_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        self.diff_mode = mode;
        self.inspector_diff_state
            .update(cx, |state, cx| state.set_mode(mode, cx));
        self.pr_diff_state
            .update(cx, |state, cx| state.set_mode(mode, cx));
        cx.notify();
    }

    // --- Mutating Git actions ---

    fn run_git_action<F>(&mut self, success_msg: Option<&'static str>, cx: &mut Context<Self>, op: F)
    where
        F: FnOnce(&PathBuf) -> (anyhow::Result<String>, CommandEntry) + Send + 'static,
    {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_git = true;

        cx.spawn(async move |this, cx| {
            let (res, entry) = cx.background_executor().spawn(async move { op(&root) }).await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_git = false;
                match res {
                    Ok(out) => {
                        if let Some(msg) = success_msg {
                            app.toast_ok(msg, cx);
                        } else if !out.trim().is_empty() {
                            let first_line = out.lines().next().unwrap_or("Done");
                            app.toast_ok(first_line, cx);
                        }
                        app.refresh_snapshot_only(cx);
                        if app.show_cmd_log {
                            cx.notify();
                        }
                    }
                    Err(err) => {
                        app.toast_err(err.to_string(), cx);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn run_remote_git_action<F>(
        &mut self,
        op_kind: RemoteGitOp,
        success_msg: Option<&'static str>,
        cx: &mut Context<Self>,
        op: F,
    ) where
        F: FnOnce(&PathBuf) -> (anyhow::Result<String>, CommandEntry) + Send + 'static,
    {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_git = true;
        self.active_remote_op = Some(op_kind);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (res, entry) = cx.background_executor().spawn(async move { op(&root) }).await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_git = false;
                app.active_remote_op = None;
                match res {
                    Ok(out) => {
                        if let Some(msg) = success_msg {
                            app.toast_ok(msg, cx);
                        } else if !out.trim().is_empty() {
                            let first_line = out.lines().next().unwrap_or("Done");
                            app.toast_ok(first_line, cx);
                        }
                        app.refresh_snapshot_only(cx);
                    }
                    Err(err) => {
                        app.toast_err(err.to_string(), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn fetch_all(&mut self, cx: &mut Context<Self>) {
        self.run_remote_git_action(RemoteGitOp::Fetch, Some("Fetched all remotes"), cx, |r| {
            git::fetch_all(r)
        });
    }

    pub fn pull(&mut self, cx: &mut Context<Self>) {
        self.run_remote_git_action(RemoteGitOp::Pull, Some("Pulled latest changes"), cx, |r| {
            git::pull(r)
        });
    }

    pub fn push(&mut self, cx: &mut Context<Self>) {
        let branch = self.snapshot.as_ref().and_then(|s| s.head_branch.clone());
        self.run_remote_git_action(RemoteGitOp::Push, Some("Pushed to remote"), cx, move |r| {
            git::push(r, branch.as_deref())
        });
    }

    pub fn checkout_target(&mut self, target: String, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, move |r| git::checkout(r, &target));
    }

    pub fn create_branch(
        &mut self,
        name: String,
        start_point: Option<String>,
        checkout_after: bool,
        cx: &mut Context<Self>,
    ) {
        self.run_git_action(None, cx, move |r| {
            git::create_branch(r, &name, start_point.as_deref(), checkout_after)
        });
    }

    pub fn delete_branch(&mut self, name: String, force: bool, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, move |r| git::delete_branch(r, &name, force));
    }

    pub fn merge_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, move |r| git::merge_branch(r, &name));
    }

    pub fn stage_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, move |r| git::stage_file(r, &path));
    }

    pub fn stage_all(&mut self, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, |r| git::stage_all(r));
    }

    pub fn unstage_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, move |r| git::unstage_file(r, &path));
    }

    pub fn unstage_all(&mut self, cx: &mut Context<Self>) {
        self.run_git_action(None, cx, |r| git::unstage_all(r));
    }

    pub fn discard_file(&mut self, file: FileChange, cx: &mut Context<Self>) {
        self.run_git_action(Some("Discarded file changes"), cx, move |r| {
            git::discard_file(r, &file)
        });
    }

    pub fn submit_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let summary = self.commit_summary_input.read(cx).value().trim().to_string();
        if summary.is_empty() {
            return;
        }
        let body = self.commit_body_input.read(cx).value().trim().to_string();
        let message = if body.is_empty() {
            summary
        } else {
            format!("{summary}\n\n{body}")
        };
        let amend = self.amend_commit;

        self.commit_summary_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.commit_body_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.amend_commit = false;
        self.selected_worktree_file = None;

        self.run_git_action(Some("Created commit"), cx, move |r| {
            git::commit(r, &message, amend)
        });
    }

    pub fn stash_push(&mut self, message: Option<String>, cx: &mut Context<Self>) {
        self.run_git_action(Some("Stashed working changes"), cx, move |r| {
            git::stash_push(r, message.as_deref())
        });
    }

    pub fn stash_pop(&mut self, reference: Option<String>, cx: &mut Context<Self>) {
        self.run_git_action(Some("Popped stash"), cx, move |r| {
            git::stash_pop(r, reference.as_deref())
        });
    }

    pub fn stash_apply(&mut self, reference: String, cx: &mut Context<Self>) {
        self.run_git_action(Some("Applied stash"), cx, move |r| {
            git::stash_apply(r, &reference)
        });
    }

    pub fn stash_drop(&mut self, reference: String, cx: &mut Context<Self>) {
        self.run_git_action(Some("Dropped stash"), cx, move |r| {
            git::stash_drop(r, &reference)
        });
    }

    pub fn cherry_pick(&mut self, hash: String, cx: &mut Context<Self>) {
        self.run_git_action(Some("Cherry-picked commit"), cx, move |r| {
            git::cherry_pick(r, &hash)
        });
    }

    pub fn revert_commit(&mut self, hash: String, cx: &mut Context<Self>) {
        self.run_git_action(Some("Reverted commit"), cx, move |r| {
            git::revert_commit(r, &hash)
        });
    }

    pub fn reset_to(&mut self, hash: String, mode: ResetMode, cx: &mut Context<Self>) {
        self.run_git_action(Some("Reset branch"), cx, move |r| {
            git::reset_to(r, &hash, mode)
        });
    }

    // --- GitHub (`gh`) background loaders & actions ---

    pub fn refresh_gh_all(&mut self, cx: &mut Context<Self>) {
        self.refresh_gh_prs(cx);
        self.refresh_gh_runs(cx);
        self.refresh_gh_issues(cx);
    }

    pub fn refresh_gh_prs(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        let state = self.pr_state_filter.clone();
        self.busy_gh = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::list_prs(&root, &state) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_gh = false;
                match res {
                    Ok(prs) => {
                        app.gh_error = None;
                        let first_num = prs.first().map(|p| p.number);
                        app.prs = prs;
                        if app.selected_pr_number.is_none()
                            && let Some(num) = first_num
                        {
                            app.select_pr(num, cx);
                        }
                    }
                    Err(err) => {
                        app.gh_error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_pr(&mut self, number: u64, cx: &mut Context<Self>) {
        self.selected_pr_number = Some(number);
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_gh = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (res, entries) = cx
                .background_executor()
                .spawn(async move { gh::view_pr_detail(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_gh = false;
                match res {
                    Ok(detail) => {
                        let files = parse_diff_lenient(&detail.diff_patch);
                        app.pr_diff_state
                            .update(cx, |s, cx| s.set_files(files, cx));
                        app.selected_pr_detail = Some(detail);
                    }
                    Err(err) => {
                        app.toast_err(format!("gh pr view failed: {err}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn checkout_pr(&mut self, number: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        self.busy_gh = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::checkout_pr(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.busy_gh = false;
                app.push_cmd(entry);
                match res {
                    Ok(_) => {
                        app.toast_ok(format!("Checked out PR #{number}"), cx);
                        app.refresh_snapshot_only(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn create_pr(
        &mut self,
        title: String,
        body: String,
        base: Option<String>,
        draft: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        self.busy_gh = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move {
                    gh::create_pr(&root, &title, &body, base.as_deref(), draft)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.busy_gh = false;
                app.push_cmd(entry);
                match res {
                    Ok(_) => {
                        app.toast_ok("Created Pull Request", cx);
                        app.refresh_gh_prs(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn merge_pr(&mut self, number: u64, strategy: MergeStrategy, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        self.busy_gh = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::merge_pr(&root, number, strategy) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.busy_gh = false;
                app.push_cmd(entry);
                match res {
                    Ok(_) => {
                        app.toast_ok(format!("Merged PR #{number}"), cx);
                        app.refresh_all(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn submit_pr_comment(
        &mut self,
        number: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = self.pr_comment_input.read(cx).value().trim().to_string();
        if body.is_empty() {
            return;
        }
        self.pr_comment_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::comment_pr(&root, number, &body) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                match res {
                    Ok(_) => {
                        app.toast_ok("Posted comment", cx);
                        app.select_pr(number, cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
            });
        })
        .detach();
    }

    pub fn review_pr(
        &mut self,
        number: u64,
        action: ReviewAction,
        body: String,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_gh = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::review_pr(&root, number, action, &body) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_gh = false;
                match res {
                    Ok(_) => {
                        app.toast_ok(format!("{} PR #{number}", action.label()), cx);
                        app.refresh_gh_prs(cx);
                        app.select_pr(number, cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn ready_pr(&mut self, number: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        self.busy_gh = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::ready_pr(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                app.busy_gh = false;
                match res {
                    Ok(_) => {
                        app.toast_ok(format!("Marked PR #{number} ready for review"), cx);
                        app.refresh_gh_prs(cx);
                        app.select_pr(number, cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_pr_web(&mut self, number: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::open_pr_web(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if let Err(err) = res {
                    app.toast_err(err.to_string(), cx);
                }
            });
        })
        .detach();
    }

    pub fn refresh_gh_runs(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::list_runs(&root) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                if let Ok(runs) = res {
                    let first_id = runs.first().map(|r| r.database_id);
                    app.runs = runs;
                    if app.selected_run_id.is_none()
                        && let Some(id) = first_id
                    {
                        app.select_run(id, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_run(&mut self, id: u64, cx: &mut Context<Self>) {
        self.selected_run_id = Some(id);
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        cx.spawn(async move |this, cx| {
            let (res, entries) = cx
                .background_executor()
                .spawn(async move { gh::view_run_detail(&root, id) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                if app.repo_generation != repo_gen {
                    return;
                }
                match res {
                    Ok(detail) => app.selected_run_detail = Some(detail),
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn rerun_failed_run(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::rerun_failed(&root, id) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                match res {
                    Ok(_) => {
                        app.toast_ok("Requested rerun of failed jobs", cx);
                        app.refresh_gh_runs(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
            });
        })
        .detach();
    }

    pub fn open_run_web(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::open_run_web(&root, id) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if let Err(err) = res {
                    app.toast_err(err.to_string(), cx);
                }
            });
        })
        .detach();
    }

    pub fn refresh_gh_issues(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        let state = self.issue_state_filter.clone();
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::list_issues(&root, &state) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                if let Ok(issues) = res {
                    let first_num = issues.first().map(|i| i.number);
                    app.issues = issues;
                    if app.selected_issue_number.is_none()
                        && let Some(num) = first_num
                    {
                        app.select_issue(num, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_issue(&mut self, number: u64, cx: &mut Context<Self>) {
        self.selected_issue_number = Some(number);
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        let repo_gen = self.repo_generation;
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::view_issue_detail(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if app.repo_generation != repo_gen {
                    return;
                }
                match res {
                    Ok(detail) => app.selected_issue_detail = Some(detail),
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn create_issue(&mut self, title: String, body: String, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::create_issue(&root, &title, &body) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                match res {
                    Ok(_) => {
                        app.toast_ok("Created issue", cx);
                        app.refresh_gh_issues(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
            });
        })
        .detach();
    }

    pub fn develop_issue(&mut self, number: u64, title: String, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entries) = cx
                .background_executor()
                .spawn(async move { gh::develop_issue(&root, number, &title) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmds(entries);
                match res {
                    Ok(_) => {
                        app.toast_ok(format!("Created branch for #{number}"), cx);
                        app.active_tab = ActiveTab::Graph;
                        app.refresh_snapshot_only(cx);
                    }
                    Err(err) => app.toast_err(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_issue_web(&mut self, number: u64, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::open_issue_web(&root, number) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if let Err(err) = res {
                    app.toast_err(err.to_string(), cx);
                }
            });
        })
        .detach();
    }

    pub fn open_repo_web(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.repo_root.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (res, entry) = cx
                .background_executor()
                .spawn(async move { gh::open_repo_web(&root) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.push_cmd(entry);
                if let Err(err) = res {
                    app.toast_err(err.to_string(), cx);
                }
            });
        })
        .detach();
    }
}

impl Render for GitOutApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = hex_color(gh_colors::CANVAS_DEFAULT);
        let fg = hex_color(gh_colors::FG);
        let app_for_open = cx.entity();
        let app_for_new_tab = cx.entity();

        v_flex()
            .id("git-out-root")
            .size_full()
            .bg(bg)
            .text_color(fg)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &RefreshAll, _, cx| {
                this.refresh_all(cx);
            }))
            .on_action(move |_: &OpenRepoModal, window, cx| {
                dialogs::open_repo_dialog(app_for_open.clone(), window, cx);
            })
            .on_action(move |_: &NewRepoTab, window, cx| {
                dialogs::open_repo_dialog(app_for_new_tab.clone(), window, cx);
            })
            .on_action(cx.listener(|this, _: &CloseRepoTab, _, cx| {
                this.close_current_repo_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &NextRepoTab, _, cx| {
                this.next_repo_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchTab1, _, cx| {
                this.switch_tab(ActiveTab::Graph, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchTab2, _, cx| {
                this.switch_tab(ActiveTab::PullRequests, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchTab3, _, cx| {
                this.switch_tab(ActiveTab::Actions, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchTab4, _, cx| {
                this.switch_tab(ActiveTab::Issues, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleCmdLog, _, cx| {
                this.show_cmd_log = !this.show_cmd_log;
                cx.notify();
            }))
            .when(!self.open_repos.is_empty(), |col| {
                col.child(toolbar::render_repo_tabs(self, cx))
            })
            .child(toolbar::render_toolbar(self, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(match self.active_tab {
                        ActiveTab::Graph => h_resizable("graph-main-split")
                            .child(
                                resizable_panel()
                                    .size(px(230.))
                                    .size_range(px(170.)..px(380.))
                                    .child(sidebar::render_sidebar(self, cx)),
                            )
                            .child(
                                resizable_panel().child(graph_view::render_graph_view(self, cx)),
                            )
                            .child(
                                resizable_panel()
                                    .size(px(470.))
                                    .size_range(px(320.)..px(820.))
                                    .child(inspector::render_inspector(self, cx)),
                            )
                            .into_any_element(),
                        ActiveTab::PullRequests => gh_prs::render_prs_view(self, cx).into_any_element(),
                        ActiveTab::Actions => gh_actions::render_actions_view(self, cx).into_any_element(),
                        ActiveTab::Issues => gh_issues::render_issues_view(self, cx).into_any_element(),
                    }),
            )
            .when(self.show_cmd_log, |col| {
                col.child(cmd_drawer::render_cmd_drawer(self, cx))
            })
    }
}

pub fn notify_info(window: &mut Window, cx: &mut App, msg: &str) {
    window.push_notification(Notification::info(msg.to_string()), cx);
}

pub fn render_gh_label(label: &GhLabel) -> impl IntoElement {
    let bg = parse_gh_label_hex(&label.color);
    let fg = contrast_fg_for_hex(&label.color);
    div()
        .px_1p5()
        .py(px(1.))
        .rounded_full()
        .bg(bg.opacity(0.88))
        .text_xs()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(label.name.clone())
}

pub fn render_gh_unavailable(
    title: &str,
    subtitle: &str,
    cx: &mut Context<GitOutApp>,
) -> impl IntoElement {
    let bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let border = hex_color(gh_colors::BORDER);

    v_flex()
        .size_full()
        .bg(bg)
        .items_center()
        .justify_center()
        .p_6()
        .gap_3()
        .child(
            v_flex()
                .max_w(px(440.))
                .p_5()
                .gap_2()
                .rounded(px(8.))
                .bg(hex_color(gh_colors::CANVAS_SUBTLE))
                .border_1()
                .border_color(border)
                .items_center()
                .child(Icon::new(IconName::GitPullRequest).large().text_color(fg_muted))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_muted)
                        .child(subtitle.to_string()),
                )
                .child(
                    Button::new("gh-retry")
                        .small()
                        .icon(IconName::RefreshCw)
                        .label("Retry")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_all(cx);
                        })),
                ),
        )
}

fn parse_diff_lenient(patch: &str) -> Vec<DiffFile> {
    if patch.trim().is_empty() {
        return Vec::new();
    }
    DiffFile::parse_with(patch, DiffParseOptions::new().truncated_hunks(true))
        .unwrap_or_default()
}

pub(crate) fn reorder_vec<T>(items: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from >= items.len() || to >= items.len() || from == to {
        return false;
    }
    let item = items.remove(from);
    items.insert(to, item);
    true
}

#[cfg(test)]
mod tests {
    use super::reorder_vec;

    #[test]
    fn reorder_vec_handles_moves_and_bounds() {
        let mut repos = vec!["a", "b", "c", "d"];
        assert!(reorder_vec(&mut repos, 0, 2));
        assert_eq!(repos, vec!["b", "c", "a", "d"]);

        assert!(reorder_vec(&mut repos, 3, 1));
        assert_eq!(repos, vec!["b", "d", "c", "a"]);

        assert!(reorder_vec(&mut repos, 1, 3));
        assert_eq!(repos, vec!["b", "c", "a", "d"]);

        assert!(!reorder_vec(&mut repos, 1, 1));
        assert!(!reorder_vec(&mut repos, 0, 99));
        assert!(!reorder_vec(&mut repos, 99, 0));
        assert_eq!(repos, vec!["b", "c", "a", "d"]);
    }
}
