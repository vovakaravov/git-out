use super::GitOutApp;
use crate::config;
use crate::gh::MergeStrategy;
use crate::git::ResetMode;
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{AppContext as _, Entity, Focusable as _, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

pub fn open_repo_dialog(app_entity: Entity<GitOutApp>, window: &mut Window, cx: &mut gpui_kit::App) {
    let path_input = cx.new(|cx| {
        InputState::new(window, cx).placeholder("/path/to/git/repository")
    });
    let recent = app_entity.read(cx).config.recent_repos.clone();
    let discovered = config::discover_home_git_repos();
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            path_input.focus_handle(cx).focus(window, cx);
        }

        let input_for_ok = path_input.clone();
        let app_for_ok = app_entity.clone();
        let app_for_recent = app_entity.clone();
        let app_for_disc = app_entity.clone();

        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);

        dialog
            .title("Open Git Repository")
            .w(px(560.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .text_color(fg_muted)
                                    .child("Repository path"),
                            )
                            .child(Input::new(&path_input).id("open-repo-path").w_full()),
                    )
                    .when(!recent.is_empty(), |col| {
                        let app_ref = app_for_recent.clone();
                        col.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(fg_muted)
                                        .child("Recent repositories"),
                                )
                                .child(
                                    v_flex()
                                        .max_h(px(140.))
                                        .id("recent-repos-scroll")
                                        .overflow_y_scrollbar()
                                        .rounded(px(6.))
                                        .border_1()
                                        .border_color(border)
                                        .bg(card_bg)
                                        .children(recent.iter().enumerate().map(|(i, p)| {
                                            render_repo_pick_row(
                                                ("recent-repo", i),
                                                p.clone(),
                                                app_ref.clone(),
                                                fg,
                                                fg_muted,
                                            )
                                        })),
                                ),
                        )
                    })
                    .when(!discovered.is_empty(), |col| {
                        let app_ref = app_for_disc.clone();
                        col.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(fg_muted)
                                        .child("Discovered in ~/"),
                                )
                                .child(
                                    v_flex()
                                        .max_h(px(180.))
                                        .id("discovered-repos-scroll")
                                        .overflow_y_scrollbar()
                                        .rounded(px(6.))
                                        .border_1()
                                        .border_color(border)
                                        .bg(card_bg)
                                        .children(discovered.iter().enumerate().map(|(i, p)| {
                                            render_repo_pick_row(
                                                ("disc-repo", i),
                                                p.clone(),
                                                app_ref.clone(),
                                                fg,
                                                fg_muted,
                                            )
                                        })),
                                ),
                        )
                    }),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("open-repo-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("open-repo-submit")
                            .primary()
                            .label("Open")
                            .on_click(move |_, window, cx| {
                                let raw = input_for_ok.read(cx).value().trim().to_string();
                                if raw.is_empty() {
                                    return;
                                }
                                let expanded = expand_tilde(&raw);
                                window.close_dialog(cx);
                                app_for_ok.update(cx, |app, cx| {
                                    app.open_repository(expanded, cx);
                                });
                            }),
                    ),
            )
    });
}

fn render_repo_pick_row(
    id: (&'static str, usize),
    path: PathBuf,
    app_entity: Entity<GitOutApp>,
    fg: gpui_kit::Hsla,
    fg_muted: gpui_kit::Hsla,
) -> impl IntoElement {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();
    let display = path.display().to_string();

    div()
        .id(id)
        .w_full()
        .px_2p5()
        .py_1p5()
        .cursor_pointer()
        .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
        .on_click(move |_, window, cx| {
            let chosen = path.clone();
            window.close_dialog(cx);
            app_entity.update(cx, |app, cx| {
                app.open_repository(chosen, cx);
            });
        })
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(IconName::FolderOpen)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child(name),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(fg_muted)
                        .truncate()
                        .child(display),
                ),
        )
}

fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    if raw == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    PathBuf::from(raw)
}

pub fn open_create_branch_dialog(
    app_entity: Entity<GitOutApp>,
    start_point: Option<String>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("feat/my-branch"));
    let initial_focus = Cell::new(true);
    let checkout_after = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            name_input.focus_handle(cx).focus(window, cx);
        }
        let name_for_ok = name_input.clone();
        let app_for_ok = app_entity.clone();
        let start_for_ok = start_point.clone();
        let checkout_flag = checkout_after.clone();
        let checkout_toggle = checkout_after.clone();

        let title = match &start_point {
            Some(sp) => format!("Create Branch at {}", &sp[..sp.len().min(8)]),
            None => "Create Branch".to_string(),
        };

        dialog
            .title(title)
            .w(px(420.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Branch name"),
                            )
                            .child(Input::new(&name_input).id("new-branch-name").w_full()),
                    )
                    .child(
                        Checkbox::new("checkout-after-create")
                            .label("Checkout branch immediately")
                            .checked(checkout_flag.get())
                            .on_click(move |checked, window, _| {
                                checkout_toggle.set(*checked);
                                window.refresh();
                            }),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("branch-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("branch-create")
                            .primary()
                            .label("Create Branch")
                            .on_click(move |_, window, cx| {
                                let name = name_for_ok.read(cx).value().trim().to_string();
                                if name.is_empty() {
                                    return;
                                }
                                let start = start_for_ok.clone();
                                let co = checkout_flag.get();
                                window.close_dialog(cx);
                                app_for_ok.update(cx, |app, cx| {
                                    app.create_branch(name, start, co, cx);
                                });
                            }),
                    ),
            )
    });
}

pub fn open_stash_dialog(app_entity: Entity<GitOutApp>, window: &mut Window, cx: &mut gpui_kit::App) {
    let msg_input = cx.new(|cx| InputState::new(window, cx).placeholder("Optional stash message"));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            msg_input.focus_handle(cx).focus(window, cx);
        }
        let msg_for_ok = msg_input.clone();
        let app_for_ok = app_entity.clone();

        dialog
            .title("Stash Working Changes")
            .w(px(420.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(hex_color(gh_colors::FG_MUTED))
                            .child("Stashes tracked and untracked files (git stash push -u)."),
                    )
                    .child(Input::new(&msg_input).id("stash-msg").w_full()),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("stash-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("stash-ok")
                            .primary()
                            .label("Stash Changes")
                            .on_click(move |_, window, cx| {
                                let msg = msg_for_ok.read(cx).value().trim().to_string();
                                window.close_dialog(cx);
                                app_for_ok.update(cx, |app, cx| {
                                    app.stash_push(if msg.is_empty() { None } else { Some(msg) }, cx);
                                });
                            }),
                    ),
            )
    });
}

pub fn open_reset_dialog(
    app_entity: Entity<GitOutApp>,
    hash: String,
    short_hash: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    window.open_dialog(cx, move |dialog, _, _| {
        let app_soft = app_entity.clone();
        let app_mixed = app_entity.clone();
        let app_hard = app_entity.clone();
        let h_soft = hash.clone();
        let h_mixed = hash.clone();
        let h_hard = hash.clone();

        dialog
            .title(format!("Reset current branch to {short_hash}"))
            .w(px(480.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(hex_color(gh_colors::FG_MUTED))
                            .child("Choose how HEAD, the index, and working directory should be updated:"),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .p_2()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(hex_color(gh_colors::BORDER))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .child("Soft (--soft)"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(hex_color(gh_colors::FG_MUTED))
                                            .child("Keep all changes staged for commit."),
                                    ),
                            )
                            .child(
                                Button::new("reset-soft")
                                    .small()
                                    .label("Soft Reset")
                                    .on_click(move |_, window, cx| {
                                        let h = h_soft.clone();
                                        window.close_dialog(cx);
                                        app_soft.update(cx, |app, cx| {
                                            app.reset_to(h, ResetMode::Soft, cx);
                                        });
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .p_2()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(hex_color(gh_colors::BORDER))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .child("Mixed (--mixed)"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(hex_color(gh_colors::FG_MUTED))
                                            .child("Keep changes in working tree, unstage them."),
                                    ),
                            )
                            .child(
                                Button::new("reset-mixed")
                                    .small()
                                    .label("Mixed Reset")
                                    .on_click(move |_, window, cx| {
                                        let h = h_mixed.clone();
                                        window.close_dialog(cx);
                                        app_mixed.update(cx, |app, cx| {
                                            app.reset_to(h, ResetMode::Mixed, cx);
                                        });
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .p_2()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(hex_color(gh_colors::RED_BTN))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(hex_color(gh_colors::RED_FG))
                                            .child("Hard (--hard)"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(hex_color(gh_colors::FG_MUTED))
                                            .child("Discard all working tree & staged changes."),
                                    ),
                            )
                            .child(
                                Button::new("reset-hard")
                                    .danger()
                                    .small()
                                    .label("Hard Reset")
                                    .on_click(move |_, window, cx| {
                                        let h = h_hard.clone();
                                        window.close_dialog(cx);
                                        app_hard.update(cx, |app, cx| {
                                            app.reset_to(h, ResetMode::Hard, cx);
                                        });
                                    }),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("reset-cancel")
                        .label("Cancel")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

pub fn open_create_pr_dialog(
    app_entity: Entity<GitOutApp>,
    default_title: String,
    default_base: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let title_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("Pull request title")
            .default_value(default_title)
    });
    let base_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("Base branch (e.g. main)")
            .default_value(default_base)
    });
    let body_input = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(6)
            .placeholder("Describe your changes…")
    });
    let is_draft = Rc::new(Cell::new(false));
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            title_input.focus_handle(cx).focus(window, cx);
        }
        let t_ok = title_input.clone();
        let b_ok = base_input.clone();
        let body_ok = body_input.clone();
        let draft_flag = is_draft.clone();
        let draft_toggle = is_draft.clone();
        let app_ok = app_entity.clone();

        dialog
            .title("Create Pull Request (gh pr create)")
            .w(px(540.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Title"),
                            )
                            .child(Input::new(&title_input).id("create-pr-title").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Base branch"),
                            )
                            .child(Input::new(&base_input).id("create-pr-base").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Description (Markdown)"),
                            )
                            .child(Textarea::new(&body_input).h(px(140.)).w_full()),
                    )
                    .child(
                        Checkbox::new("create-pr-draft")
                            .label("Create as draft pull request")
                            .checked(draft_flag.get())
                            .on_click(move |checked, window, _| {
                                draft_toggle.set(*checked);
                                window.refresh();
                            }),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("create-pr-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("create-pr-submit")
                            .primary()
                            .label("Create Pull Request")
                            .on_click(move |_, window, cx| {
                                let title = t_ok.read(cx).value().trim().to_string();
                                if title.is_empty() {
                                    return;
                                }
                                let base = b_ok.read(cx).value().trim().to_string();
                                let body = body_ok.read(cx).value().to_string();
                                let draft = draft_flag.get();
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.create_pr(
                                        title,
                                        body,
                                        if base.is_empty() { None } else { Some(base) },
                                        draft,
                                        cx,
                                    );
                                });
                            }),
                    ),
            )
    });
}

pub fn open_merge_pr_dialog(
    app_entity: Entity<GitOutApp>,
    pr_number: u64,
    pr_title: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    window.open_dialog(cx, move |dialog, _, _| {
        let app_sq = app_entity.clone();
        let app_mg = app_entity.clone();
        let app_rb = app_entity.clone();

        dialog
            .title(format!("Merge PR #{pr_number}"))
            .w(px(460.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(pr_title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(hex_color(gh_colors::FG_MUTED))
                            .child("Select a merge strategy to run via `gh pr merge`:"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .pt_2()
                            .child(
                                Button::new("merge-squash")
                                    .primary()
                                    .label("Squash & Merge")
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        app_sq.update(cx, |app, cx| {
                                            app.merge_pr(pr_number, MergeStrategy::Squash, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("merge-commit")
                                    .label("Merge Commit")
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        app_mg.update(cx, |app, cx| {
                                            app.merge_pr(pr_number, MergeStrategy::Merge, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("merge-rebase")
                                    .label("Rebase & Merge")
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        app_rb.update(cx, |app, cx| {
                                            app.merge_pr(pr_number, MergeStrategy::Rebase, cx);
                                        });
                                    }),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("merge-pr-cancel")
                        .label("Cancel")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

pub fn open_create_issue_dialog(
    app_entity: Entity<GitOutApp>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("Issue title"));
    let body_input = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(6)
            .placeholder("Describe the issue…")
    });
    let initial_focus = Cell::new(true);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            title_input.focus_handle(cx).focus(window, cx);
        }
        let t_ok = title_input.clone();
        let b_ok = body_input.clone();
        let app_ok = app_entity.clone();

        dialog
            .title("New GitHub Issue (gh issue create)")
            .w(px(520.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Title"),
                            )
                            .child(Input::new(&title_input).id("create-issue-title").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hex_color(gh_colors::FG_MUTED))
                                    .child("Description"),
                            )
                            .child(Textarea::new(&body_input).h(px(140.)).w_full()),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("create-issue-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("create-issue-submit")
                            .primary()
                            .label("Submit Issue")
                            .on_click(move |_, window, cx| {
                                let title = t_ok.read(cx).value().trim().to_string();
                                if title.is_empty() {
                                    return;
                                }
                                let body = b_ok.read(cx).value().to_string();
                                window.close_dialog(cx);
                                app_ok.update(cx, |app, cx| {
                                    app.create_issue(title, body, cx);
                                });
                            }),
                    ),
            )
    });
}

pub fn open_confirm_danger(
    title: String,
    description: String,
    ok_label: String,
    window: &mut Window,
    cx: &mut gpui_kit::App,
    on_confirm: Rc<dyn Fn(&mut Window, &mut gpui_kit::App)>,
) {
    window.open_alert_dialog(cx, move |alert, _, _| {
        let cb = on_confirm.clone();
        alert
            .confirm()
            .title(title.clone())
            .description(description.clone())
            .ok_text(ok_label.clone())
            .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                cb(window, cx);
                true
            })
    });
}
