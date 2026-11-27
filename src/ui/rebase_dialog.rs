use super::GitOutApp;
use crate::git::{RebaseAction, RebaseTodoItem};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Entity, InteractiveElement as _, ParentElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn open_interactive_rebase_dialog(
    app_entity: Entity<GitOutApp>,
    base_hash: String,
    base_short: String,
    commits: Vec<RebaseTodoItem>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let reword_inputs: HashMap<String, Entity<InputState>> = commits
        .iter()
        .map(|c| {
            let default_msg = c.subject.clone();
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Updated commit message…")
                    .default_value(default_msg)
            });
            (c.hash.clone(), input)
        })
        .collect();

    let state = Rc::new(RefCell::new(commits));

    window.open_dialog(cx, move |dialog, _, _| {
        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);
        let blue = hex_color(gh_colors::BLUE);
        let green = hex_color(gh_colors::GREEN_FG);
        let purple = hex_color(gh_colors::PURPLE);
        let cyan = hex_color(gh_colors::CYAN);
        let red = hex_color(gh_colors::RED_FG);

        let items_snapshot = state.borrow().clone();
        let can_start = !items_snapshot.is_empty();

        let state_submit = state.clone();
        let inputs_submit = reword_inputs.clone();
        let app_submit = app_entity.clone();
        let base_submit = base_hash.clone();

        dialog
            .title(format!("Interactive Rebase onto {base_short}"))
            .w(px(680.))
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(fg_muted)
                            .child("Commits apply top-to-bottom (oldest first). Click an action badge to cycle Pick → Reword → Squash → Fixup → Drop, or reorder rows."),
                    )
                    .child(
                        div()
                            .id("rebase-todo-scroll")
                            .max_h(px(380.))
                            .overflow_y_scrollbar()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(border)
                            .bg(card_bg)
                            .p_2()
                            .child(
                                v_flex()
                                    .gap_1p5()
                                    .children(items_snapshot.into_iter().enumerate().map(|(idx, item)| {
                                        let len = state.borrow().len();
                                        let color = match item.action {
                                            RebaseAction::Pick => green,
                                            RebaseAction::Reword => blue,
                                            RebaseAction::Squash => purple,
                                            RebaseAction::Fixup => cyan,
                                            RebaseAction::Drop => red,
                                        };
                                        let state_act = state.clone();
                                        let state_up = state.clone();
                                        let state_dn = state.clone();
                                        let is_reword = item.action == RebaseAction::Reword;
                                        let rw_input = reword_inputs.get(&item.hash).cloned();

                                        v_flex()
                                            .gap_1()
                                            .p_1p5()
                                            .rounded(px(5.))
                                            .bg(hex_color(gh_colors::CANVAS_SUBTLE))
                                            .border_1()
                                            .border_color(border)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .gap_2()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        h_flex()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .gap_2()
                                                            .items_center()
                                                            .child(
                                                                Button::new(("rb-act", idx))
                                                                    .xsmall()
                                                                    .label(item.action.label())
                                                                    .tooltip("Click to cycle rebase action")
                                                                    .on_click(move |_, window, _| {
                                                                        if let Some(row) = state_act.borrow_mut().get_mut(idx) {
                                                                            row.action = row.action.cycle_next();
                                                                        }
                                                                        window.refresh();
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(px(8.))
                                                                    .h(px(8.))
                                                                    .rounded_full()
                                                                    .bg(color)
                                                                    .flex_shrink_0(),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_family("monospace")
                                                                    .text_color(fg_muted)
                                                                    .flex_shrink_0()
                                                                    .child(item.short_hash.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(if item.action == RebaseAction::Drop {
                                                                        fg_muted
                                                                    } else {
                                                                        fg
                                                                    })
                                                                    .truncate()
                                                                    .child(item.subject.clone()),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_1()
                                                            .flex_shrink_0()
                                                            .child(
                                                                Button::new(("rb-up", idx))
                                                                    .ghost()
                                                                    .xsmall()
                                                                    .icon(IconName::ChevronUp)
                                                                    .disabled(idx == 0)
                                                                    .on_click(move |_, window, _| {
                                                                        let mut vec = state_up.borrow_mut();
                                                                        if idx > 0 && idx < vec.len() {
                                                                            vec.swap(idx, idx - 1);
                                                                        }
                                                                        window.refresh();
                                                                    }),
                                                            )
                                                            .child(
                                                                Button::new(("rb-dn", idx))
                                                                    .ghost()
                                                                    .xsmall()
                                                                    .icon(IconName::ChevronDown)
                                                                    .disabled(idx + 1 >= len)
                                                                    .on_click(move |_, window, _| {
                                                                        let mut vec = state_dn.borrow_mut();
                                                                        if idx + 1 < vec.len() {
                                                                            vec.swap(idx, idx + 1);
                                                                        }
                                                                        window.refresh();
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                            .when(is_reword, |row| {
                                                if let Some(inp) = rw_input {
                                                    row.child(
                                                        Input::new(&inp)
                                                            .id(("rb-rw-msg", idx))
                                                            .small()
                                                            .w_full(),
                                                    )
                                                } else {
                                                    row
                                                }
                                            })
                                    })),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("rb-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("rb-run")
                            .primary()
                            .icon(IconName::Play)
                            .label("Start Interactive Rebase")
                            .disabled(!can_start)
                            .on_click(move |_, window, cx| {
                                let mut final_items = state_submit.borrow().clone();
                                for item in &mut final_items {
                                    if item.action == RebaseAction::Reword
                                        && let Some(inp) = inputs_submit.get(&item.hash)
                                    {
                                        let v = inp.read(cx).value().trim().to_string();
                                        if !v.is_empty() {
                                            item.reword_message = Some(v);
                                        }
                                    }
                                }
                                window.close_dialog(cx);
                                let b = base_submit.clone();
                                app_submit.update(cx, |app, cx| {
                                    app.run_interactive_rebase(b, final_items, cx);
                                });
                            }),
                    ),
            )
    });
}
