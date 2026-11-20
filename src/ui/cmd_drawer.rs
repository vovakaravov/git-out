use super::GitOutApp;
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::{ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px};

pub fn render_cmd_drawer(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let border = hex_color(gh_colors::BORDER);
    let header_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let body_bg = hex_color(gh_colors::CANVAS_INSET);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let ok_fg = hex_color(gh_colors::GREEN_FG);
    let err_fg = hex_color(gh_colors::RED_FG);

    v_flex()
        .w_full()
        .h(px(190.))
        .flex_shrink_0()
        .border_t_1()
        .border_color(border)
        .bg(body_bg)
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_1()
                .bg(header_bg)
                .border_b_1()
                .border_color(border)
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(IconName::Terminal)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child("Command Log"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg_muted)
                                .child(format!("{} commands (git & gh)", app.cmd_log.len())),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("cmd-clear")
                                .ghost()
                                .xsmall()
                                .label("Clear")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cmd_log.clear();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("cmd-close")
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_cmd_log = false;
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .child(
            div()
                .id("cmd-log-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_2()
                .child(
                    v_flex()
                        .gap_1()
                        .when(app.cmd_log.is_empty(), |col| {
                            col.child(
                                div()
                                    .p_2()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("No git or gh commands recorded yet."),
                            )
                        })
                        .children(app.cmd_log.iter().enumerate().map(|(idx, entry)| {
                            let full_cmd = entry.display_cmd();
                            let cmd_for_copy = full_cmd.clone();
                            let status_badge = if entry.ok { "OK" } else { "ERR" };
                            let status_color = if entry.ok { ok_fg } else { err_fg };
                            let detail_line = if !entry.stderr.is_empty() {
                                entry.stderr.clone()
                            } else {
                                entry.stdout_preview.clone()
                            };

                            h_flex()
                                .w_full()
                                .px_2()
                                .py_1()
                                .gap_2()
                                .rounded(px(4.))
                                .bg(hex_color(gh_colors::CANVAS_DEFAULT))
                                .border_1()
                                .border_color(hex_color(gh_colors::BORDER_MUTED))
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
                                                .text_xs()
                                                .text_color(fg_muted)
                                                .flex_shrink_0()
                                                .child(entry.timestamp.format("%H:%M:%S").to_string()),
                                        )
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded(px(4.))
                                                .bg(status_color.opacity(0.15))
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .text_color(status_color)
                                                .flex_shrink_0()
                                                .child(status_badge),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(fg_muted)
                                                .flex_shrink_0()
                                                .child(format!("{}ms", entry.duration_ms)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(fg)
                                                .truncate()
                                                .child(full_cmd),
                                        )
                                        .when(!detail_line.is_empty(), |row| {
                                            row.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if entry.ok { fg_muted } else { err_fg })
                                                    .truncate()
                                                    .child(format!("— {detail_line}")),
                                            )
                                        }),
                                )
                                .child(
                                    Button::new(("copy-cmd", idx))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Copy)
                                        .tooltip("Copy command")
                                        .on_click(move |_, window, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                cmd_for_copy.clone(),
                                            ));
                                            super::notify_info(
                                                window,
                                                cx,
                                                "Copied command to clipboard",
                                            );
                                        }),
                                )
                        })),
                ),
        )
}

use gpui_kit::prelude::FluentBuilder as _;
