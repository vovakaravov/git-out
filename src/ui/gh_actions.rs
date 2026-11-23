use super::{GitOutApp, render_gh_unavailable};
use crate::gh::{CiState, short_iso_date};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{Context, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px};

pub fn render_actions_view(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let has_github = app
        .snapshot
        .as_ref()
        .is_some_and(|s| s.github_nwo.is_some());
    if !has_github {
        return render_gh_unavailable(
            "No GitHub remote detected",
            "GitHub Actions requires a github.com `origin` remote.",
            cx,
        )
        .into_any_element();
    }

    h_resizable("actions-split")
        .child(
            resizable_panel()
                .size(px(390.))
                .size_range(px(280.)..px(580.))
                .child(render_runs_list(app, cx)),
        )
        .child(resizable_panel().child(render_run_detail(app, cx)))
        .into_any_element()
}

fn render_runs_list(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let panel_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let blue = hex_color(gh_colors::BLUE);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);

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
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(fg)
                        .child("GitHub Actions Runs"),
                )
                .child(
                    Button::new("runs-refresh")
                        .xsmall()
                        .icon(IconName::RefreshCw)
                        .label("Reload")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_gh_runs(cx);
                        })),
                ),
        )
        .child(
            div()
                .id("runs-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_2()
                .child(
                    v_flex()
                        .gap_1p5()
                        .when(app.runs.is_empty(), |col| {
                            col.child(
                                div()
                                    .p_4()
                                    .text_xs()
                                    .text_color(fg_muted)
                                    .child("No workflow runs found."),
                            )
                        })
                        .children(app.runs.iter().enumerate().map(|(i, run)| {
                            let id = run.database_id;
                            let is_sel = app.selected_run_id == Some(id);
                            let (icon, ic_color) = match run.ci_state() {
                                CiState::Success => (IconName::CircleCheck, green),
                                CiState::Failure => (IconName::CircleX, red),
                                CiState::Pending => (IconName::Clock, yellow),
                                CiState::None => (IconName::CircleDot, fg_muted),
                            };

                            v_flex()
                                .id(("run-item", i))
                                .w_full()
                                .p_2p5()
                                .gap_1()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .bg(if is_sel {
                                    blue.opacity(0.14)
                                } else {
                                    canvas_bg
                                })
                                .border_1()
                                .border_color(if is_sel {
                                    blue.opacity(0.5)
                                } else {
                                    hex_color(gh_colors::BORDER_MUTED)
                                })
                                .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_run(id, cx);
                                }))
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .items_center()
                                        .child(Icon::new(icon).small().text_color(ic_color))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .text_color(fg)
                                                .truncate()
                                                .child(run.title_or_workflow().to_string()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child(format!("#{}", run.number))
                                        .child(run.workflow_name.clone())
                                        .child(format!("· {}", run.head_branch))
                                        .child(short_iso_date(&run.created_at)),
                                )
                        })),
                ),
        )
}

fn render_run_detail(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let canvas_bg = hex_color(gh_colors::CANVAS_DEFAULT);
    let subtle_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let inset_bg = hex_color(gh_colors::CANVAS_INSET);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let green = hex_color(gh_colors::GREEN_FG);
    let red = hex_color(gh_colors::RED_FG);
    let yellow = hex_color(gh_colors::YELLOW);

    let Some(detail) = app.selected_run_detail.as_ref() else {
        return v_flex()
            .size_full()
            .bg(canvas_bg)
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Select a workflow run to inspect jobs and failed logs."),
            )
            .into_any_element();
    };

    let run_id = detail.database_id;
    let is_failed = detail.conclusion.eq_ignore_ascii_case("failure");

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
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_base()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(fg)
                                .child(format!(
                                    "{} · #{}",
                                    detail.display_title, detail.number
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg_muted)
                                .child(format!("{} ({})", detail.workflow_name, detail.event)),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .when(is_failed, |row| {
                            row.child(
                                Button::new("run-rerun")
                                    .primary()
                                    .small()
                                    .icon(IconName::RotateCcw)
                                    .label("Rerun Failed Jobs")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.rerun_failed_run(run_id, cx);
                                    })),
                            )
                        })
                        .child(
                            Button::new("run-web")
                                .small()
                                .icon(IconName::ExternalLink)
                                .label("Open on GitHub")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_run_web(run_id, cx);
                                })),
                        ),
                ),
        )
        .child(
            div()
                .id("run-detail-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_4()
                .child(
                    v_flex()
                        .gap_4()
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(fg_muted)
                                        .child(format!("JOBS ({})", detail.jobs.len())),
                                )
                                .children(detail.jobs.iter().map(|job| {
                                    let (ic, c) = match job.ci_state() {
                                        CiState::Success => (IconName::CircleCheck, green),
                                        CiState::Failure => (IconName::CircleX, red),
                                        CiState::Pending => (IconName::Clock, yellow),
                                        CiState::None => (IconName::CircleDot, fg_muted),
                                    };
                                    v_flex()
                                        .p_2p5()
                                        .gap_1()
                                        .rounded(px(6.))
                                        .bg(subtle_bg)
                                        .border_1()
                                        .border_color(border)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(Icon::new(ic).small().text_color(c))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                        .text_color(fg)
                                                        .child(job.name.clone()),
                                                ),
                                        )
                                        .when(!job.steps.is_empty(), |col| {
                                            col.child(
                                                v_flex()
                                                    .pl_5()
                                                    .gap_0p5()
                                                    .children(job.steps.iter().map(|step| {
                                                        let step_failed = step
                                                            .conclusion
                                                            .eq_ignore_ascii_case("failure");
                                                        div()
                                                            .text_xs()
                                                            .text_color(if step_failed {
                                                                red
                                                            } else {
                                                                fg_muted
                                                            })
                                                            .child(format!(
                                                                "{}. {} ({})",
                                                                step.number,
                                                                step.name,
                                                                if step.conclusion.is_empty() {
                                                                    step.status.as_str()
                                                                } else {
                                                                    step.conclusion.as_str()
                                                                }
                                                            ))
                                                    })),
                                            )
                                        })
                                })),
                        )
                        .when(!detail.failed_logs.trim().is_empty(), |col| {
                            col.child(
                                v_flex()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(red)
                                            .child("FAILED LOGS (`gh run view --log-failed`)"),
                                    )
                                    .child(
                                        div()
                                            .p_3()
                                            .rounded(px(6.))
                                            .bg(inset_bg)
                                            .border_1()
                                            .border_color(border)
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(fg)
                                            .child(detail.failed_logs.clone()),
                                    ),
                            )
                        }),
                ),
        )
        .into_any_element()
}
