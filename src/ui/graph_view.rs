use super::{ActiveTab, GitOutApp, Selection, graph_canvas};
use crate::gh::{CiState, PullRequest};
use crate::git::{RefKind, format_relative_time};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{Context, InteractiveElement as _, IntoElement, ParentElement, ScrollStrategy, StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px, uniform_list};
use std::collections::HashMap;

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
                    .child("Open any Git repository to view its commit graph, stage changes, and browse PRs/CI via `gh`."),
            )
            .into_any_element();
    };

    // Map branch name -> PullRequest for live PR & CI badges right on the commit graph.
    let pr_by_branch: HashMap<String, PullRequest> = app
        .prs
        .iter()
        .map(|pr| (pr.head_ref_name.clone(), pr.clone()))
        .collect();

    let query = app.search_input.read(cx).value().trim().to_lowercase();
    let filtered_indices: Vec<usize> = snap
        .commits
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            if query.is_empty() {
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
    let head_filtered_pos = filtered_indices
        .iter()
        .position(|&idx| snap.commits[idx].is_head);

    let wip_selected = matches!(app.selection, Selection::Wip);
    let dirty = snap.is_dirty();
    let staged_n = snap.staged_files.len();
    let unstaged_n = snap.unstaged_files.len();

    let selected_hash = match &app.selection {
        Selection::Commit(h) => Some(h.clone()),
        Selection::Wip => None,
    };

    let item_count = filtered_indices.len();
    let list_scroll = app.graph_scroll.clone();
    let app_entity = cx.entity();

    v_flex()
        .size_full()
        .bg(canvas_bg)
        // Top search + jump strip
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_1p5()
                .gap_2()
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
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_muted)
                        .flex_shrink_0()
                        .child(format!("{item_count} commits")),
                )
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
                                .bg(if dirty {
                                    orange.opacity(0.20)
                                } else {
                                    border
                                })
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(if dirty { orange } else { fg_muted })
                                .child("WIP"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child(if dirty {
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
                                    .text_color(orange)
                                    .child(format!("{unstaged_n} unstaged")),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_subtle)
                        .child("Click to stage & commit"),
                ),
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
                    let commits = snap.commits.clone();
                    let graph_rows = snap.graph_rows.clone();
                    let is_filtered = !query.is_empty();
                    let app_ref = app_entity.clone();
                    let selected_hash = selected_hash.clone();
                    let prs_map = pr_by_branch.clone();

                    area.child(
                        uniform_list("commit-graph-list", item_count, move |range, _, _| {
                            range
                                .map(|vis_ix| {
                                    let real_ix = filtered_indices[vis_ix];
                                    let commit = &commits[real_ix];
                                    let grow = &graph_rows[real_ix];
                                    let is_selected = selected_hash
                                        .as_deref()
                                        .is_some_and(|h| h == commit.hash);
                                    let hash_click = commit.hash.clone();
                                    let app_click = app_ref.clone();

                                    // Find matching PR from local branch names on this commit
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
                                        .on_click(move |_, _, cx| {
                                            let h = hash_click.clone();
                                            app_click.update(cx, |app, cx| {
                                                app.select_commit(h, cx);
                                            });
                                        })
                                        .when(!is_filtered, |row| {
                                            row.child(graph_canvas::render_graph_cell(
                                                grow.clone(),
                                                graph_w,
                                            ))
                                        })
                                        .when(is_filtered, |row| row.pl_3())
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
