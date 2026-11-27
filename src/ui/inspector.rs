use super::{
    GitOutApp, Selection, dialogs,
    history_dialog::{FileInspectorMode, open_file_inspector_dialog},
    reflog_dialog,
};
use crate::git::patch::{build_hunk_patch, build_patch_for_diff_range};
use crate::git::{ChangeKind, FileChange, format_full_date};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffMode, DiffSide};
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
                    .child("Select a commit, stash, or working tree to inspect diffs."),
            )
            .into_any_element();
    };

    match &app.selection {
        Selection::Wip => render_wip_inspector(app, snap, cx).into_any_element(),
        Selection::Commit(hash) => render_commit_inspector(app, snap, hash, cx).into_any_element(),
        Selection::Stash(reference) => render_stash_inspector(app, reference, cx).into_any_element(),
        Selection::Compare { base, head } => {
            render_compare_inspector(app, base, head, cx).into_any_element()
        }
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
    let red = hex_color(gh_colors::RED_FG);
    let blue = hex_color(gh_colors::BLUE);

    let app_entity = cx.entity();
    let has_staged = !snap.staged_files.is_empty();
    let has_unstaged = !snap.unstaged_files.is_empty();
    let can_undo = !snap.commits.is_empty();
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
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child({
                            let app_discard = app_entity.clone();
                            let dirty = snap.is_dirty();
                            Button::new("wip-discard-all")
                                .danger()
                                .xsmall()
                                .icon(IconName::Trash)
                                .label("Discard All")
                                .tooltip("Discard all staged, unstaged, and untracked changes")
                                .disabled(!dirty)
                                .on_click(move |_, window, cx| {
                                    let app = app_discard.clone();
                                    dialogs::open_confirm_danger(
                                        "Discard All Working Changes".into(),
                                        "Permanently discard all staged, unstaged, and untracked changes in the working tree? This cannot be undone.".into(),
                                        "Discard All".into(),
                                        window,
                                        cx,
                                        Rc::new(move |_, cx| {
                                            app.update(cx, |a, cx| a.discard_all_changes(cx));
                                        }),
                                    );
                                })
                        })
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
                ),
        )
        // Unstaged & Staged File Lists
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
                                    let app_discard_unstaged = app_entity.clone();
                                    row.child(
                                        h_flex()
                                            .gap_1()
                                            .items_center()
                                            .child(
                                                Button::new("discard-unstaged-btn")
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Trash)
                                                    .tooltip("Discard all unstaged and untracked changes")
                                                    .on_click(move |_, window, cx| {
                                                        let app = app_discard_unstaged.clone();
                                                        dialogs::open_confirm_danger(
                                                            "Discard Unstaged Changes".into(),
                                                            "Permanently discard all unstaged modifications and delete untracked files? Staged changes will be preserved.".into(),
                                                            "Discard Unstaged".into(),
                                                            window,
                                                            cx,
                                                            Rc::new(move |_, cx| {
                                                                app.update(cx, |a, cx| a.discard_all_unstaged(cx));
                                                            }),
                                                        );
                                                    }),
                                            )
                                            .child(
                                                Button::new("stage-all-btn")
                                                    .xsmall()
                                                    .icon(IconName::Plus)
                                                    .label("Stage All")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.stage_all(cx);
                                                    })),
                                            ),
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
                            let bcol = if file.kind == ChangeKind::Conflicted {
                                red
                            } else {
                                orange
                            };
                            render_wip_file_row(
                                ("unstaged-f", i),
                                file.clone(),
                                selected_file.as_ref() == Some(file),
                                app_entity.clone(),
                                fg,
                                fg_muted,
                                bcol,
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
                        .h(px(62.))
                        .w_full(),
                )
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_1()
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
                                            this.toggle_amend_commit(cx);
                                        })),
                                )
                                .child(
                                    Button::new("toggle-sign")
                                        .xsmall()
                                        .outline()
                                        .icon(IconName::BadgeCheck)
                                        .label(if app.sign_commit {
                                            "Sign: ON"
                                        } else {
                                            "Sign"
                                        })
                                        .tooltip("GPG/SSH sign commit (git commit -S)")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sign_commit = !this.sign_commit;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("btn-undo-commit")
                                        .xsmall()
                                        .ghost()
                                        .icon(IconName::Undo2)
                                        .label("Undo Last")
                                        .disabled(!can_undo)
                                        .tooltip("Soft-reset HEAD~1 and keep changes staged")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.undo_last_commit(cx);
                                        })),
                                ),
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
        // Conflict resolution bar OR Hunk / Line Staging bar
        .child(render_wip_context_bar(app, cx))
        // Diff viewer
        .child(render_diff_pane(app, cx))
}

fn render_wip_context_bar(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let red = hex_color(gh_colors::RED_FG);
    let orange = hex_color(gh_colors::ORANGE);
    let green = hex_color(gh_colors::GREEN_FG);
    let blue = hex_color(gh_colors::BLUE);

    // 1. Active Conflict Resolver Bar
    if let Some(c_path) = app.selected_conflict_path.clone() {
        let diff_state = app.inspector_diff_state.read(cx);
        let total = diff_state
            .files()
            .first()
            .map(|f| f.conflicts().len())
            .unwrap_or(0);
        let resolved_cnt = (0..total)
            .filter(|&i| diff_state.conflict_resolution(&c_path, i).is_some())
            .count();

        let p_ours = c_path.clone();
        let p_theirs = c_path.clone();
        let p_save = c_path.clone();

        return h_flex()
            .w_full()
            .px_3()
            .py_1p5()
            .gap_2()
            .bg(red.opacity(0.12))
            .border_b_1()
            .border_color(red.opacity(0.4))
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(Icon::new(IconName::TriangleAlert).xsmall().text_color(red))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child(format!(
                                "Conflict: {resolved_cnt}/{total} blocks resolved — click Accept in diff below"
                            )),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_shrink_0()
                    .child(
                        Button::new("c-ours")
                            .xsmall()
                            .label("Use Ours")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.resolve_conflict_ours(p_ours.clone(), cx);
                            })),
                    )
                    .child(
                        Button::new("c-theirs")
                            .xsmall()
                            .label("Use Theirs")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.resolve_conflict_theirs(p_theirs.clone(), cx);
                            })),
                    )
                    .child(
                        Button::new("c-save-resolved")
                            .xsmall()
                            .primary()
                            .icon(IconName::Check)
                            .label("Save & Stage")
                            .disabled(total == 0 || resolved_cnt < total)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save_resolved_conflict(p_save.clone(), cx);
                            })),
                    ),
            )
            .into_any_element();
    }

    // 2. Single-file Hunk & Selected-Lines Staging Bar
    let Some(sel_file) = app.selected_worktree_file.as_ref() else {
        return div().into_any_element();
    };
    if sel_file.kind == ChangeKind::Untracked {
        return div().into_any_element();
    }
    let Some(parsed_file) = app.parsed_wip_patch.first() else {
        return div().into_any_element();
    };
    if parsed_file.hunks.is_empty() {
        return div().into_any_element();
    }

    let is_staged = sel_file.staged;
    let selected_range = app.inspector_diff_state.read(cx).selected_lines();
    let app_entity = cx.entity();

    let line_patch = selected_range.as_ref().and_then(|r| {
        let s_old = r.side() == DiffSide::Original;
        let e_old = r.end_side() == DiffSide::Original;
        build_patch_for_diff_range(parsed_file, s_old, r.start(), e_old, r.end(), is_staged)
    });

    h_flex()
        .w_full()
        .px_2p5()
        .py_1p5()
        .gap_1p5()
        .flex_wrap()
        .bg(hex_color(gh_colors::CANVAS_INSET))
        .border_b_1()
        .border_color(border)
        .items_center()
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(fg_muted)
                .child("Hunks:"),
        )
        .children(parsed_file.hunks.iter().enumerate().map(|(hi, hunk)| {
            let hunk_patch = build_hunk_patch(parsed_file, hi);
            let h_stage = hunk_patch.clone();
            let h_disc = hunk_patch;
            let app_st = app_entity.clone();
            let app_dc = app_entity.clone();
            let line_hint = if is_staged {
                hunk.new_start
            } else {
                hunk.old_start
            };

            h_flex()
                .gap_0p5()
                .px_1()
                .py(px(2.))
                .rounded(px(4.))
                .bg(hex_color(gh_colors::CANVAS_SUBTLE))
                .border_1()
                .border_color(border)
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(fg_muted)
                        .pr_1()
                        .child(format!("#{} L{line_hint}", hi + 1)),
                )
                .when_some(h_stage, |row, p| {
                    row.child(
                        Button::new(("hunk-act", hi))
                            .xsmall()
                            .label(if is_staged { "Unstage" } else { "Stage" })
                            .on_click(move |_, _, cx| {
                                let patch = p.clone();
                                app_st.update(cx, |app, cx| {
                                    app.stage_patch(
                                        patch,
                                        is_staged,
                                        if is_staged {
                                            "Unstaged hunk"
                                        } else {
                                            "Staged hunk"
                                        },
                                        cx,
                                    );
                                });
                            }),
                    )
                })
                .when(!is_staged, |row| {
                    row.when_some(h_disc, |r2, p| {
                        r2.child(
                            Button::new(("hunk-disc", hi))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Trash)
                                .tooltip("Discard this hunk")
                                .on_click(move |_, window, cx| {
                                    let patch = p.clone();
                                    let a = app_dc.clone();
                                    dialogs::open_confirm_danger(
                                        format!("Discard Hunk #{}?", hi + 1),
                                        "Reverts only this hunk in the working tree. Cannot be undone.".to_string(),
                                        "Discard Hunk".to_string(),
                                        window,
                                        cx,
                                        Rc::new(move |_, cx| {
                                            let pt = patch.clone();
                                            a.update(cx, |app, cx| {
                                                app.discard_patch(pt, cx);
                                            });
                                        }),
                                    );
                                }),
                        )
                    })
                })
        }))
        .when_some(line_patch.as_ref().map(|lp| {
            let r = selected_range.as_ref().unwrap();
            (lp.clone(), r.start().min(r.end()), r.start().max(r.end()))
        }), |row, (lp, lo, hi)| {
            let app_ln = app_entity.clone();
            row.child(
                Button::new("btn-stage-lines")
                    .xsmall()
                    .primary()
                    .icon(IconName::Check)
                    .label(if is_staged {
                        format!("Unstage Lines (L{lo}–L{hi})")
                    } else {
                        format!("Stage Lines (L{lo}–L{hi})")
                    })
                    .on_click(move |_, _, cx| {
                        let patch = lp.clone();
                        app_ln.update(cx, |app, cx| {
                            app.stage_patch(
                                patch,
                                is_staged,
                                if is_staged {
                                    "Unstaged selected lines"
                                } else {
                                    "Staged selected lines"
                                },
                                cx,
                            );
                        });
                    }),
            )
        })
        .when(selected_range.is_none(), |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(blue.opacity(0.85))
                    .child("Tip: click line numbers in diff to stage/unstage lines")
                    .when(parsed_file.hunks.is_empty(), |d| d.text_color(green))
                    .when(is_staged, |d| d.text_color(orange)),
            )
        })
        .into_any_element()
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
    let is_tracked = file.kind != ChangeKind::Untracked;
    let p_hist = file.path.clone();
    let p_blame = file.path.clone();
    let app_hist = app_entity.clone();
    let app_blame = app_entity.clone();

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
                .gap_0p5()
                .flex_shrink_0()
                .when(is_tracked, |acts| {
                    acts.child(
                        Button::new((id.0, id.1 * 10 + 7))
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileClock)
                            .tooltip("File History")
                            .on_click(move |_, window, cx| {
                                open_file_inspector_dialog(
                                    app_hist.clone(),
                                    p_hist.clone(),
                                    FileInspectorMode::History,
                                    None,
                                    window,
                                    cx,
                                );
                            }),
                    )
                    .child(
                        Button::new((id.0, id.1 * 10 + 8))
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileSearch)
                            .tooltip("Git Blame")
                            .on_click(move |_, window, cx| {
                                open_file_inspector_dialog(
                                    app_blame.clone(),
                                    p_blame.clone(),
                                    FileInspectorMode::Blame,
                                    None,
                                    window,
                                    cx,
                                );
                            }),
                    )
                })
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
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1p5()
                                    .items_center()
                                    .when(c.signature_status == 'G', |r| {
                                        r.child(
                                            Icon::new(IconName::BadgeCheck)
                                                .small()
                                                .text_color(green),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(fg)
                                            .child(c.subject.clone()),
                                    ),
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
                // Action buttons
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1()
                        .pt_1()
                        .child({
                            let h = hash_str.clone();
                            Button::new("act-checkout")
                                .xsmall()
                                .icon(IconName::Check)
                                .label("Checkout")
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
                            let sh = short.clone();
                            let app_tg = app_entity.clone();
                            Button::new("act-tag-here")
                                .xsmall()
                                .icon(IconName::Tag)
                                .label("Tag")
                                .on_click(move |_, window, cx| {
                                    reflog_dialog::open_create_tag_dialog(
                                        app_tg.clone(),
                                        h.clone(),
                                        sh.clone(),
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
                            Button::new("act-rebase-i")
                                .xsmall()
                                .icon(IconName::GitBranch)
                                .label("Rebase -i")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_interactive_rebase_from(h.clone(), sh.clone(), cx);
                                }))
                        })
                        .child({
                            let h = hash_str.clone();
                            let sh = short.clone();
                            let cmp_base = app.compare_base.clone();
                            Button::new("act-compare")
                                .xsmall()
                                .icon(IconName::GitCompare)
                                .label(if let Some((_, ref lbl)) = cmp_base {
                                    format!("vs {lbl}")
                                } else {
                                    "Compare".to_string()
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some((base_h, _)) = cmp_base.clone() {
                                        this.compare_commits(base_h, h.clone(), cx);
                                    } else {
                                        this.set_compare_base(h.clone(), sh.clone(), cx);
                                    }
                                }))
                        })
                        .child({
                            let h = hash_str.clone();
                            Button::new("act-copy-patch")
                                .xsmall()
                                .icon(IconName::FileDiff)
                                .label("Patch")
                                .tooltip("Copy `git format-patch`")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy_commit_patch(h.clone(), cx);
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
                                let p_hist = f.path.clone();
                                let p_blame = f.path.clone();
                                let rev_blame = hash_str.clone();
                                let app_h = app_entity.clone();
                                let app_b = app_entity.clone();
                                let badge_c = match f.kind {
                                    ChangeKind::Added => green,
                                    ChangeKind::Deleted => hex_color(gh_colors::RED_FG),
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
                                    .justify_between()
                                    .bg(if is_sel {
                                        blue.opacity(0.15)
                                    } else {
                                        canvas_bg
                                    })
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .child(
                                        h_flex()
                                            .id(("cf-sel", i))
                                            .flex_1()
                                            .min_w_0()
                                            .gap_1p5()
                                            .items_center()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_commit_file(
                                                    Some(path_click.clone()),
                                                    cx,
                                                );
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
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_0p5()
                                            .flex_shrink_0()
                                            .child(
                                                Button::new(("cf-hist", i))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::FileClock)
                                                    .tooltip("File History")
                                                    .on_click(move |_, window, cx| {
                                                        open_file_inspector_dialog(
                                                            app_h.clone(),
                                                            p_hist.clone(),
                                                            FileInspectorMode::History,
                                                            None,
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                            .child(
                                                Button::new(("cf-blame", i))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::FileSearch)
                                                    .tooltip("Blame at this commit")
                                                    .on_click(move |_, window, cx| {
                                                        open_file_inspector_dialog(
                                                            app_b.clone(),
                                                            p_blame.clone(),
                                                            FileInspectorMode::Blame,
                                                            Some(rev_blame.clone()),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            ),
                                    )
                            })),
                    ),
            )
        })
        // Diff viewer
        .child(render_diff_pane(app, cx))
}

fn render_stash_inspector(
    app: &GitOutApp,
    reference: &str,
    cx: &mut Context<GitOutApp>,
) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let yellow = hex_color(gh_colors::YELLOW);

    let detail = app
        .stash_detail
        .as_ref()
        .filter(|d| d.reference == reference);
    let r_apply = reference.to_string();
    let r_pop = reference.to_string();
    let r_br = reference.to_string();
    let r_drop = reference.to_string();
    let app_entity = cx.entity();

    v_flex()
        .size_full()
        .bg(panel_bg)
        .border_l_1()
        .border_color(border)
        .child(
            v_flex()
                .w_full()
                .p_3()
                .gap_2()
                .bg(canvas_bg)
                .border_b_1()
                .border_color(border)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(IconName::Archive).small().text_color(yellow))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(fg)
                                        .child(reference.to_string()),
                                ),
                        )
                        .child(
                            Button::new("stash-back-wip")
                                .ghost()
                                .xsmall()
                                .label("Back to WIP")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.select_wip(cx);
                                })),
                        ),
                )
                .when_some(detail, |col, d| {
                    col.child(
                        div()
                            .text_xs()
                            .text_color(fg_muted)
                            .child(format!("{} · {} files", d.message, d.files.len())),
                    )
                })
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(
                            Button::new("st-insp-apply")
                                .xsmall()
                                .label("Apply")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.stash_apply(r_apply.clone(), cx);
                                })),
                        )
                        .child(
                            Button::new("st-insp-pop")
                                .xsmall()
                                .primary()
                                .label("Pop")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.stash_pop(Some(r_pop.clone()), cx);
                                })),
                        )
                        .child(
                            Button::new("st-insp-branch")
                                .xsmall()
                                .icon(IconName::GitBranchPlus)
                                .label("Branch…")
                                .on_click(move |_, window, cx| {
                                    dialogs::open_branch_from_stash_dialog(
                                        app_entity.clone(),
                                        r_br.clone(),
                                        window,
                                        cx,
                                    );
                                }),
                        )
                        .child({
                            let app_d = cx.entity();
                            Button::new("st-insp-drop")
                                .xsmall()
                                .danger()
                                .label("Drop")
                                .on_click(move |_, window, cx| {
                                    let r = r_drop.clone();
                                    let a = app_d.clone();
                                    dialogs::open_confirm_danger(
                                        format!("Drop {r}?"),
                                        "Permanently delete this stash?".to_string(),
                                        "Drop".to_string(),
                                        window,
                                        cx,
                                        Rc::new(move |_, cx| {
                                            let rf = r.clone();
                                            a.update(cx, |app, cx| app.stash_drop(rf, cx));
                                        }),
                                    );
                                })
                        }),
                ),
        )
        .child(render_diff_pane(app, cx))
}

fn render_compare_inspector(
    app: &GitOutApp,
    base: &str,
    head: &str,
    cx: &mut Context<GitOutApp>,
) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let blue = hex_color(gh_colors::BLUE);

    let detail = app.compare_detail.as_ref();
    let short_b: String = detail
        .map(|d| d.base.as_str())
        .unwrap_or(base)
        .chars()
        .take(7)
        .collect();
    let short_h: String = detail
        .map(|d| d.head.as_str())
        .unwrap_or(head)
        .chars()
        .take(7)
        .collect();
    let file_count = detail.map(|d| d.files.len()).unwrap_or(0);
    let swap_b = head.to_string();
    let swap_h = base.to_string();

    v_flex()
        .size_full()
        .bg(panel_bg)
        .border_l_1()
        .border_color(border)
        .child(
            v_flex()
                .w_full()
                .p_3()
                .gap_2()
                .bg(canvas_bg)
                .border_b_1()
                .border_color(border)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Icon::new(IconName::GitCompareArrows)
                                        .small()
                                        .text_color(blue),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(fg)
                                        .child(format!("Compare {short_b} .. {short_h}")),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("cmp-swap")
                                        .xsmall()
                                        .label("Swap")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.compare_commits(
                                                swap_b.clone(),
                                                swap_h.clone(),
                                                cx,
                                            );
                                        })),
                                )
                                .child(
                                    Button::new("cmp-close")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::X)
                                        .label("Close")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.clear_compare_base(cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(fg_muted)
                        .child(format!("{file_count} changed files across range")),
                ),
        )
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
                            Button::new("diff-ws")
                                .xsmall()
                                .outline()
                                .label(if app.ignore_whitespace {
                                    "-w: ON"
                                } else {
                                    "-w"
                                })
                                .tooltip("Ignore whitespace changes (`git diff -w`)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_ignore_whitespace(cx);
                                })),
                        )
                        .child(
                            Button::new("diff-wrap")
                                .xsmall()
                                .outline()
                                .icon(IconName::TextWrap)
                                .label(if app.soft_wrap_diff {
                                    "Wrap: ON"
                                } else {
                                    "Wrap"
                                })
                                .tooltip("Toggle soft-wrap in diff viewer")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_soft_wrap(cx);
                                })),
                        )
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
                            .soft_wrap(app.soft_wrap_diff)
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
