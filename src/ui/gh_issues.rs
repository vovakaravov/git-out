use super::{GitOutApp, dialogs, render_gh_label, render_gh_unavailable};
use crate::gh::short_iso_date;
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{Context, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px};

pub fn render_issues_view(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let has_github = app
        .snapshot
        .as_ref()
        .is_some_and(|s| s.github_nwo.is_some());
    if !has_github {
        return render_gh_unavailable(
            "No GitHub remote detected",
            "GitHub Issues requires a github.com `origin` remote.",
            cx,
        )
        .into_any_element();
    }

    h_resizable("issues-split")
        .child(
            resizable_panel()
                .size(px(390.))
                .size_range(px(280.)..px(580.))
                .child(render_issue_list(app, cx)),
        )
        .child(resizable_panel().child(render_issue_detail(app, cx)))
        .into_any_element()
}

fn render_issue_list(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let accent = hex_color(gh_colors::ACCENT);
    let green = hex_color(gh_colors::GREEN_FG);
    let purple = hex_color(gh_colors::PURPLE);

    let app_entity = cx.entity();

    v_flex()
        .size_full()
        .bg(panel_bg)
        .border_r_1()
        .border_color(border)
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(border)
                .justify_between()
                .items_center()
                .child(
                    h_flex()
                        .gap_1()
                        .children(["open", "closed", "all"].iter().enumerate().map(|(idx, &st)| {
                            let active = app.issue_state_filter == st;
                            Button::new(("issue-filter", idx))
                                .xsmall()
                                .outline()
                                .label(match st {
                                    "open" => "Open",
                                    "closed" => "Closed",
                                    _ => "All",
                                })
                                .disabled(active)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.issue_state_filter = st.to_string();
                                    this.refresh_gh_issues(cx);
                                }))
                        })),
                )
                .child(
                    Button::new("new-issue-btn")
                        .primary()
                        .xsmall()
                        .icon(IconName::Plus)
                        .label("New Issue")
                        .on_click(move |_, window, cx| {
                            dialogs::open_create_issue_dialog(app_entity.clone(), window, cx);
                        }),
                ),
        )
        .child(
            div()
                .id("issue-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_2()
                .child(
                    v_flex()
                        .gap_1p5()
                        .when(app.issues.is_empty(), |col| {
                            col.child(
                                div()
                                    .p_4()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("No issues matching filter."),
                            )
                        })
                        .children(app.issues.iter().enumerate().map(|(i, issue)| {
                            let num = issue.number;
                            let is_sel = app.selected_issue_number == Some(num);
                            let is_closed = issue.state.eq_ignore_ascii_case("CLOSED");

                            v_flex()
                                .id(("issue-item", i))
                                .w_full()
                                .p_2p5()
                                .gap_1()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .bg(if is_sel {
                                    accent.opacity(0.14)
                                } else {
                                    canvas_bg
                                })
                                .border_1()
                                .border_color(if is_sel {
                                    accent.opacity(0.5)
                                } else {
                                    hex_color(gh_colors::BORDER_MUTED)
                                })
                                .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_issue(num, cx);
                                }))
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .items_center()
                                        .child(
                                            Icon::new(if is_closed {
                                                IconName::CircleCheck
                                            } else {
                                                IconName::CircleDot
                                            })
                                            .small()
                                            .text_color(if is_closed { purple } else { green }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .text_color(fg)
                                                .truncate()
                                                .child(issue.title.clone()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child(format!("#{num}"))
                                        .when(!issue.author.login.is_empty(), |r| {
                                            r.child(format!("opened by @{}", issue.author.login))
                                        })
                                        .child(short_iso_date(&issue.created_at)),
                                )
                                .when(!issue.labels.is_empty(), |col| {
                                    col.child(
                                        h_flex()
                                            .flex_wrap()
                                            .gap_1()
                                            .children(issue.labels.iter().map(render_gh_label)),
                                    )
                                })
                        })),
                ),
        )
}

fn render_issue_detail(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let green = hex_color(gh_colors::GREEN_FG);

    let Some(detail) = app.selected_issue_detail.as_ref() else {
        return v_flex()
            .size_full()
            .bg(canvas_bg)
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Select an issue to view details or create a branch from it."),
            )
            .into_any_element();
    };

    let num = detail.number;
    let title = detail.title.clone();

    v_flex()
        .size_full()
        .bg(canvas_bg)
        .child(
            v_flex()
                .w_full()
                .p_4()
                .gap_2()
                .bg(subtle_bg)
                .border_b_1()
                .border_color(border)
                .child(
                    h_flex()
                        .justify_between()
                        .items_start()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .text_base()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(fg)
                                .child(format!("{} #{num}", detail.title)),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(green.opacity(0.18))
                                .border_1()
                                .border_color(green.opacity(0.45))
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(green)
                                .child(detail.state.clone()),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Button::new("issue-create-branch")
                                .primary()
                                .small()
                                .icon(IconName::GitBranchPlus)
                                .label("Create Branch")
                                .tooltip("gh issue develop --checkout (or local branch)")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.develop_issue(num, title.clone(), cx);
                                })),
                        )
                        .child(
                            Button::new("issue-web")
                                .small()
                                .icon(IconName::ExternalLink)
                                .label("Open on GitHub")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_issue_web(num, cx);
                                })),
                        ),
                ),
        )
        .child(
            div()
                .id("issue-detail-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_4()
                .child(
                    v_flex()
                        .gap_4()
                        .child(
                            v_flex()
                                .p_3()
                                .gap_2()
                                .rounded(px(6.))
                                .bg(subtle_bg)
                                .border_1()
                                .border_color(border)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child(format!(
                                            "@{} · {}",
                                            detail.author.login,
                                            short_iso_date(&detail.created_at)
                                        )),
                                )
                                .child(if detail.body.trim().is_empty() {
                                    div()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child("No description provided.")
                                        .into_any_element()
                                } else {
                                    TextView::markdown(
                                        ("issue-body-md", num as usize),
                                        detail.body.clone(),
                                    )
                                    .into_any_element()
                                }),
                        )
                        .children(detail.comments.iter().enumerate().map(|(i, c)| {
                            v_flex()
                                .p_3()
                                .gap_1()
                                .rounded(px(6.))
                                .bg(subtle_bg)
                                .border_1()
                                .border_color(border)
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child(format!("@{}", c.author.login))
                                        .child(short_iso_date(&c.created_at)),
                                )
                                .child(TextView::markdown(("issue-cmt-md", i), c.body.clone()))
                        })),
                ),
        )
        .into_any_element()
}
