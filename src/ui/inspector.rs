use super::{GitOutApp, Selection, dialogs};
use crate::git::{FileChange, format_full_date};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffMode};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

pub fn render_inspector(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg_muted = hex_color(gh_colors::FG_MUTED);

    let Some(snap) = app.snapshot.as_ref() else {
        return v_flex()
            .size_full()
            .bg(bg)
            .border_l_1()
            .border_color(border)
            .p_4()
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Select a commit or working tree to inspect diffs."),
            )
            .into_any_element();
    };

    match &app.selection {
        Selection::Wip => render_wip_inspector(app, snap, cx).into_any_element(),
        Selection::Commit(hash) => render_commit_inspector(app, snap, hash, cx).into_any_element(),
    }
}

fn render_wip_inspector(
    app: &GitOutApp,
    snap: &crate::git::RepoSnapshot,
    cx: &mut Context<GitOutApp>,
) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let green = hex_color(gh_colors::GREEN_FG);
    let orange = hex_color(gh_colors::ORANGE);
    let blue = hex_color(gh_colors::BLUE);

    let app_entity = cx.entity();
    let has_staged = !snap.staged_files.is_empty();
    let has_unstaged = !snap.unstaged_files.is_empty();
    let can_commit = (has_staged || app.amend_commit)
        && !app.commit_summary_input.read(cx).value().trim().is_empty();

    let selected_file = app.selected_worktree_file.clone();

    v_flex()
        .size_full()
        .bg(panel_bg)
        .border_l_1()
        .border_color(border)
        // Header
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(border)
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::FileDiff).small().text_color(orange))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child("Working Changes"),
                        ),
                )
                .child(
                    Button::new("wip-diff-all")
                        .ghost()
                        .xsmall()
                        .label("All Diffs")
                        .disabled(selected_file.is_none())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.select_worktree_file(None, cx);
                        })),
                ),
        )
        // Unstaged & Staged File Lists (scrollable upper half)
        .child(
            div()
                .id("wip-files-scroll")
                .max_h(px(235.))
                .min_h(px(110.))
                .overflow_y_scrollbar()
                .border_b_1()
                .border_color(border)
                .p_2()
                .child(
                    v_flex()
                        .gap_2()
                        // Unstaged section
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .px_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(orange)
                                        .child(format!(
                                            "UNSTAGED CHANGES ({})",
                                            snap.unstaged_files.len()
                                        )),
                                )
                                .when(has_unstaged, |row| {
                                    row.child(
                                        Button::new("stage-all-btn")
                                            .xsmall()
                                            .icon(IconName::Plus)
                                            .label("Stage All")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.stage_all(cx);
                                            })),
                                    )
                                }),
                        )
                        .when(!has_unstaged, |col| {
                            col.child(
                                div()
                                    .px_2()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("No unstaged files."),
                            )
                        })
                        .children(snap.unstaged_files.iter().enumerate().map(|(i, file)| {
                            render_wip_file_row(
                                ("unstaged-f", i),
                                file.clone(),
                                selected_file.as_ref() == Some(file),
                                app_entity.clone(),
                                fg,
                                fg_muted,
                                orange,
                                blue,
                            )
                        }))
                        // Staged section
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .px_1()
                                .pt_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(green)
                                        .child(format!(
                                            "STAGED CHANGES ({})",
                                            snap.staged_files.len()
                                        )),
                                )
                                .when(has_staged, |row| {
                                    row.child(
                                        Button::new("unstage-all-btn")
                                            .xsmall()
                                            .label("Unstage All")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.unstage_all(cx);
                                            })),
                                    )
                                }),
                        )
                        .when(!has_staged, |col| {
                            col.child(
                                div()
                                    .px_2()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("Nothing staged yet."),
                            )
                        })
                        .children(snap.staged_files.iter().enumerate().map(|(i, file)| {
                            render_wip_file_row(
                                ("staged-f", i),
                                file.clone(),
                                selected_file.as_ref() == Some(file),
                                app_entity.clone(),
                                fg,
                                fg_muted,
                                green,
                                blue,
                            )
                        })),
                ),
        )
        // Commit composer
        .child(
            v_flex()
                .w_full()
                .p_2p5()
                .gap_2()
                .border_b_1()
                .border_color(border)
                .bg(canvas_bg)
                .child(
                    Input::new(&app.commit_summary_input)
                        .id("commit-summary")
                        .small()
                        .w_full(),
                )
                .child(
                    Textarea::new(&app.commit_body_input)
                        .h(px(64.))
                        .w_full(),
                )
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            Button::new("toggle-amend")
                                .xsmall()
                                .outline()
                                .label(if app.amend_commit {
                                    "Amend: ON"
                                } else {
                                    "Amend"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.amend_commit = !this.amend_commit;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("btn-commit")
                                .primary()
                                .small()
                                .icon(IconName::Check)
                                .label(if app.amend_commit {
                                    "Amend Commit"
                                } else {
                                    "Commit Changes"
                                })
                                .disabled(!can_commit)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit_commit(window, cx);
                                })),
                        ),
                ),
        )
        // Diff viewer
        .child(render_diff_pane(app, cx))
}

#[allow(clippy::too_many_arguments)]
fn render_wip_file_row(
    id: (&'static str, usize),
    file: FileChange,
    is_selected: bool,
    app_entity: gpui_kit::Entity<GitOutApp>,
    fg: gpui_kit::Hsla,
    fg_muted: gpui_kit::Hsla,
    badge_color: gpui_kit::Hsla,
    blue: gpui_kit::Hsla,
) -> impl IntoElement {
    let f_select = file.clone();
    let f_action = file.clone();
    let f_discard = file.clone();
    let app_sel = app_entity.clone();

    h_flex()
        .id(id)
        .w_full()
        .px_2()
        .py_1()
        .gap_1p5()
        .rounded(px(5.))
        .items_center()
        .justify_between()
        .bg(if is_selected {
            blue.opacity(0.15)
        } else {
            hex_color(gh_colors::CANVAS_DEFAULT)
        })
        .border_1()
        .border_color(if is_selected {
            blue.opacity(0.5)
        } else {
            hex_color(gh_colors::BORDER_MUTED)
        })
        .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
        .child(
            h_flex()
                .id((id.0, id.1 * 10 + 9))
                .flex_1()
                .min_w_0()
                .gap_1p5()
                .items_center()
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    let fc = f_select.clone();
                    app_sel.update(cx, |app, cx| {
                        app.select_worktree_file(Some(fc), cx);
                    });
                })
                .child(
                    div()
                        .w(px(16.))
                        .text_center()
                        .text_xs()
                        .font_family("monospace")
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(badge_color)
                        .child(file.kind.badge()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(if is_selected { fg } else { fg_muted })
                        .truncate()
                        .child(file.path.clone()),
                ),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_shrink_0()
                .when(!file.staged, |acts| {
                    let path_stage = f_action.path.clone();
                    let app_st = app_entity.clone();
                    let app_disc = app_entity.clone();
                    let f_disc = f_discard.clone();
                    acts.child(
                        Button::new((id.0, id.1 * 10 + 1))
                            .xsmall()
                            .label("Stage")
                            .on_click(move |_, _, cx| {
                                let p = path_stage.clone();
                                app_st.update(cx, |app, cx| {
                                    app.stage_file(p, cx);
                                });
                            }),
                    )
                    .child(
                        Button::new((id.0, id.1 * 10 + 2))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Trash)
                            .tooltip("Discard changes")
                            .on_click(move |_, window, cx| {
                                let target = f_disc.clone();
                                let app_d = app_disc.clone();
                                let desc = format!(
                                    "Discard local changes to `{}`? This cannot be undone.",
                                    target.path
                                );
                                dialogs::open_confirm_danger(
                                    "Discard File Changes?".to_string(),
                                    desc,
                                    "Discard".to_string(),
                                    window,
                                    cx,
                                    Rc::new(move |_, cx| {
                                        let fc = target.clone();
                                        app_d.update(cx, |app, cx| {
                                            app.discard_file(fc, cx);
                                        });
                                    }),
                                );
                            }),
                    )
                })
                .when(file.staged, |acts| {
                    let path_unstage = f_action.path.clone();
                    let app_unst = app_entity.clone();
                    acts.child(
                        Button::new((id.0, id.1 * 10 + 3))
                            .xsmall()
                            .label("Unstage")
                            .on_click(move |_, _, cx| {
                                let p = path_unstage.clone();
                                app_unst.update(cx, |app, cx| {
                                    app.unstage_file(p, cx);
                                });
                            }),
                    )
                }),
        )
}

fn render_commit_inspector(
    app: &GitOutApp,
    snap: &crate::git::RepoSnapshot,
    hash: &str,
    cx: &mut Context<GitOutApp>,
) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let blue = hex_color(gh_colors::BLUE);
    let green = hex_color(gh_colors::GREEN_FG);
    let orange = hex_color(gh_colors::ORANGE);

    let commit = snap.commits.iter().find(|c| c.hash == hash);
    let detail = app
        .commit_detail
        .as_ref()
        .filter(|d| d.hash == hash);

    let app_entity = cx.entity();
    let hash_str = hash.to_string();
    let short = hash.chars().take(7).collect::<String>();

    v_flex()
        .size_full()
        .bg(panel_bg)
        .border_l_1()
        .border_color(border)
        // Commit metadata + chunky actions
        .child(
            v_flex()
                .w_full()
                .p_3()
                .gap_2()
                .border_b_1()
                .border_color(border)
                .bg(canvas_bg)
                .when_some(commit, |col, c| {
                    let sha_copy = c.hash.clone();
                    col.child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(fg)
                                    .child(c.subject.clone()),
                            )
                            .child(
                                Button::new("copy-sha")
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .label(short.clone())
                                    .tooltip("Copy full commit SHA")
                                    .on_click(move |_, window, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            sha_copy.clone(),
                                        ));
                                        super::notify_info(window, cx, "Copied SHA to clipboard");
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(fg_muted)
                            .child(format!(
                                "{} <{}> · {}",
                                c.author,
                                c.author_email,
                                format_full_date(c.timestamp)
                            )),
                    )
                })
                .when_some(detail, |col, d| {
                    let rest = d
                        .body
                        .lines()
                        .skip(1)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string();
                    col.when(!rest.is_empty(), |col| {
                        col.child(
                            div()
                                .p_2()
                                .rounded(px(5.))
                                .bg(panel_bg)
                                .border_1()
                                .border_color(hex_color(gh_colors::BORDER_MUTED))
                                .text_xs()
                                .text_color(fg_muted)
                                .child(rest),
                        )
                    })
                })
                // Chunky Git action buttons
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1p5()
                        .pt_1()
                        .child({
                            let h = hash_str.clone();
                            Button::new("act-checkout")
                                .xsmall()
                                .icon(IconName::Check)
                                .label("Checkout")
                                .tooltip("Checkout this commit (detached HEAD)")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.checkout_target(h.clone(), cx);
                                }))
                        })
                        .child({
                            let h = hash_str.clone();
                            let app_br = app_entity.clone();
                            Button::new("act-branch-here")
                                .xsmall()
                                .icon(IconName::GitBranchPlus)
                                .label("Branch")
                                .tooltip("Create new branch at this commit")
                                .on_click(move |_, window, cx| {
                                    dialogs::open_create_branch_dialog(
                                        app_br.clone(),
                                        Some(h.clone()),
                                        window,
                                        cx,
                                    );
                                })
                        })
                        .child({
                            let h = hash_str.clone();
                            Button::new("act-cherry-pick")
                                .xsmall()
                                .icon(IconName::GitCommitHorizontal)
                                .label("Cherry-pick")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.cherry_pick(h.clone(), cx);
                                }))
                        })
                        .child({
                            let h = hash_str.clone();
                            Button::new("act-revert")
                                .xsmall()
                                .icon(IconName::RotateCcw)
                                .label("Revert")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.revert_commit(h.clone(), cx);
                                }))
                        })
                        .child({
                            let h = hash_str.clone();
                            let sh = short.clone();
                            let app_rst = app_entity.clone();
                            Button::new("act-reset")
                                .xsmall()
                                .danger()
                                .label("Reset…")
                                .on_click(move |_, window, cx| {
                                    dialogs::open_reset_dialog(
                                        app_rst.clone(),
                                        h.clone(),
                                        sh.clone(),
                                        window,
                                        cx,
                                    );
                                })
                        }),
                ),
        )
        // Changed files selector
        .when_some(detail, |col, d| {
            let selected_path = app.selected_commit_file.clone();
            col.child(
                div()
                    .id("commit-files-scroll")
                    .max_h(px(165.))
                    .overflow_y_scrollbar()
                    .border_b_1()
                    .border_color(border)
                    .p_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .px_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(fg_muted)
                                            .child(format!("CHANGED FILES ({})", d.files.len())),
                                    )
                                    .child(
                                        Button::new("commit-diff-all")
                                            .ghost()
                                            .xsmall()
                                            .label("Show All")
                                            .disabled(selected_path.is_none())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.select_commit_file(None, cx);
                                            })),
                                    ),
                            )
                            .children(d.files.iter().enumerate().map(|(i, f)| {
                                let is_sel = selected_path.as_deref() == Some(f.path.as_str());
                                let path_click = f.path.clone();
                                let badge_c = match f.kind {
                                    crate::git::ChangeKind::Added => green,
                                    crate::git::ChangeKind::Deleted => hex_color(gh_colors::RED_FG),
                                    _ => orange,
                                };
                                h_flex()
                                    .id(("commit-file", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .gap_1p5()
                                    .rounded(px(4.))
                                    .items_center()
                                    .cursor_pointer()
                                    .bg(if is_sel {
                                        blue.opacity(0.15)
                                    } else {
                                        canvas_bg
                                    })
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_commit_file(Some(path_click.clone()), cx);
                                    }))
                                    .child(
                                        div()
                                            .w(px(14.))
                                            .text_xs()
                                            .font_family("monospace")
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(badge_c)
                                            .child(f.kind.badge()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(if is_sel { fg } else { fg_muted })
                                            .truncate()
                                            .child(f.path.clone()),
                                    )
                            })),
                    ),
            )
        })
        // Diff viewer
        .child(render_diff_pane(app, cx))
}

fn render_diff_pane(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let fg = hex_color(gh_colors::FG);

    let is_split = app.diff_mode == DiffMode::Split;
    let has_parsed_files = !app.inspector_diff_state.read(cx).files().is_empty();

    v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .bg(canvas_bg)
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_1()
                .bg(subtle_bg)
                .border_b_1()
                .border_color(border)
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(fg_muted)
                        .child("DIFF"),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("diff-unified")
                                .xsmall()
                                .icon(IconName::Rows2)
                                .label("Unified")
                                .disabled(!is_split)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_diff_mode(DiffMode::Unified, cx);
                                })),
                        )
                        .child(
                            Button::new("diff-split")
                                .xsmall()
                                .icon(IconName::Columns2)
                                .label("Split")
                                .disabled(is_split)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_diff_mode(DiffMode::Split, cx);
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .when(has_parsed_files, |area| {
                    area.child(
                        Diff::new(&app.inspector_diff_state)
                            .line_number(true)
                            .flex_1()
                            .size_full(),
                    )
                })
                .when(!has_parsed_files, |area| {
                    if app.inspector_raw_patch.trim().is_empty() {
                        area.child(
                            div()
                                .p_4()
                                .text_xs()
                                .text_color(fg_muted)
                                .child("No diff content to display."),
                        )
                    } else {
                        area.child(
                            div()
                                .id("raw-patch-fallback")
                                .size_full()
                                .overflow_y_scrollbar()
                                .p_3()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(fg)
                                .child(app.inspector_raw_patch.clone()),
                        )
                    }
                }),
        )
}
