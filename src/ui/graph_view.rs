use super::{ActiveTab, GitOutApp, Selection, dialogs, graph_canvas, reflog_dialog};
use crate::gh::{CiState, PullRequest};
use crate::git::{BisectAction, RefKind, RepoOpState, format_relative_time};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement, ScrollStrategy,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px, uniform_list,
};
use std::collections::HashMap;
use std::rc::Rc;

pub fn render_graph_view(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let fg_subtle = hex_color(gh_colors::FG_SUBTLE);
    let blue = hex_color(gh_colors::BLUE);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);
    let purple = hex_color(gh_colors::PURPLE);
    let orange = hex_color(gh_colors::ORANGE);

    let Some(snap) = app.snapshot.as_ref() else {
        return v_flex()
            .size_full()
            .bg(canvas_bg)
            .items_center()
            .justify_center()
            .p_6()
            .gap_3()
            .child(Icon::new(IconName::GitGraph).large().text_color(blue))
            .child(
                div()
                    .text_base()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child("Welcome to Git Out"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Open any Git repository to view its commit graph, stage hunks/lines, resolve conflicts, and browse PRs/CI via `gh`."),
            )
            .into_any_element();
    };

    let pr_by_branch: HashMap<String, PullRequest> = app
        .prs
        .iter()
        .map(|pr| (pr.head_ref_name.clone(), pr.clone()))
        .collect();

    let pickaxe_mode = app.pickaxe_commits.is_some();
    let commits_source = app.pickaxe_commits.as_ref().unwrap_or(&snap.commits);

    let query = app.search_input.read(cx).value().trim().to_lowercase();
    let filtered_indices: Vec<usize> = commits_source
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            if pickaxe_mode || query.is_empty() {
                return true;
            }
            c.subject.to_lowercase().contains(&query)
                || c.author.to_lowercase().contains(&query)
                || c.short_hash.to_lowercase().contains(&query)
                || c.hash.to_lowercase().starts_with(&query)
                || c.refs
                    .iter()
                    .any(|r| r.name.to_lowercase().contains(&query))
        })
        .map(|(idx, _)| idx)
        .collect();

    let max_lanes = snap
        .graph_rows
        .iter()
        .map(|r| r.lane_count)
        .max()
        .unwrap_or(2);
    let graph_w = graph_canvas::graph_column_width(max_lanes);

    let head_col = snap
        .commits
        .iter()
        .position(|c| c.is_head)
        .and_then(|idx| snap.graph_rows.get(idx))
        .map(|r| r.commit_col)
        .unwrap_or(0);
    let head_filtered_pos = if pickaxe_mode {
        None
    } else {
        filtered_indices
            .iter()
            .position(|&idx| snap.commits[idx].is_head)
    };

    let wip_selected = matches!(app.selection, Selection::Wip);
    let dirty = snap.is_dirty();
    let staged_n = snap.staged_files.len();
    let unstaged_n = snap.unstaged_files.len();
    let has_stashes = !snap.stashes.is_empty();
    let has_conflicts = snap.has_conflicts();

    let selected_hash = match &app.selection {
        Selection::Commit(h) => Some(h.clone()),
        _ => None,
    };

    let item_count = filtered_indices.len();
    let list_scroll = app.graph_scroll.clone();
    let app_entity = cx.entity();
    let op_state = snap.op_state.clone();
    let compare_base = app.compare_base.clone();

    v_flex()
        .size_full()
        .bg(canvas_bg)
        // Top search + pickaxe + branch scope strip
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_1p5()
                .gap_1p5()
                .bg(subtle_bg)
                .border_b_1()
                .border_color(border)
                .items_center()
                .child(
                    div().flex_1().child(
                        Input::new(&app.search_input)
                            .id("graph-search")
                            .small()
                            .cleanable(true)
                            .prefix(Icon::new(IconName::Search).xsmall().text_color(fg_muted)),
                    ),
                )
                .child({
                    let q_term = app.search_input.read(cx).value().trim().to_string();
                    Button::new("btn-pickaxe")
                        .xsmall()
                        .icon(IconName::FileSearch)
                        .label("Code (-S)")
                        .tooltip("Search commit history for added/removed string (git log -S)")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_pickaxe_search(q_term.clone(), cx);
                        }))
                })
                .child(
                    Button::new("toggle-branch-scope")
                        .xsmall()
                        .outline()
                        .icon(IconName::GitBranch)
                        .label(if app.current_branch_only {
                            "HEAD Only"
                        } else {
                            "All Branches"
                        })
                        .tooltip("Toggle showing all branches vs current branch history")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_current_branch_only(cx);
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_muted)
                        .flex_shrink_0()
                        .child(format!("{item_count} commits")),
                )
                .when(snap.has_more_commits && !pickaxe_mode, |row| {
                    row.child(
                        Button::new("btn-load-more")
                            .xsmall()
                            .label("+500")
                            .tooltip("Load 500 more commits")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_more_commits(cx);
                            })),
                    )
                })
                .when_some(head_filtered_pos, |row, pos| {
                    let handle = list_scroll.clone();
                    row.child(
                        Button::new("jump-head")
                            .xsmall()
                            .icon(IconName::GitCommitHorizontal)
                            .label("HEAD")
                            .tooltip("Scroll to HEAD commit")
                            .on_click(move |_, _, _| {
                                handle.scroll_to_item(pos, ScrollStrategy::Center);
                            }),
                    )
                }),
        )
        // In-progress Git Operation Banner (Merge / Rebase / Cherry-pick / Revert / Bisect)
        .when(op_state != RepoOpState::None, |col| {
            let label = match &op_state {
                RepoOpState::Merging => "MERGE IN PROGRESS".to_string(),
                RepoOpState::Rebasing {
                    step: Some(s),
                    total: Some(t),
                } => format!("REBASE IN PROGRESS (step {s}/{t})"),
                RepoOpState::Rebasing { .. } => "REBASE IN PROGRESS".to_string(),
                RepoOpState::CherryPicking => "CHERRY-PICK IN PROGRESS".to_string(),
                RepoOpState::Reverting => "REVERT IN PROGRESS".to_string(),
                RepoOpState::Bisecting => "GIT BISECT IN PROGRESS".to_string(),
                RepoOpState::None => String::new(),
            };
            let is_bisect = op_state == RepoOpState::Bisecting;
            let can_skip = matches!(
                op_state,
                RepoOpState::Rebasing { .. } | RepoOpState::CherryPicking | RepoOpState::Reverting
            );
            let banner_color = if is_bisect { purple } else { orange };

            col.child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .bg(banner_color.opacity(0.14))
                    .border_b_1()
                    .border_color(banner_color.opacity(0.4))
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .small()
                                    .text_color(banner_color),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(banner_color)
                                    .child(label),
                            )
                            .when(has_conflicts, |r| {
                                r.child(
                                    div()
                                        .text_xs()
                                        .text_color(red)
                                        .child("Resolve conflicted files in WIP inspector"),
                                )
                            }),
                    )
                    .child(if is_bisect {
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("bisect-good")
                                    .xsmall()
                                    .primary()
                                    .label("Good")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.bisect_mark(BisectAction::Good, cx);
                                    })),
                            )
                            .child(
                                Button::new("bisect-bad")
                                    .xsmall()
                                    .danger()
                                    .label("Bad")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.bisect_mark(BisectAction::Bad, cx);
                                    })),
                            )
                            .child(
                                Button::new("bisect-skip")
                                    .xsmall()
                                    .label("Skip")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.bisect_mark(BisectAction::Skip, cx);
                                    })),
                            )
                            .child(
                                Button::new("bisect-reset")
                                    .xsmall()
                                    .label("End Bisect")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.bisect_mark(BisectAction::Reset, cx);
                                    })),
                            )
                    } else {
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("op-continue")
                                    .xsmall()
                                    .primary()
                                    .icon(IconName::Play)
                                    .label("Continue")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.continue_op(cx);
                                    })),
                            )
                            .when(can_skip, |r| {
                                r.child(
                                    Button::new("op-skip")
                                        .xsmall()
                                        .label("Skip")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.skip_op(cx);
                                        })),
                                )
                            })
                            .child(
                                Button::new("op-abort")
                                    .xsmall()
                                    .danger()
                                    .icon(IconName::Ban)
                                    .label("Abort")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.abort_op(cx);
                                    })),
                            )
                    }),
            )
        })
        // Compare Base Banner
        .when_some(compare_base.clone(), |col, (_, label)| {
            col.child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1()
                    .bg(blue.opacity(0.12))
                    .border_b_1()
                    .border_color(blue.opacity(0.35))
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(Icon::new(IconName::GitCompare).xsmall().text_color(blue))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(fg)
                                    .child(format!("Compare base: `{label}` — right-click any commit to compare")),
                            ),
                    )
                    .child(
                        Button::new("clear-compare-base")
                            .xsmall()
                            .ghost()
                            .icon(IconName::X)
                            .label("Clear")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_compare_base(cx);
                            })),
                    ),
            )
        })
        // Pickaxe Active Banner
        .when_some(app.pickaxe_query.clone(), |col, term| {
            col.child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1()
                    .bg(purple.opacity(0.12))
                    .border_b_1()
                    .border_color(purple.opacity(0.35))
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(fg)
                            .child(format!("Code Search (`git log -S \"{term}\"`): {item_count} matches")),
                    )
                    .child(
                        Button::new("clear-pickaxe")
                            .xsmall()
                            .ghost()
                            .icon(IconName::X)
                            .label("Exit Code Search")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_pickaxe_search(cx);
                            })),
                    ),
            )
        })
        // Pinned WIP / Working Tree row
        .child(
            h_flex()
                .id("wip-row")
                .w_full()
                .h(px(graph_canvas::ROW_HEIGHT))
                .flex_shrink_0()
                .pr_3()
                .items_center()
                .cursor_pointer()
                .bg(if wip_selected {
                    blue.opacity(0.14)
                } else if has_conflicts {
                    red.opacity(0.10)
                } else if dirty {
                    orange.opacity(0.06)
                } else {
                    subtle_bg
                })
                .border_b_1()
                .border_color(border)
                .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.select_wip(cx);
                }))
                .child(graph_canvas::render_wip_cell(graph_w, dirty, head_col))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded(px(4.))
                                .bg(if has_conflicts {
                                    red.opacity(0.22)
                                } else if dirty {
                                    orange.opacity(0.20)
                                } else {
                                    border
                                })
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(if has_conflicts {
                                    red
                                } else if dirty {
                                    orange
                                } else {
                                    fg_muted
                                })
                                .child("WIP"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child(if has_conflicts {
                                    "Merge Conflicts Detected"
                                } else if dirty {
                                    "Working Tree Changes"
                                } else {
                                    "Working Tree Clean"
                                }),
                        )
                        .when(dirty, |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(green)
                                    .child(format!("{staged_n} staged")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if has_conflicts { red } else { orange })
                                    .child(format!("{unstaged_n} unstaged")),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_subtle)
                        .child("Click to stage & commit"),
                )
                .context_menu({
                    let app_wip = app_entity.clone();
                    move |menu, _, _| {
                        let app_stage = app_wip.clone();
                        let app_unstage = app_wip.clone();
                        let app_discard = app_wip.clone();
                        let app_stash = app_wip.clone();
                        let app_pop = app_wip.clone();
                        let app_patch = app_wip.clone();
                        menu.label("Working Tree")
                            .item(
                                PopupMenuItem::new("Stage all changes")
                                    .icon(IconName::Plus)
                                    .disabled(unstaged_n == 0)
                                    .on_click(move |_, _, cx| {
                                        app_stage.update(cx, |app, cx| app.stage_all(cx));
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Unstage all changes")
                                    .icon(IconName::X)
                                    .disabled(staged_n == 0)
                                    .on_click(move |_, _, cx| {
                                        app_unstage.update(cx, |app, cx| app.unstage_all(cx));
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Discard all changes…")
                                    .icon(IconName::Trash)
                                    .disabled(unstaged_n == 0 && staged_n == 0)
                                    .on_click(move |_, window, cx| {
                                        let app = app_discard.clone();
                                        dialogs::open_confirm_danger(
                                            "Discard All Working Changes".into(),
                                            "Permanently discard all staged, unstaged, and untracked changes in the working tree? This cannot be undone.".into(),
                                            "Discard All".into(),
                                            window,
                                            cx,
                                            Rc::new(move |_, cx| {
                                                app.update(cx, |app, cx| app.discard_all_changes(cx));
                                            }),
                                        );
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Stash changes…")
                                    .icon(IconName::Archive)
                                    .disabled(!dirty)
                                    .on_click(move |_, window, cx| {
                                        dialogs::open_stash_dialog(app_stash.clone(), window, cx);
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Pop latest stash")
                                    .icon(IconName::ArchiveRestore)
                                    .disabled(!has_stashes)
                                    .on_click(move |_, _, cx| {
                                        app_pop.update(cx, |app, cx| app.stash_pop(None, cx));
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Apply patch file…")
                                    .icon(IconName::FileDiff)
                                    .on_click(move |_, window, cx| {
                                        reflog_dialog::open_apply_patch_dialog(
                                            app_patch.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                    }
                }),
        )
        // Virtualized Commit Graph Rows
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .when(item_count == 0, |area| {
                    area.child(
                        div()
                            .p_6()
                            .text_xs()
                            .text_color(fg_muted)
                            .child("No matching commits."),
                    )
                })
                .when(item_count > 0, |area| {
                    let commits = commits_source.clone();
                    let graph_rows = snap.graph_rows.clone();
                    let is_filtered = pickaxe_mode || !query.is_empty();
                    let app_ref = app_entity.clone();
                    let selected_hash = selected_hash.clone();
                    let prs_map = pr_by_branch.clone();
                    let head_ref_str = snap
                        .head_branch
                        .clone()
                        .or_else(|| snap.head_hash.clone())
                        .unwrap_or_else(|| "HEAD".to_string());
                    let is_bisecting = op_state == RepoOpState::Bisecting;

                    area.child(
                        uniform_list("commit-graph-list", item_count, move |range, _, _| {
                            range
                                .map(|vis_ix| {
                                    let real_ix = filtered_indices[vis_ix];
                                    let commit = &commits[real_ix];
                                    let is_selected = selected_hash
                                        .as_deref()
                                        .is_some_and(|h| h == commit.hash);
                                    let hash_click = commit.hash.clone();
                                    let short_hash = commit.short_hash.clone();
                                    let subject_copy = commit.subject.clone();
                                    let local_branches_here: Vec<String> = commit
                                        .refs
                                        .iter()
                                        .filter(|r| r.kind == RefKind::LocalBranch)
                                        .map(|r| r.name.clone())
                                        .collect();
                                    let dbl_checkout_target = local_branches_here
                                        .first()
                                        .cloned()
                                        .unwrap_or_else(|| commit.hash.clone());
                                    let app_click = app_ref.clone();
                                    let app_menu = app_ref.clone();
                                    let hash_menu = commit.hash.clone();
                                    let cmp_base_opt = compare_base.clone();
                                    let head_for_bisect = head_ref_str.clone();

                                    let matched_pr = commit.refs.iter().find_map(|r| {
                                        if r.kind == RefKind::LocalBranch {
                                            prs_map.get(&r.name).cloned()
                                        } else {
                                            None
                                        }
                                    });

                                    h_flex()
                                        .id(("commit-row", real_ix))
                                        .w_full()
                                        .h(px(graph_canvas::ROW_HEIGHT))
                                        .pr_3()
                                        .items_center()
                                        .cursor_pointer()
                                        .bg(if is_selected {
                                            blue.opacity(0.16)
                                        } else if commit.is_head {
                                            blue.opacity(0.05)
                                        } else {
                                            canvas_bg
                                        })
                                        .border_b_1()
                                        .border_color(hex_color(gh_colors::BORDER_MUTED))
                                        .hover(|s| s.bg(hex_color(gh_colors::CANVAS_SUBTLE)))
                                        .on_click(move |event, _, cx| {
                                            let h = hash_click.clone();
                                            let dbl_target = dbl_checkout_target.clone();
                                            let is_dbl = event.click_count() >= 2;
                                            app_click.update(cx, |app, cx| {
                                                app.select_commit(h, cx);
                                                if is_dbl {
                                                    app.checkout_target(dbl_target, cx);
                                                }
                                            });
                                        })
                                        .context_menu(move |mut menu, _, _| {
                                            menu = menu.label(format!("Commit {short_hash}"));
                                            for br in &local_branches_here {
                                                let br_name = br.clone();
                                                let app_br = app_menu.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(format!(
                                                        "Checkout branch '{br_name}'"
                                                    ))
                                                    .icon(IconName::GitBranch)
                                                    .on_click(move |_, _, cx| {
                                                        let b = br_name.clone();
                                                        app_br.update(cx, |app, cx| {
                                                            app.checkout_target(b, cx);
                                                        });
                                                    }),
                                                );
                                            }
                                            let h_co = hash_menu.clone();
                                            let app_co = app_menu.clone();
                                            let h_br = hash_menu.clone();
                                            let app_new_br = app_menu.clone();
                                            let h_tag = hash_menu.clone();
                                            let sh_tag = short_hash.clone();
                                            let app_tag = app_menu.clone();
                                            let h_cp = hash_menu.clone();
                                            let app_cp = app_menu.clone();
                                            let h_rev = hash_menu.clone();
                                            let app_rev = app_menu.clone();
                                            let h_rb = hash_menu.clone();
                                            let sh_rb = short_hash.clone();
                                            let app_rb = app_menu.clone();
                                            let h_rst = hash_menu.clone();
                                            let sh_rst = short_hash.clone();
                                            let app_rst = app_menu.clone();
                                            let h_copy = hash_menu.clone();
                                            let msg_copy = subject_copy.clone();
                                            let h_patch = hash_menu.clone();
                                            let app_patch = app_menu.clone();
                                            let h_set_cmp = hash_menu.clone();
                                            let sh_set_cmp = short_hash.clone();
                                            let app_set_cmp = app_menu.clone();

                                            menu = menu
                                                .item(
                                                    PopupMenuItem::new(format!(
                                                        "Checkout commit ({short_hash})"
                                                    ))
                                                    .icon(IconName::Check)
                                                    .on_click(move |_, _, cx| {
                                                        let h = h_co.clone();
                                                        app_co.update(cx, |app, cx| {
                                                            app.checkout_target(h, cx);
                                                        });
                                                    }),
                                                )
                                                .item(
                                                    PopupMenuItem::new("Create branch here…")
                                                        .icon(IconName::GitBranchPlus)
                                                        .on_click(move |_, window, cx| {
                                                            dialogs::open_create_branch_dialog(
                                                                app_new_br.clone(),
                                                                Some(h_br.clone()),
                                                                window,
                                                                cx,
                                                            );
                                                        }),
                                                )
                                                .item(
                                                    PopupMenuItem::new("Create tag here…")
                                                        .icon(IconName::Tag)
                                                        .on_click(move |_, window, cx| {
                                                            reflog_dialog::open_create_tag_dialog(
                                                                app_tag.clone(),
                                                                h_tag.clone(),
                                                                sh_tag.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        }),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new("Cherry-pick commit")
                                                        .icon(IconName::GitCommitHorizontal)
                                                        .on_click(move |_, _, cx| {
                                                            let h = h_cp.clone();
                                                            app_cp.update(cx, |app, cx| {
                                                                app.cherry_pick(h, cx);
                                                            });
                                                        }),
                                                )
                                                .item(
                                                    PopupMenuItem::new("Revert commit")
                                                        .icon(IconName::RotateCcw)
                                                        .on_click(move |_, _, cx| {
                                                            let h = h_rev.clone();
                                                            app_rev.update(cx, |app, cx| {
                                                                app.revert_commit(h, cx);
                                                            });
                                                        }),
                                                )
                                                .item(
                                                    PopupMenuItem::new(
                                                        "Interactive Rebase from here…",
                                                    )
                                                    .icon(IconName::GitBranch)
                                                    .on_click(move |_, _, cx| {
                                                        let h = h_rb.clone();
                                                        let sh = sh_rb.clone();
                                                        app_rb.update(cx, |app, cx| {
                                                            app.open_interactive_rebase_from(
                                                                h, sh, cx,
                                                            );
                                                        });
                                                    }),
                                                )
                                                .item(
                                                    PopupMenuItem::new(
                                                        "Reset current branch to here…",
                                                    )
                                                    .icon(IconName::Trash)
                                                    .on_click(move |_, window, cx| {
                                                        dialogs::open_reset_dialog(
                                                            app_rst.clone(),
                                                            h_rst.clone(),
                                                            sh_rst.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new("Set as Compare Base")
                                                        .icon(IconName::GitCompare)
                                                        .on_click(move |_, _, cx| {
                                                            let h = h_set_cmp.clone();
                                                            let sh = sh_set_cmp.clone();
                                                            app_set_cmp.update(cx, |app, cx| {
                                                                app.set_compare_base(h, sh, cx);
                                                            });
                                                        }),
                                                );

                                            if let Some((base_h, base_lbl)) = cmp_base_opt.clone() {
                                                let head_cmp = hash_menu.clone();
                                                let app_cmp = app_menu.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(format!(
                                                        "Compare with `{base_lbl}`"
                                                    ))
                                                    .icon(IconName::GitCompareArrows)
                                                    .on_click(move |_, _, cx| {
                                                        let b = base_h.clone();
                                                        let hd = head_cmp.clone();
                                                        app_cmp.update(cx, |app, cx| {
                                                            app.compare_commits(b, hd, cx);
                                                        });
                                                    }),
                                                );
                                            }

                                            if !is_bisecting {
                                                let h_good = hash_menu.clone();
                                                let h_bad = head_for_bisect.clone();
                                                let app_bs = app_menu.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(
                                                        "Bisect: Start (this Good, HEAD Bad)",
                                                    )
                                                    .icon(IconName::Search)
                                                    .on_click(move |_, _, cx| {
                                                        let g = h_good.clone();
                                                        let b = h_bad.clone();
                                                        app_bs.update(cx, |app, cx| {
                                                            app.bisect_start(b, g, cx);
                                                        });
                                                    }),
                                                );
                                            }

                                            menu.separator()
                                                .item(
                                                    PopupMenuItem::new("Copy commit SHA")
                                                        .icon(IconName::Copy)
                                                        .on_click(move |_, window, cx| {
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(
                                                                    h_copy.clone(),
                                                                ),
                                                            );
                                                            super::notify_info(
                                                                window,
                                                                cx,
                                                                "Copied commit SHA",
                                                            );
                                                        }),
                                                )
                                                .item(
                                                    PopupMenuItem::new("Copy commit message")
                                                        .icon(IconName::Copy)
                                                        .on_click(move |_, window, cx| {
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(
                                                                    msg_copy.clone(),
                                                                ),
                                                            );
                                                            super::notify_info(
                                                                window,
                                                                cx,
                                                                "Copied commit message",
                                                            );
                                                        }),
                                                )
                                                .item(
                                                    PopupMenuItem::new("Copy commit .patch")
                                                        .icon(IconName::FileDiff)
                                                        .on_click(move |_, _, cx| {
                                                            let h = h_patch.clone();
                                                            app_patch.update(cx, |app, cx| {
                                                                app.copy_commit_patch(h, cx);
                                                            });
                                                        }),
                                                )
                                        })
                                        .when(!is_filtered && real_ix < graph_rows.len(), |row| {
                                            row.child(graph_canvas::render_graph_cell(
                                                graph_rows[real_ix].clone(),
                                                graph_w,
                                            ))
                                        })
                                        .when(is_filtered || real_ix >= graph_rows.len(), |row| {
                                            row.pl_3()
                                        })
                                        // Refs + PR badge + subject
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_1p5()
                                                .items_center()
                                                .children(commit.refs.iter().map(|r| {
                                                    let (pill_bg, pill_fg, pill_border) =
                                                        match r.kind {
                                                            RefKind::Head => (
                                                                green.opacity(0.18),
                                                                green,
                                                                green.opacity(0.45),
                                                            ),
                                                            RefKind::LocalBranch => (
                                                                blue.opacity(0.15),
                                                                blue,
                                                                blue.opacity(0.40),
                                                            ),
                                                            RefKind::RemoteBranch => (
                                                                purple.opacity(0.15),
                                                                purple,
                                                                purple.opacity(0.40),
                                                            ),
                                                            RefKind::Tag => (
                                                                yellow.opacity(0.16),
                                                                yellow,
                                                                yellow.opacity(0.42),
                                                            ),
                                                        };
                                                    div()
                                                        .px_1p5()
                                                        .py(px(1.))
                                                        .rounded(px(4.))
                                                        .bg(pill_bg)
                                                        .border_1()
                                                        .border_color(pill_border)
                                                        .text_xs()
                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                        .text_color(pill_fg)
                                                        .flex_shrink_0()
                                                        .child(r.name.clone())
                                                }))
                                                .when_some(matched_pr, |row, pr| {
                                                    let pr_num = pr.number;
                                                    let app_pr = app_ref.clone();
                                                    let ci_color = match pr.ci_state() {
                                                        CiState::Success => green,
                                                        CiState::Failure => red,
                                                        CiState::Pending => yellow,
                                                        CiState::None => fg_muted,
                                                    };
                                                    row.child(
                                                        h_flex()
                                                            .id(("graph-pr-pill", real_ix))
                                                            .px_1p5()
                                                            .py(px(1.))
                                                            .gap_1()
                                                            .rounded(px(4.))
                                                            .bg(hex_color(gh_colors::BTN_BG))
                                                            .border_1()
                                                            .border_color(border)
                                                            .items_center()
                                                            .flex_shrink_0()
                                                            .cursor_pointer()
                                                            .on_click(move |_, _, cx| {
                                                                cx.stop_propagation();
                                                                app_pr.update(cx, |app, cx| {
                                                                    app.active_tab =
                                                                        ActiveTab::PullRequests;
                                                                    app.select_pr(pr_num, cx);
                                                                });
                                                            })
                                                            .child(
                                                                div()
                                                                    .size(px(6.))
                                                                    .rounded_full()
                                                                    .bg(ci_color),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        gpui_kit::FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(fg)
                                                                    .child(format!("PR #{pr_num}")),
                                                            ),
                                                    )
                                                })
                                                .when(commit.signature_status == 'G', |row| {
                                                    row.child(
                                                        Icon::new(IconName::BadgeCheck)
                                                            .xsmall()
                                                            .text_color(green),
                                                    )
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if commit.is_head {
                                                            fg
                                                        } else {
                                                            hex_color(0xd0d7de)
                                                        })
                                                        .truncate()
                                                        .child(commit.subject.clone()),
                                                ),
                                        )
                                        // Author + SHA + Time
                                        .child(
                                            h_flex()
                                                .gap_2p5()
                                                .pl_2()
                                                .flex_shrink_0()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .w(px(92.))
                                                        .text_xs()
                                                        .text_color(fg_muted)
                                                        .truncate()
                                                        .child(commit.author.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(54.))
                                                        .text_xs()
                                                        .font_family("monospace")
                                                        .text_color(fg_subtle)
                                                        .child(commit.short_hash.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(56.))
                                                        .text_right()
                                                        .text_xs()
                                                        .text_color(fg_subtle)
                                                        .child(format_relative_time(
                                                            commit.timestamp,
                                                        )),
                                                ),
                                        )
                                })
                                .collect::<Vec<_>>()
                        })
                        .track_scroll(&list_scroll)
                        .size_full(),
                    )
                }),
        )
        .into_any_element()
}
