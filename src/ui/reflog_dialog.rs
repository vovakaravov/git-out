use super::{GitOutApp, dialogs};
use crate::git::{ReflogEntry, RemoteInfo, format_relative_time};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Entity, Focusable as _, InteractiveElement as _, ParentElement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};
use std::cell::Cell;
use std::rc::Rc;

pub fn open_reflog_dialog(
    app_entity: Entity<GitOutApp>,
    entries: Vec<ReflogEntry>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter reflog entries…"));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            filter_input.focus_handle(cx).focus(window, cx);
        }

        let q = filter_input.read(cx).value().trim().to_lowercase();
        let filtered: Vec<&ReflogEntry> = entries
            .iter()
            .filter(|e| {
                q.is_empty()
                    || e.action.to_lowercase().contains(&q)
                    || e.selector.to_lowercase().contains(&q)
                    || e.short_hash.to_lowercase().contains(&q)
            })
            .collect();

        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let fg_subtle = hex_color(gh_colors::FG_SUBTLE);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);
        let blue = hex_color(gh_colors::BLUE);
        let yellow = hex_color(gh_colors::YELLOW);

        dialog
            .title("Reflog Time Machine (Ctrl+Shift+R)")
            .w(px(760.))
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        Input::new(&filter_input)
                            .id("reflog-filter")
                            .small()
                            .prefix(Icon::new(IconName::Search).xsmall().text_color(fg_muted))
                            .w_full(),
                    )
                    .child(
                        div()
                            .id("reflog-scroll")
                            .max_h(px(400.))
                            .overflow_y_scrollbar()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(border)
                            .bg(card_bg)
                            .p_2()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .when(filtered.is_empty(), |c| {
                                        c.child(
                                            div()
                                                .p_3()
                                                .text_xs()
                                                .text_color(fg_muted)
                                                .child("No reflog entries match filter."),
                                        )
                                    })
                                    .children(filtered.into_iter().enumerate().map(|(i, e)| {
                                        let h_co = e.hash.clone();
                                        let h_br = e.hash.clone();
                                        let h_rst = e.hash.clone();
                                        let sh_rst = e.short_hash.clone();
                                        let app_co = app_entity.clone();
                                        let app_br = app_entity.clone();
                                        let app_rst = app_entity.clone();

                                        h_flex()
                                            .id(("rf-row", i))
                                            .w_full()
                                            .p_2()
                                            .gap_2()
                                            .rounded(px(5.))
                                            .bg(hex_color(gh_colors::CANVAS_SUBTLE))
                                            .border_1()
                                            .border_color(border)
                                            .items_center()
                                            .justify_between()
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
                                                            .bg(yellow.opacity(0.14))
                                                            .text_xs()
                                                            .font_family("monospace")
                                                            .text_color(yellow)
                                                            .flex_shrink_0()
                                                            .child(e.selector.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_family("monospace")
                                                            .text_color(blue)
                                                            .flex_shrink_0()
                                                            .child(e.short_hash.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(fg)
                                                            .truncate()
                                                            .child(e.action.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(fg_subtle)
                                                            .flex_shrink_0()
                                                            .child(format_relative_time(
                                                                e.timestamp,
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .flex_shrink_0()
                                                    .child(
                                                        Button::new(("rf-co", i))
                                                            .xsmall()
                                                            .label("Checkout")
                                                            .on_click(move |_, window, cx| {
                                                                let h = h_co.clone();
                                                                window.close_dialog(cx);
                                                                app_co.update(cx, |app, cx| {
                                                                    app.checkout_target(h, cx);
                                                                });
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new(("rf-br", i))
                                                            .xsmall()
                                                            .icon(IconName::GitBranchPlus)
                                                            .label("Branch")
                                                            .on_click(move |_, window, cx| {
                                                                window.close_dialog(cx);
                                                                dialogs::open_create_branch_dialog(
                                                                    app_br.clone(),
                                                                    Some(h_br.clone()),
                                                                    window,
                                                                    cx,
                                                                );
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new(("rf-rst", i))
                                                            .xsmall()
                                                            .danger()
                                                            .label("Reset…")
                                                            .on_click(move |_, window, cx| {
                                                                window.close_dialog(cx);
                                                                dialogs::open_reset_dialog(
                                                                    app_rst.clone(),
                                                                    h_rst.clone(),
                                                                    sh_rst.clone(),
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
                    Button::new("rf-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

pub fn open_create_tag_dialog(
    app_entity: Entity<GitOutApp>,
    target_hash: String,
    short_hash: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("v1.0.0"));
    let msg_input = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(3)
            .placeholder("Optional tag annotation message (leave empty for lightweight tag)…")
    });
    let push_after = Rc::new(Cell::new(false));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            name_input.focus_handle(cx).focus(window, cx);
        }
        let n_ok = name_input.clone();
        let m_ok = msg_input.clone();
        let push_flag = push_after.clone();
        let push_toggle = push_after.clone();
        let app_ok = app_entity.clone();
        let target_ok = target_hash.clone();

        dialog
            .title(format!("Create Tag at {short_hash}"))
            .w(px(440.))
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Tag name"),
                            )
                            .child(Input::new(&name_input).id("tag-name-input").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Annotation message (optional)"),
                            )
                            .child(Textarea::new(&msg_input).h(px(76.)).w_full()),
                    )
                    .child(
                        Checkbox::new("tag-push-origin")
                            .label("Push tag to origin immediately")
                            .checked(push_flag.get())
                            .on_click(move |checked, window, _| {
                                push_toggle.set(*checked);
                                window.refresh();
                            }),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("tag-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("tag-submit")
                            .primary()
                            .icon(IconName::Tag)
                            .label("Create Tag")
                            .on_click(move |_, window, cx| {
                                let name = n_ok.read(cx).value().trim().to_string();
                                if name.is_empty() {
                                    return;
                                }
                                let msg = m_ok.read(cx).value().trim().to_string();
                                let p = push_flag.get();
                                let tgt = target_ok.clone();
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.create_tag(
                                        name,
                                        tgt,
                                        if msg.is_empty() { None } else { Some(msg) },
                                        p,
                                        cx,
                                    );
                                });
                            }),
                    ),
            )
    });
}

pub fn open_rename_branch_dialog(
    app_entity: Entity<GitOutApp>,
    old_name: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let name_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("New branch name")
            .default_value(old_name.clone())
    });
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            name_input.focus_handle(cx).focus(window, cx);
        }
        let n_ok = name_input.clone();
        let old_ok = old_name.clone();
        let app_ok = app_entity.clone();

        dialog
            .title(format!("Rename Branch `{old_name}`"))
            .w(px(420.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(hex_color(gh_colors::FG_MUTED))
                            .child("New branch name (`git branch -m`)"),
                    )
                    .child(Input::new(&name_input).id("rename-branch-input").w_full()),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("ren-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ren-submit")
                            .primary()
                            .label("Rename Branch")
                            .on_click(move |_, window, cx| {
                                let new_name = n_ok.read(cx).value().trim().to_string();
                                if new_name.is_empty() || new_name == old_ok {
                                    return;
                                }
                                let old = old_ok.clone();
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.rename_branch(old, new_name, cx);
                                });
                            }),
                    ),
            )
    });
}

pub fn open_remotes_dialog(
    app_entity: Entity<GitOutApp>,
    remotes: Vec<RemoteInfo>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("upstream"));
    let url_input = cx.new(|cx| {
        InputState::new(window, cx).placeholder("git@github.com:org/repo.git")
    });

    window.open_dialog(cx, move |dialog, _, _| {
        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);

        let n_ok = name_input.clone();
        let u_ok = url_input.clone();
        let app_add = app_entity.clone();

        dialog
            .title("Manage Git Remotes")
            .w(px(540.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(border)
                            .bg(card_bg)
                            .p_2()
                            .when(remotes.is_empty(), |c| {
                                c.child(
                                    div()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child("No remotes configured."),
                                )
                            })
                            .children(remotes.iter().enumerate().map(|(i, rm)| {
                                let r_name = rm.name.clone();
                                let app_rm = app_entity.clone();
                                h_flex()
                                    .id(("rm-row", i))
                                    .w_full()
                                    .p_1p5()
                                    .rounded(px(4.))
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(fg)
                                                    .child(rm.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family("monospace")
                                                    .text_color(fg_muted)
                                                    .truncate()
                                                    .child(rm.fetch_url.clone()),
                                            ),
                                    )
                                    .child(
                                        Button::new(("rm-del", i))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Trash)
                                            .tooltip("Remove remote")
                                            .on_click(move |_, window, cx| {
                                                let n = r_name.clone();
                                                window.close_dialog(cx);
                                                app_rm.update(cx, |app, cx| {
                                                    app.remote_remove(n, cx);
                                                });
                                            }),
                                    )
                            })),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(fg_muted)
                                    .child("Add Remote"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(140.))
                                            .child(Input::new(&name_input).id("rm-name").small().w_full()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&url_input).id("rm-url").small().w_full()),
                                    )
                                    .child(
                                        Button::new("rm-add-btn")
                                            .primary()
                                            .small()
                                            .icon(IconName::Plus)
                                            .label("Add")
                                            .on_click(move |_, window, cx| {
                                                let n = n_ok.read(cx).value().trim().to_string();
                                                let u = u_ok.read(cx).value().trim().to_string();
                                                if n.is_empty() || u.is_empty() {
                                                    return;
                                                }
                                                window.close_dialog(cx);
                                                app_add.update(cx, |app, cx| {
                                                    app.remote_add(n, u, cx);
                                                });
                                            }),
                                    ),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("rm-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

pub fn open_add_worktree_dialog(
    app_entity: Entity<GitOutApp>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let path_input =
        cx.new(|cx| InputState::new(window, cx).placeholder("../repo-wt-feature"));
    let branch_input = cx.new(|cx| InputState::new(window, cx).placeholder("feat/my-worktree"));
    let create_branch = Rc::new(Cell::new(true));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            path_input.focus_handle(cx).focus(window, cx);
        }
        let p_ok = path_input.clone();
        let b_ok = branch_input.clone();
        let cb_flag = create_branch.clone();
        let cb_toggle = create_branch.clone();
        let app_ok = app_entity.clone();

        dialog
            .title("Add Git Worktree")
            .w(px(460.))
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Directory path"),
                            )
                            .child(Input::new(&path_input).id("wt-path").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Branch / commit ref"),
                            )
                            .child(Input::new(&branch_input).id("wt-branch").w_full()),
                    )
                    .child(
                        Checkbox::new("wt-create-br")
                            .label("Create new branch (-b)")
                            .checked(cb_flag.get())
                            .on_click(move |checked, window, _| {
                                cb_toggle.set(*checked);
                                window.refresh();
                            }),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("wt-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("wt-submit")
                            .primary()
                            .label("Add Worktree")
                            .on_click(move |_, window, cx| {
                                let p = p_ok.read(cx).value().trim().to_string();
                                let b = b_ok.read(cx).value().trim().to_string();
                                if p.is_empty() {
                                    return;
                                }
                                let cb = cb_flag.get();
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.worktree_add(p, b, cb, cx);
                                });
                            }),
                    ),
            )
    });
}

pub fn open_apply_patch_dialog(
    app_entity: Entity<GitOutApp>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let path_input = cx.new(|cx| InputState::new(window, cx).placeholder("/path/to/fix.patch"));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            path_input.focus_handle(cx).focus(window, cx);
        }
        let p_ok = path_input.clone();
        let app_ok = app_entity.clone();

        dialog
            .title("Apply Patch File (git apply)")
            .w(px(460.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(hex_color(gh_colors::FG_MUTED))
                            .child("Path to `.patch` or `.diff` file:"),
                    )
                    .child(Input::new(&path_input).id("apply-patch-path").w_full()),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("ap-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ap-submit")
                            .primary()
                            .label("Apply Patch")
                            .on_click(move |_, window, cx| {
                                let p = p_ok.read(cx).value().trim().to_string();
                                if p.is_empty() {
                                    return;
                                }
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.apply_patch_file(p, cx);
                                });
                            }),
                    ),
            )
    });
}
