use super::GitOutApp;
use crate::git::{self, BlameLine, FileHistoryEntry, format_relative_time};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffParseOptions, DiffState};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Entity, Focusable as _, InteractiveElement as _, ParentElement,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileInspectorMode {
    Blame,
    History,
}

#[derive(Clone)]
struct InspectorModalState {
    mode: FileInspectorMode,
    blame_lines: Vec<BlameLine>,
    blame_loaded: bool,
    history: Vec<FileHistoryEntry>,
    history_loaded: bool,
    selected_commit: Option<String>,
    loading: bool,
    error: Option<String>,
}

pub fn open_file_inspector_dialog(
    app_entity: Entity<GitOutApp>,
    path: String,
    initial_mode: FileInspectorMode,
    rev: Option<String>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let Some(root) = app_entity.read(cx).repo_root.clone() else {
        return;
    };

    let diff_state: Entity<DiffState> =
        cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Unified));

    let state = Rc::new(RefCell::new(InspectorModalState {
        mode: initial_mode,
        blame_lines: Vec::new(),
        blame_loaded: false,
        history: Vec::new(),
        history_loaded: false,
        selected_commit: None,
        loading: true,
        error: None,
    }));

    // Initial background fetch for both Blame and History so switching tabs is instant.
    {
        let state_bg = state.clone();
        let path_bg = path.clone();
        let rev_bg = rev.clone();
        let root_bg = root.clone();
        let diff_bg = diff_state.clone();
        let win_handle = window.window_handle();

        cx.spawn(async move |cx| {
            let p1 = path_bg.clone();
            let r1 = root_bg.clone();
            let rv1 = rev_bg.clone();
            let p2 = path_bg.clone();
            let r2 = root_bg.clone();

            let (blame_res, hist_res) = cx
                .background_executor()
                .spawn(async move {
                    let (b, _) = git::load_file_blame(&r1, &p1, rv1.as_deref());
                    let (h, _) = git::load_file_history(&r2, &p2, 120);
                    (b, h)
                })
                .await;

            let first_commit = hist_res
                .as_ref()
                .ok()
                .and_then(|v| v.first())
                .map(|e| (e.hash.clone(), e.file_path.clone()));

            let first_patch = if let Some((h, fp)) = first_commit {
                let r3 = root_bg.clone();
                let h_cl = h.clone();
                let fp_cl = fp.clone();
                let patch = cx
                    .background_executor()
                    .spawn(async move {
                        git::load_commit_file_patch(&r3, &h_cl, &fp_cl, false)
                            .0
                            .unwrap_or_default()
                    })
                    .await;
                Some((h, patch))
            } else {
                None
            };

            let _ = win_handle.update(cx, |_, window, cx| {
                {
                    let mut st = state_bg.borrow_mut();
                    st.loading = false;
                    st.blame_loaded = true;
                    st.history_loaded = true;
                    match blame_res {
                        Ok(lines) => st.blame_lines = lines,
                        Err(e) => {
                            if initial_mode == FileInspectorMode::Blame {
                                st.error = Some(e.to_string());
                            }
                        }
                    }
                    match hist_res {
                        Ok(entries) => st.history = entries,
                        Err(e) => {
                            if st.error.is_none() {
                                st.error = Some(e.to_string());
                            }
                        }
                    }
                    if let Some((h, patch)) = first_patch {
                        st.selected_commit = Some(h);
                        let parsed = DiffFile::parse_with(
                            &patch,
                            DiffParseOptions::new().truncated_hunks(true),
                        )
                        .unwrap_or_default();
                        diff_bg.update(cx, |ds, cx| ds.set_files(parsed, cx));
                    }
                }
                window.refresh();
            });
        })
        .detach();
    }

    window.open_dialog(cx, move |dialog, _, cx| {
        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let fg_subtle = hex_color(gh_colors::FG_SUBTLE);
        let border = hex_color(gh_colors::BORDER);
        let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
        let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
        let blue = hex_color(gh_colors::BLUE);

        let snap = state.borrow().clone();
        let state_blame_tab = state.clone();
        let state_hist_tab = state.clone();
        let app_jump_blame = app_entity.clone();
        let app_jump_hist = app_entity.clone();

        dialog
            .title(format!("File Inspector · {path}"))
            .w(px(920.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(
                                        Button::new("fi-tab-blame")
                                            .small()
                                            .icon(IconName::FileSearch)
                                            .label(format!("Blame ({} lines)", snap.blame_lines.len()))
                                            .disabled(snap.mode == FileInspectorMode::Blame)
                                            .on_click(move |_, window, _| {
                                                state_blame_tab.borrow_mut().mode =
                                                    FileInspectorMode::Blame;
                                                window.refresh();
                                            }),
                                    )
                                    .child(
                                        Button::new("fi-tab-history")
                                            .small()
                                            .icon(IconName::FileClock)
                                            .label(format!("History ({} commits)", snap.history.len()))
                                            .disabled(snap.mode == FileInspectorMode::History)
                                            .on_click(move |_, window, _| {
                                                state_hist_tab.borrow_mut().mode =
                                                    FileInspectorMode::History;
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .when_some(
                                snap.selected_commit
                                    .clone()
                                    .filter(|_| snap.mode == FileInspectorMode::History),
                                |row, sel_hash| {
                                    let app_j = app_jump_hist.clone();
                                    row.child(
                                        Button::new("fi-jump-commit")
                                            .xsmall()
                                            .icon(IconName::GitCommitHorizontal)
                                            .label("Jump to Commit in Graph")
                                            .on_click(move |_, window, cx| {
                                                let h = sel_hash.clone();
                                                window.close_dialog(cx);
                                                app_j.update(cx, |app, cx| {
                                                    app.jump_to_commit(&h, cx);
                                                });
                                            }),
                                    )
                                },
                            ),
                    )
                    .when(snap.loading, |col| {
                        col.child(
                            div()
                                .p_4()
                                .text_xs()
                                .text_color(fg_muted)
                                .child("Loading git blame & history…"),
                        )
                    })
                    .when_some(snap.error.clone(), |col, err| {
                        col.child(
                            div()
                                .p_3()
                                .rounded(px(5.))
                                .bg(hex_color(gh_colors::RED_BTN).opacity(0.15))
                                .text_xs()
                                .text_color(hex_color(gh_colors::RED_FG))
                                .child(err),
                        )
                    })
                    .when(!snap.loading && snap.mode == FileInspectorMode::Blame, |col| {
                        col.child(
                            div()
                                .id("blame-scroll")
                                .h(px(460.))
                                .overflow_y_scrollbar()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(border)
                                .bg(canvas_bg)
                                .child(
                                    v_flex().children(snap.blame_lines.iter().enumerate().map(
                                        |(i, bl)| {
                                            let app_j = app_jump_blame.clone();
                                            let h_jump = bl.hash.clone();
                                            let author_short: String =
                                                bl.author.chars().take(12).collect();
                                            let sum_short: String =
                                                bl.summary.chars().take(24).collect();
                                            let rel = format_relative_time(bl.timestamp);
                                            h_flex()
                                                .id(("blame-row", i))
                                                .w_full()
                                                .px_2()
                                                .py(px(2.))
                                                .gap_2()
                                                .items_center()
                                                .hover(|s| s.bg(subtle_bg))
                                                .child(
                                                    div()
                                                        .id(("blame-sha", i))
                                                        .w(px(58.))
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .font_family("monospace")
                                                        .text_color(blue)
                                                        .cursor_pointer()
                                                        .on_click(move |_, window, cx| {
                                                            let h = h_jump.clone();
                                                            window.close_dialog(cx);
                                                            app_j.update(cx, |app, cx| {
                                                                app.jump_to_commit(&h, cx);
                                                            });
                                                        })
                                                        .child(bl.short_hash.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(92.))
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .text_color(fg_muted)
                                                        .truncate()
                                                        .child(author_short),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(140.))
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .text_color(fg_subtle)
                                                        .truncate()
                                                        .child(sum_short),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(52.))
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .text_color(fg_subtle)
                                                        .child(rel),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(40.))
                                                        .flex_shrink_0()
                                                        .text_right()
                                                        .text_xs()
                                                        .font_family("monospace")
                                                        .text_color(fg_subtle)
                                                        .child(format!("{}", bl.line_no)),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .text_xs()
                                                        .font_family("monospace")
                                                        .text_color(fg)
                                                        .child(bl.content.clone()),
                                                )
                                        },
                                    )),
                                ),
                        )
                    })
                    .when(
                        !snap.loading && snap.mode == FileInspectorMode::History,
                        |col| {
                            let has_files = !diff_state.read(cx).files().is_empty();
                            col.child(
                                h_flex()
                                    .h(px(460.))
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("hist-commit-list")
                                            .w(px(300.))
                                            .h_full()
                                            .overflow_y_scrollbar()
                                            .rounded(px(6.))
                                            .border_1()
                                            .border_color(border)
                                            .bg(subtle_bg)
                                            .p_1p5()
                                            .child(v_flex().gap_1().children(
                                                snap.history.iter().enumerate().map(|(i, he)| {
                                                    let is_sel = snap.selected_commit.as_deref()
                                                        == Some(he.hash.as_str());
                                                    let state_sel = state.clone();
                                                    let diff_sel = diff_state.clone();
                                                    let root_sel = root.clone();
                                                    let h_sel = he.hash.clone();
                                                    let fp_sel = he.file_path.clone();

                                                    v_flex()
                                                        .id(("fh-row", i))
                                                        .w_full()
                                                        .p_2()
                                                        .gap_0p5()
                                                        .rounded(px(5.))
                                                        .cursor_pointer()
                                                        .bg(if is_sel {
                                                            blue.opacity(0.16)
                                                        } else {
                                                            canvas_bg
                                                        })
                                                        .border_1()
                                                        .border_color(if is_sel {
                                                            blue.opacity(0.5)
                                                        } else {
                                                            border
                                                        })
                                                        .hover(|s| {
                                                            s.bg(hex_color(gh_colors::BTN_BG))
                                                        })
                                                        .on_click(move |_, window, cx| {
                                                            state_sel.borrow_mut().selected_commit =
                                                                Some(h_sel.clone());
                                                            let win_h = window.window_handle();
                                                            let r = root_sel.clone();
                                                            let h = h_sel.clone();
                                                            let fp = fp_sel.clone();
                                                            let ds = diff_sel.clone();
                                                            cx.spawn(async move |cx| {
                                                                let patch = cx
                                                                    .background_executor()
                                                                    .spawn(async move {
                                                                        git::load_commit_file_patch(
                                                                            &r, &h, &fp, false,
                                                                        )
                                                                        .0
                                                                        .unwrap_or_default()
                                                                    })
                                                                    .await;
                                                                let _ = win_h.update(
                                                                    cx,
                                                                    |_, window, cx| {
                                                                        let parsed =
                                                                            DiffFile::parse_with(
                                                                                &patch,
                                                                                DiffParseOptions::new()
                                                                                    .truncated_hunks(true),
                                                                            )
                                                                            .unwrap_or_default();
                                                                        ds.update(cx, |s, cx| {
                                                                            s.set_files(parsed, cx)
                                                                        });
                                                                        window.refresh();
                                                                    },
                                                                );
                                                            })
                                                            .detach();
                                                        })
                                                        .child(
                                                            h_flex()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_family("monospace")
                                                                        .text_color(blue)
                                                                        .child(
                                                                            he.short_hash.clone(),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(fg_subtle)
                                                                        .child(
                                                                            format_relative_time(
                                                                                he.timestamp,
                                                                            ),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(
                                                                    gpui_kit::FontWeight::SEMIBOLD,
                                                                )
                                                                .text_color(fg)
                                                                .truncate()
                                                                .child(he.subject.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(fg_muted)
                                                                .child(he.author.clone()),
                                                        )
                                                }),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .h_full()
                                            .min_w_0()
                                            .rounded(px(6.))
                                            .border_1()
                                            .border_color(border)
                                            .bg(canvas_bg)
                                            .when(has_files, |d| {
                                                d.child(
                                                    Diff::new(&diff_state)
                                                        .line_number(true)
                                                        .size_full(),
                                                )
                                            })
                                            .when(!has_files, |d| {
                                                d.child(
                                                    div()
                                                        .p_4()
                                                        .text_xs()
                                                        .text_color(fg_muted)
                                                        .child("Select a commit to inspect its patch."),
                                                )
                                            }),
                                    ),
                            )
                        },
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("fi-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

pub fn open_file_finder_dialog(
    app_entity: Entity<GitOutApp>,
    files: Vec<String>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let filter_input =
        cx.new(|cx| InputState::new(window, cx).placeholder("Filter tracked files…"));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            filter_input.focus_handle(cx).focus(window, cx);
        }

        let q = filter_input.read(cx).value().trim().to_lowercase();
        let filtered: Vec<String> = files
            .iter()
            .filter(|f| q.is_empty() || f.to_lowercase().contains(&q))
            .take(150)
            .cloned()
            .collect();

        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);

        dialog
            .title("File History & Blame Finder (Ctrl+Shift+F)")
            .w(px(620.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Input::new(&filter_input)
                            .id("ff-filter")
                            .small()
                            .prefix(Icon::new(IconName::Search).xsmall().text_color(fg_muted))
                            .w_full(),
                    )
                    .child(
                        div()
                            .id("ff-scroll")
                            .max_h(px(360.))
                            .overflow_y_scrollbar()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(border)
                            .bg(card_bg)
                            .p_1p5()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .when(filtered.is_empty(), |c| {
                                        c.child(
                                            div()
                                                .p_3()
                                                .text_xs()
                                                .text_color(fg_muted)
                                                .child("No tracked files match filter."),
                                        )
                                    })
                                    .children(filtered.into_iter().enumerate().map(|(i, path)| {
                                        let app_b = app_entity.clone();
                                        let app_h = app_entity.clone();
                                        let p_b = path.clone();
                                        let p_h = path.clone();

                                        h_flex()
                                            .id(("ff-row", i))
                                            .w_full()
                                            .px_2()
                                            .py_1()
                                            .rounded(px(4.))
                                            .justify_between()
                                            .items_center()
                                            .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_xs()
                                                    .font_family("monospace")
                                                    .text_color(fg)
                                                    .truncate()
                                                    .child(path),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .flex_shrink_0()
                                                    .child(
                                                        Button::new(("ff-blame", i))
                                                            .xsmall()
                                                            .icon(IconName::FileSearch)
                                                            .label("Blame")
                                                            .on_click(move |_, window, cx| {
                                                                window.close_dialog(cx);
                                                                open_file_inspector_dialog(
                                                                    app_b.clone(),
                                                                    p_b.clone(),
                                                                    FileInspectorMode::Blame,
                                                                    None,
                                                                    window,
                                                                    cx,
                                                                );
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new(("ff-hist", i))
                                                            .xsmall()
                                                            .icon(IconName::FileClock)
                                                            .label("History")
                                                            .on_click(move |_, window, cx| {
                                                                window.close_dialog(cx);
                                                                open_file_inspector_dialog(
                                                                    app_h.clone(),
                                                                    p_h.clone(),
                                                                    FileInspectorMode::History,
                                                                    None,
                                                                    window,
                                                                    cx,
                                                                );
                                                            }),
                                                    ),
                                            )
                                    })),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("ff-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}
