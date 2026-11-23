use super::{GitOutApp, dialogs, render_gh_label, render_gh_unavailable};
use crate::gh::{CiState, short_iso_date};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::Diff;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{Context, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px};

pub fn render_prs_view(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let has_github = app
        .snapshot
        .as_ref()
        .is_some_and(|s| s.github_nwo.is_some());
    if !has_github {
        return render_gh_unavailable(
            "No GitHub remote detected",
            "This repository doesn't appear to have a github.com `origin` remote.",
            cx,
        )
        .into_any_element();
    }

    h_resizable("prs-split")
        .child(
            resizable_panel()
                .size(px(390.))
                .size_range(px(280.)..px(580.))
                .child(render_pr_list(app, cx)),
        )
        .child(resizable_panel().child(render_pr_detail(app, cx)))
        .into_any_element()
}

fn render_pr_list(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let accent = hex_color(gh_colors::ACCENT);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);
    let purple = hex_color(gh_colors::PURPLE);

    let app_entity = cx.entity();
    let head_branch = app
        .snapshot
        .as_ref()
        .and_then(|s| s.head_branch.clone())
        .unwrap_or_default();

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
                        .children(["open", "merged", "closed", "all"].iter().enumerate().map(|(idx, &st)| {
                            let active = app.pr_state_filter == st;
                            Button::new(("pr-filter", idx))
                                .xsmall()
                                .outline()
                                .label(match st {
                                    "open" => "Open",
                                    "merged" => "Merged",
                                    "closed" => "Closed",
                                    _ => "All",
                                })
                                .disabled(active)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.pr_state_filter = st.to_string();
                                    this.refresh_gh_prs(cx);
                                }))
                        })),
                )
                .child(
                    Button::new("new-pr-btn")
                        .primary()
                        .xsmall()
                        .icon(IconName::Plus)
                        .label("New PR")
                        .on_click(move |_, window, cx| {
                            dialogs::open_create_pr_dialog(
                                app_entity.clone(),
                                head_branch.clone(),
                                "main".to_string(),
                                window,
                                cx,
                            );
                        }),
                ),
        )
        .child(
            div()
                .id("pr-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_2()
                .child(
                    v_flex()
                        .gap_1p5()
                        .when_some(app.gh_error.as_ref(), |col, err| {
                            col.child(
                                div()
                                    .p_2()
                                    .rounded(px(6.))
                                    .bg(red.opacity(0.12))
                                    .border_1()
                                    .border_color(red.opacity(0.4))
                                    .text_xs()
                                    .text_color(red)
                                    .child(format!("gh error: {err}")),
                            )
                        })
                        .when(app.prs.is_empty() && app.gh_error.is_none(), |col| {
                            col.child(
                                div()
                                    .p_4()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("No pull requests matching filter."),
                            )
                        })
                        .children(app.prs.iter().enumerate().map(|(i, pr)| {
                            let num = pr.number;
                            let is_sel = app.selected_pr_number == Some(num);
                            let state_upper = pr.state.to_uppercase();
                            let icon = if pr.is_draft {
                                IconName::GitPullRequestDraft
                            } else if state_upper == "MERGED" || state_upper == "CLOSED" {
                                IconName::GitPullRequestClosed
                            } else {
                                IconName::GitPullRequest
                            };
                            let icon_color = if pr.is_draft {
                                fg_muted
                            } else if state_upper == "MERGED" {
                                purple
                            } else if state_upper == "CLOSED" {
                                red
                            } else {
                                green
                            };
                            let ci_color = match pr.ci_state() {
                                CiState::Success => green,
                                CiState::Failure => red,
                                CiState::Pending => yellow,
                                CiState::None => fg_muted,
                            };

                            v_flex()
                                .id(("pr-item", i))
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
                                    this.select_pr(num, cx);
                                }))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap_1p5()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_1p5()
                                                .items_center()
                                                .child(Icon::new(icon).small().text_color(icon_color))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                        .text_color(fg)
                                                        .truncate()
                                                        .child(pr.title.clone()),
                                                ),
                                        )
                                        .when(pr.ci_state() != CiState::None, |row| {
                                            row.child(div().size(px(7.)).rounded_full().bg(ci_color))
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child(format!("#{num}"))
                                        .child(format!(
                                            "{} → {}",
                                            pr.head_ref_name, pr.base_ref_name
                                        ))
                                        .when(!pr.author.login.is_empty(), |r| {
                                            r.child(format!("by @{}", pr.author.login))
                                        }),
                                )
                                .when(pr.is_draft || !pr.review_decision.is_empty(), |col| {
                                    col.child(
                                        h_flex()
                                            .flex_wrap()
                                            .gap_1()
                                            .when(pr.is_draft, |row| {
                                                row.child(render_review_pill(
                                                    "Draft",
                                                    fg_muted,
                                                ))
                                            })
                                            .when_some(
                                                review_decision_style(
                                                    &pr.review_decision,
                                                    green,
                                                    red,
                                                    yellow,
                                                ),
                                                |row, (label, color)| {
                                                    row.child(render_review_pill(label, color))
                                                },
                                            ),
                                    )
                                })
                                .when(!pr.labels.is_empty(), |col| {
                                    col.child(
                                        h_flex()
                                            .flex_wrap()
                                            .gap_1()
                                            .children(pr.labels.iter().map(render_gh_label)),
                                    )
                                })
                        })),
                ),
        )
}

fn render_pr_detail(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);

    let Some(detail) = app.selected_pr_detail.as_ref() else {
        return v_flex()
            .size_full()
            .bg(canvas_bg)
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Select a pull request to view description, CI checks, and diff."),
            )
            .into_any_element();
    };

    let num = detail.number;
    let title = detail.title.clone();
    let app_entity = cx.entity();
    let show_files = app.pr_show_files;
    let has_diff = !app.pr_diff_state.read(cx).files().is_empty();
    let pr_ci = detail.ci_state();

    v_flex()
        .size_full()
        .bg(canvas_bg)
        // Header + chunky GitHub actions
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
                        .w_full()
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
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .when(detail.is_draft, |row| {
                                    row.child(render_review_pill("Draft", fg_muted))
                                })
                                .when_some(
                                    review_decision_style(
                                        &detail.review_decision,
                                        green,
                                        red,
                                        yellow,
                                    ),
                                    |row, (label, color)| {
                                        row.child(render_review_pill(label, color))
                                    },
                                )
                                .when(pr_ci != CiState::None, |row| {
                                    let (ci_label, ci_col) = match pr_ci {
                                        CiState::Success => ("CI Passing", green),
                                        CiState::Failure => ("CI Failing", red),
                                        CiState::Pending => ("CI Running", yellow),
                                        CiState::None => ("", fg_muted),
                                    };
                                    row.child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(ci_col.opacity(0.16))
                                            .border_1()
                                            .border_color(ci_col.opacity(0.45))
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(ci_col)
                                            .child(ci_label),
                                    )
                                })
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
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_xs()
                        .text_color(fg_muted)
                        .child(format!(
                            "@{} wants to merge `{}` into `{}`",
                            detail.author.login, detail.head_ref_name, detail.base_ref_name
                        ))
                        .child(format!(
                            "· +{} -{} ({} files)",
                            detail.additions, detail.deletions, detail.changed_files
                        )),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1p5()
                        .pt_1()
                        .child(
                            Button::new("pr-checkout")
                                .small()
                                .icon(IconName::GitBranch)
                                .label("Checkout PR")
                                .tooltip("gh pr checkout")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.checkout_pr(num, cx);
                                })),
                        )
                        .when(detail.state.eq_ignore_ascii_case("OPEN"), |row| {
                            let app_rev = app_entity.clone();
                            let app_m = app_entity.clone();
                            let t_rev = title.clone();
                            let t = title.clone();
                            row.child(
                                Button::new("pr-review")
                                    .small()
                                    .icon(IconName::Check)
                                    .label("Review…")
                                    .tooltip("Approve, request changes, or comment via gh pr review")
                                    .on_click(move |_, window, cx| {
                                        dialogs::open_review_pr_dialog(
                                            app_rev.clone(),
                                            num,
                                            t_rev.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                            .when(detail.is_draft, |r| {
                                r.child(
                                    Button::new("pr-ready")
                                        .small()
                                        .icon(IconName::Play)
                                        .label("Ready for Review")
                                        .tooltip("Mark draft PR ready for review (gh pr ready)")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.ready_pr(num, cx);
                                        })),
                                )
                            })
                            .child(
                                Button::new("pr-merge")
                                    .primary()
                                    .small()
                                    .icon(IconName::GitMerge)
                                    .label("Merge…")
                                    .on_click(move |_, window, cx| {
                                        dialogs::open_merge_pr_dialog(
                                            app_m.clone(),
                                            num,
                                            t.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                        })
                        .child(
                            Button::new("pr-web")
                                .small()
                                .icon(IconName::ExternalLink)
                                .label("Open on GitHub")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_pr_web(num, cx);
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("pr-subtab-overview")
                                .xsmall()
                                .outline()
                                .label("Overview")
                                .disabled(!show_files)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pr_show_files = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("pr-subtab-files")
                                .xsmall()
                                .outline()
                                .label(format!("Files Changed ({})", detail.changed_files))
                                .disabled(show_files)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pr_show_files = true;
                                    cx.notify();
                                })),
                        ),
                ),
        )
        // Body
        .when(show_files, |col| {
            col.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .when(has_diff, |area| {
                        area.child(Diff::new(&app.pr_diff_state).line_number(true).size_full())
                    })
                    .when(!has_diff, |area| {
                        area.child(
                            div()
                                .id("pr-raw-diff")
                                .size_full()
                                .overflow_y_scrollbar()
                                .p_4()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(fg)
                                .child(if detail.diff_patch.trim().is_empty() {
                                    "No diff returned by `gh pr diff`.".to_string()
                                } else {
                                    detail.diff_patch.clone()
                                }),
                        )
                    }),
            )
        })
        .when(!show_files, |col| {
            col.child(
                div()
                    .id("pr-overview-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_4()
                    .child(
                        v_flex()
                            .gap_4()
                            // PR description
                            .child(
                                v_flex()
                                    .p_3()
                                    .rounded(px(6.))
                                    .bg(subtle_bg)
                                    .border_1()
                                    .border_color(border)
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(fg_muted)
                                            .child("DESCRIPTION"),
                                    )
                                    .child(if detail.body.trim().is_empty() {
                                        div()
                                            .text_xs()
                                            .text_color(fg_muted)
                                            .child("No description provided.")
                                            .into_any_element()
                                    } else {
                                        TextView::markdown(
                                            ("pr-body-md", num as usize),
                                            detail.body.clone(),
                                        )
                                        .into_any_element()
                                    }),
                            )
                            // CI Checks
                            .when(!detail.status_check_rollup.is_empty(), |col| {
                                col.child(
                                    v_flex()
                                        .p_3()
                                        .rounded(px(6.))
                                        .bg(subtle_bg)
                                        .border_1()
                                        .border_color(border)
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(fg_muted)
                                                .child(format!(
                                                    "CHECKS ({})",
                                                    detail.status_check_rollup.len()
                                                )),
                                        )
                                        .children(detail.status_check_rollup.iter().map(|chk| {
                                            let (ic, col_c) = match chk.ci_state() {
                                                CiState::Success => (IconName::CircleCheck, green),
                                                CiState::Failure => (IconName::CircleX, red),
                                                CiState::Pending => (IconName::Clock, yellow),
                                                CiState::None => (IconName::CircleDot, fg_muted),
                                            };
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(Icon::new(ic).small().text_color(col_c))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(fg)
                                                        .child(chk.display_name().to_string()),
                                                )
                                        })),
                                )
                            })
                            // Reviews & Comments
                            .when(!detail.reviews.is_empty() || !detail.comments.is_empty(), |col| {
                                col.child(
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(fg_muted)
                                                .child("ACTIVITY & COMMENTS"),
                                        )
                                        .children(detail.reviews.iter().enumerate().map(|(i, r)| {
                                            let st_upper = r.state.to_uppercase();
                                            let (badge_label, badge_col) = match st_upper.as_str() {
                                                "APPROVED" => ("APPROVED", green),
                                                "CHANGES_REQUESTED" => ("CHANGES REQUESTED", red),
                                                "DISMISSED" => ("DISMISSED", fg_muted),
                                                _ => ("COMMENTED", hex_color(gh_colors::BLUE)),
                                            };
                                            v_flex()
                                                .p_3()
                                                .gap_1p5()
                                                .rounded(px(6.))
                                                .bg(subtle_bg)
                                                .border_1()
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
                                                                    div()
                                                                        .text_xs()
                                                                        .font_weight(
                                                                            gpui_kit::FontWeight::SEMIBOLD,
                                                                        )
                                                                        .text_color(fg)
                                                                        .child(format!(
                                                                            "@{}",
                                                                            r.author.login
                                                                        )),
                                                                )
                                                                .child(render_review_pill(
                                                                    badge_label,
                                                                    badge_col,
                                                                )),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(fg_muted)
                                                                .child(short_iso_date(
                                                                    &r.submitted_at,
                                                                )),
                                                        ),
                                                )
                                                .when(!r.body.trim().is_empty(), |card| {
                                                    card.child(TextView::markdown(
                                                        ("pr-rev-md", i),
                                                        r.body.clone(),
                                                    ))
                                                })
                                        }))
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
                                                .child(TextView::markdown(("pr-cmt-md", i), c.body.clone()))
                                        })),
                                )
                            })
                            // Add comment composer
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
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(fg_muted)
                                            .child("ADD COMMENT"),
                                    )
                                    .child(Textarea::new(&app.pr_comment_input).h(px(72.)).w_full())
                                    .child(
                                        h_flex().justify_end().child(
                                            Button::new("pr-post-comment")
                                                .primary()
                                                .small()
                                                .icon(IconName::MessageSquare)
                                                .label("Comment")
                                                .disabled(
                                                    app.pr_comment_input
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .is_empty(),
                                                )
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.submit_pr_comment(num, window, cx);
                                                })),
                                        ),
                                    ),
                            ),
                    ),
            )
        })
        .into_any_element()
}

fn review_decision_style(
    decision: &str,
    green: gpui_kit::Hsla,
    red: gpui_kit::Hsla,
    yellow: gpui_kit::Hsla,
) -> Option<(&'static str, gpui_kit::Hsla)> {
    match decision.to_uppercase().as_str() {
        "APPROVED" => Some(("Approved", green)),
        "CHANGES_REQUESTED" => Some(("Changes Requested", red)),
        "REVIEW_REQUIRED" => Some(("Review Required", yellow)),
        _ => None,
    }
}

fn render_review_pill(label: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .px_1p5()
        .py(px(1.))
        .rounded_full()
        .bg(color.opacity(0.16))
        .border_1()
        .border_color(color.opacity(0.45))
        .text_xs()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label)
}
