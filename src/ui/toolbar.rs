use super::{ActiveTab, GitOutApp, RemoteGitOp, dialogs};
use crate::gh::CiState;
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex};
use gpui_kit::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};

#[derive(Clone)]
struct DraggedRepoTab {
    from_idx: usize,
    name: String,
}

struct DraggedRepoTabPreview {
    name: String,
}

impl Render for DraggedRepoTabPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let blue = hex_color(gh_colors::BLUE);
        h_flex()
            .h(px(26.))
            .px_2p5()
            .gap_1p5()
            .items_center()
            .rounded(px(5.))
            .bg(hex_color(gh_colors::CANVAS_DEFAULT))
            .border_1()
            .border_color(blue)
            .opacity(0.92)
            .child(Icon::new(IconName::FolderOpen).xsmall().text_color(blue))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(hex_color(gh_colors::FG))
                    .child(self.name.clone()),
            )
    }
}

pub fn render_repo_tabs(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let tab_bar_bg = hex_color(gh_colors::CANVAS_INSET);
    let active_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let inactive_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let blue = hex_color(gh_colors::BLUE);
    let app_for_add = cx.entity();

    h_flex()
        .w_full()
        .h(px(34.))
        .flex_shrink_0()
        .px_2()
        .gap_1()
        .bg(tab_bar_bg)
        .border_b_1()
        .border_color(border)
        .items_center()
        .child(
            h_flex()
                .id("repo-tabs-scroll")
                .flex_1()
                .min_w_0()
                .gap_1()
                .items_center()
                .overflow_x_scrollbar()
                .children(app.open_repos.iter().enumerate().map(|(idx, path)| {
                    let is_active = app.repo_root.as_ref() == Some(path);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("repo")
                        .to_string();
                    let branch = if is_active {
                        app.snapshot.as_ref().and_then(|s| s.head_branch.clone())
                    } else {
                        app.repo_cache
                            .get(path)
                            .and_then(|c| c.snapshot.as_ref())
                            .and_then(|s| s.head_branch.clone())
                    };

                    h_flex()
                        .id(("repo-tab", idx))
                        .h(px(26.))
                        .pl_2p5()
                        .pr_1()
                        .gap_1p5()
                        .rounded(px(5.))
                        .items_center()
                        .flex_shrink_0()
                        .cursor_pointer()
                        .bg(if is_active { active_bg } else { inactive_bg })
                        .border_1()
                        .border_color(if is_active {
                            blue.opacity(0.6)
                        } else {
                            hex_color(gh_colors::BORDER_MUTED)
                        })
                        .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                        .on_drag(
                            DraggedRepoTab {
                                from_idx: idx,
                                name: name.clone(),
                            },
                            |drag, _, _, cx| {
                                cx.new(|_| DraggedRepoTabPreview {
                                    name: drag.name.clone(),
                                })
                            },
                        )
                        .drag_over::<DraggedRepoTab>(move |style, drag, _, _| {
                            if drag.from_idx == idx {
                                style
                            } else if drag.from_idx > idx {
                                style
                                    .border_l_2()
                                    .border_color(blue)
                                    .bg(hex_color(gh_colors::BTN_BG))
                            } else {
                                style
                                    .border_r_2()
                                    .border_color(blue)
                                    .bg(hex_color(gh_colors::BTN_BG))
                            }
                        })
                        .on_drop(cx.listener(move |this, drag: &DraggedRepoTab, _, cx| {
                            this.reorder_repo_tab(drag.from_idx, idx, cx);
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_to_repo_tab(idx, cx);
                        }))
                        .child(
                            Icon::new(IconName::FolderOpen)
                                .xsmall()
                                .text_color(if is_active { blue } else { fg_muted }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(if is_active {
                                    gpui_kit::FontWeight::SEMIBOLD
                                } else {
                                    gpui_kit::FontWeight::MEDIUM
                                })
                                .text_color(if is_active { fg } else { fg_muted })
                                .child(name),
                        )
                        .when_some(branch, |row, br| {
                            row.child(
                                div()
                                    .px_1()
                                    .rounded(px(3.))
                                    .bg(blue.opacity(0.14))
                                    .text_xs()
                                    .text_color(blue)
                                    .child(br),
                            )
                        })
                        .child(
                            Button::new(("close-repo-tab", idx))
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .tooltip("Close tab (Ctrl+W)")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_repo_tab(idx, cx);
                                })),
                        )
                }))
                .child(
                    div()
                        .flex_shrink_0()
                        .on_drop(cx.listener(move |this, drag: &DraggedRepoTab, _, cx| {
                            let last = this.open_repos.len().saturating_sub(1);
                            this.reorder_repo_tab(drag.from_idx, last, cx);
                        }))
                        .child(
                            Button::new("add-repo-tab")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Plus)
                                .tooltip("Open repository in new tab (Ctrl+T)")
                                .on_click(move |_, window, cx| {
                                    dialogs::open_repo_dialog(app_for_add.clone(), window, cx);
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .drag_over::<DraggedRepoTab>(move |style, _, _, _| {
                            style.border_l_2().border_color(blue)
                        })
                        .on_drop(cx.listener(move |this, drag: &DraggedRepoTab, _, cx| {
                            let last = this.open_repos.len().saturating_sub(1);
                            this.reorder_repo_tab(drag.from_idx, last, cx);
                        })),
                ),
        )
}

pub fn render_toolbar(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let header_bg = hex_color(gh_colors::CANVAS_INSET);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let blue = hex_color(gh_colors::BLUE);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);

    let has_repo = app.snapshot.is_some();
    let remote_busy = app.active_remote_op.is_some();
    let repo_name = app
        .snapshot
        .as_ref()
        .map(|s| s.repo_name.clone())
        .unwrap_or_else(|| "Open Repo".to_string());
    let branch_name = app
        .snapshot
        .as_ref()
        .and_then(|s| s.head_branch.clone())
        .unwrap_or_else(|| {
            app.snapshot
                .as_ref()
                .and_then(|s| s.head_hash.as_ref().map(|h| h.chars().take(7).collect()))
                .unwrap_or_else(|| "no repo".to_string())
        });
    let (ahead, behind) = app
        .snapshot
        .as_ref()
        .map(|s| s.head_ahead_behind())
        .unwrap_or((0, 0));
    let has_stashes = app
        .snapshot
        .as_ref()
        .is_some_and(|s| !s.stashes.is_empty());
    let has_dirty = app.snapshot.as_ref().is_some_and(|s| s.is_dirty());
    let has_github = app
        .snapshot
        .as_ref()
        .is_some_and(|s| s.github_nwo.is_some());

    let pr_count = app.prs.len();
    let issue_count = app.issues.len();
    let latest_run_ci = app.runs.first().map(|r| r.ci_state()).unwrap_or(CiState::None);

    let app_for_repo = cx.entity();
    let app_for_branch = cx.entity();
    let app_for_stash = cx.entity();

    let pull_label = if behind > 0 {
        format!("Pull ↓{behind}")
    } else {
        "Pull".to_string()
    };
    let push_label = if ahead > 0 {
        format!("Push ↑{ahead}")
    } else {
        "Push".to_string()
    };

    h_flex()
        .w_full()
        .h(px(56.))
        .flex_shrink_0()
        .px_3()
        .gap_3()
        .bg(header_bg)
        .border_b_1()
        .border_color(border)
        .items_center()
        // Left: Brand + Repo Switcher (flex_1 + overflow_hidden keeps center buttons anchored)
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .gap_2()
                .items_center()
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .flex_shrink_0()
                        .pr_1()
                        .child(Icon::new(IconName::GitGraph).text_color(blue))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(fg)
                                .child("Git Out"),
                        ),
                )
                .child(
                    Button::new("repo-switcher")
                        .outline()
                        .h(px(36.))
                        .px_3()
                        .max_w(px(260.))
                        .icon(IconName::FolderOpen)
                        .label(format!("{repo_name} · {branch_name}"))
                        .tooltip("Open or switch repository (Ctrl+O)")
                        .on_click(move |_, window, cx| {
                            dialogs::open_repo_dialog(app_for_repo.clone(), window, cx);
                        }),
                ),
        )
        // Center: Chunky GitKraken-style Git action bar
        .child(
            h_flex()
                .flex_shrink_0()
                .gap_2()
                .items_center()
                .child(
                    Button::new("btn-fetch")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::CloudDownload)
                        .label("Fetch")
                        .loading(app.active_remote_op == Some(RemoteGitOp::Fetch))
                        .disabled(!has_repo || remote_busy)
                        .tooltip("git fetch --all --prune --tags")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.fetch_all(cx);
                        })),
                )
                .child(
                    Button::new("btn-pull")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::ArrowDown)
                        .label(pull_label)
                        .loading(app.active_remote_op == Some(RemoteGitOp::Pull))
                        .disabled(!has_repo || remote_busy)
                        .tooltip("git pull")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pull(cx);
                        })),
                )
                .child(
                    Button::new("btn-push")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::ArrowUp)
                        .label(push_label)
                        .loading(app.active_remote_op == Some(RemoteGitOp::Push))
                        .disabled(!has_repo || remote_busy)
                        .tooltip("git push (auto-sets upstream if missing)")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.push(cx);
                        })),
                )
                .child(div().w(px(1.)).h(px(22.)).bg(border).mx_1())
                .child(
                    Button::new("btn-branch")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::GitBranchPlus)
                        .label("Branch")
                        .disabled(!has_repo)
                        .tooltip("Create new branch")
                        .on_click(move |_, window, cx| {
                            dialogs::open_create_branch_dialog(
                                app_for_branch.clone(),
                                None,
                                window,
                                cx,
                            );
                        }),
                )
                .child(
                    Button::new("btn-stash")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::Archive)
                        .label("Stash")
                        .disabled(!has_repo || !has_dirty)
                        .tooltip("Stash working tree changes")
                        .on_click(move |_, window, cx| {
                            dialogs::open_stash_dialog(app_for_stash.clone(), window, cx);
                        }),
                )
                .child(
                    Button::new("btn-pop")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::ArchiveRestore)
                        .label("Pop")
                        .disabled(!has_repo || !has_stashes)
                        .tooltip("Pop latest stash")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.stash_pop(None, cx);
                        })),
                )
                .child(
                    Button::new("btn-refresh")
                        .h(px(36.))
                        .w(px(36.))
                        .icon(IconName::RefreshCw)
                        .disabled(!has_repo)
                        .tooltip("Refresh git & gh (Ctrl+R)")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_all(cx);
                        })),
                ),
        )
        // Right: Mode Tabs + Open on GitHub + Cmd Log
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .justify_end()
                .gap_2()
                .items_center()
                .child(
                    h_flex()
                        .p(px(3.))
                        .gap_1()
                        .rounded(px(7.))
                        .bg(hex_color(gh_colors::CANVAS_SUBTLE))
                        .border_1()
                        .border_color(border)
                        .child(render_mode_tab(
                            "tab-graph",
                            "Graph",
                            IconName::GitGraph,
                            None,
                            None,
                            app.active_tab == ActiveTab::Graph,
                            cx.listener(|this, _, _, cx| {
                                this.active_tab = ActiveTab::Graph;
                                cx.notify();
                            }),
                        ))
                        .child(render_mode_tab(
                            "tab-prs",
                            "PRs",
                            IconName::GitPullRequest,
                            (pr_count > 0).then_some(pr_count.to_string()),
                            None,
                            app.active_tab == ActiveTab::PullRequests,
                            cx.listener(|this, _, _, cx| {
                                this.switch_tab(ActiveTab::PullRequests, cx);
                            }),
                        ))
                        .child(render_mode_tab(
                            "tab-actions",
                            "Actions",
                            IconName::Play,
                            None,
                            match latest_run_ci {
                                CiState::Success => Some(green),
                                CiState::Failure => Some(red),
                                CiState::Pending => Some(yellow),
                                CiState::None => None,
                            },
                            app.active_tab == ActiveTab::Actions,
                            cx.listener(|this, _, _, cx| {
                                this.switch_tab(ActiveTab::Actions, cx);
                            }),
                        ))
                        .child(render_mode_tab(
                            "tab-issues",
                            "Issues",
                            IconName::CircleDot,
                            (issue_count > 0).then_some(issue_count.to_string()),
                            None,
                            app.active_tab == ActiveTab::Issues,
                            cx.listener(|this, _, _, cx| {
                                this.switch_tab(ActiveTab::Issues, cx);
                            }),
                        )),
                )
                .when(has_github, |row| {
                    row.child(
                        Button::new("btn-gh-web")
                            .h(px(36.))
                            .w(px(36.))
                            .icon(IconName::ExternalLink)
                            .tooltip("Open repository on GitHub (gh repo view --web)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_repo_web(cx);
                            })),
                    )
                })
                .child(
                    Button::new("btn-cmd-log")
                        .h(px(36.))
                        .px_3()
                        .icon(IconName::Terminal)
                        .label(format!("{}", app.cmd_log.len()))
                        .tooltip("Toggle git/gh command log (Ctrl+`)")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_cmd_log = !this.show_cmd_log;
                            cx.notify();
                        })),
                ),
        )
}

fn render_mode_tab(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    count_badge: Option<String>,
    status_dot: Option<gpui_kit::Hsla>,
    active: bool,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let bg = if active {
        hex_color(gh_colors::BTN_BG)
    } else {
        gpui_kit::transparent_black()
    };

    div()
        .id(id)
        .h(px(30.))
        .px_3()
        .flex()
        .items_center()
        .rounded(px(5.))
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
        .on_click(on_click)
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .child(Icon::new(icon).text_color(if active { fg } else { fg_muted }))
                .child(
                    div()
                        .text_sm()
                        .font_weight(if active {
                            gpui_kit::FontWeight::SEMIBOLD
                        } else {
                            gpui_kit::FontWeight::MEDIUM
                        })
                        .text_color(if active { fg } else { fg_muted })
                        .child(label),
                )
                .when_some(status_dot, |row, color| {
                    row.child(div().size(px(7.)).rounded_full().bg(color))
                })
                .when_some(count_badge, |row, count| {
                    row.child(
                        div()
                            .px_1p5()
                            .py(px(1.))
                            .rounded_full()
                            .bg(hex_color(gh_colors::BORDER))
                            .text_xs()
                            .text_color(fg_muted)
                            .child(count),
                    )
                }),
        )
}
