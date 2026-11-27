use super::{GitOutApp, dialogs, reflog_dialog};
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::diff::DiffMode;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Entity, Focusable as _, InteractiveElement as _, ParentElement,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use std::cell::Cell;
use std::rc::Rc;

struct PaletteItem {
    category: &'static str,
    label: String,
    hint: String,
    icon: IconName,
    run: Rc<dyn Fn(Entity<GitOutApp>, &mut Window, &mut gpui_kit::App)>,
}

pub fn open_command_palette(
    app_entity: Entity<GitOutApp>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let query_input = cx.new(|cx| {
        InputState::new(window, cx).placeholder("Type a git command, branch, tag, or repo…")
    });
    let initial_focus = Cell::new(true);

    // Snapshot commands + repo refs + recent repos once when palette opens.
    let mut items: Vec<PaletteItem> = vec![
        PaletteItem {
            category: "Action",
            label: "Fetch All Remotes".into(),
            hint: "git fetch --all --prune --tags".into(),
            icon: IconName::CloudDownload,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.fetch_all(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Pull".into(),
            hint: "git pull".into(),
            icon: IconName::ArrowDown,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.pull(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Pull (Rebase)".into(),
            hint: "git pull --rebase".into(),
            icon: IconName::ArrowDown,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.pull_rebase(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Push".into(),
            hint: "git push".into(),
            icon: IconName::ArrowUp,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.push(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Force Push (--force-with-lease)".into(),
            hint: "Safe force push".into(),
            icon: IconName::ArrowUp,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.push_force_with_lease(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Create Branch…".into(),
            hint: "git checkout -b".into(),
            icon: IconName::GitBranchPlus,
            run: Rc::new(|app, win, cx| dialogs::open_create_branch_dialog(app, None, win, cx)),
        },
        PaletteItem {
            category: "Action",
            label: "Stash Working Changes…".into(),
            hint: "git stash push".into(),
            icon: IconName::Archive,
            run: Rc::new(|app, win, cx| dialogs::open_stash_dialog(app, win, cx)),
        },
        PaletteItem {
            category: "Action",
            label: "Pop Latest Stash".into(),
            hint: "git stash pop".into(),
            icon: IconName::ArchiveRestore,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.stash_pop(None, cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Discard All Working Changes…".into(),
            hint: "git reset --hard HEAD && git clean -fd".into(),
            icon: IconName::Trash,
            run: Rc::new(|app, window, cx| {
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
            }),
        },
        PaletteItem {
            category: "Action",
            label: "Undo Last Commit (Soft Reset HEAD~1)".into(),
            hint: "Keeps changes staged".into(),
            icon: IconName::Undo2,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.undo_last_commit(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Reflog Time Machine".into(),
            hint: "Ctrl+Shift+R".into(),
            icon: IconName::Clock,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.open_reflog(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "File History & Blame Finder".into(),
            hint: "Ctrl+Shift+F".into(),
            icon: IconName::FileSearch,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.open_file_finder(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Manage Remotes…".into(),
            hint: "Add or remove git remotes".into(),
            icon: IconName::GitGraph,
            run: Rc::new(|app, win, cx| {
                let remotes = app
                    .read(cx)
                    .snapshot
                    .as_ref()
                    .map(|s| s.remotes.clone())
                    .unwrap_or_default();
                reflog_dialog::open_remotes_dialog(app, remotes, win, cx);
            }),
        },
        PaletteItem {
            category: "Action",
            label: "Add Worktree…".into(),
            hint: "git worktree add".into(),
            icon: IconName::FolderOpen,
            run: Rc::new(|app, win, cx| reflog_dialog::open_add_worktree_dialog(app, win, cx)),
        },
        PaletteItem {
            category: "Action",
            label: "Update Submodules".into(),
            hint: "git submodule update --init --recursive".into(),
            icon: IconName::RefreshCw,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.submodule_update(cx))),
        },
        PaletteItem {
            category: "Action",
            label: "Apply Patch File…".into(),
            hint: "git apply".into(),
            icon: IconName::FileDiff,
            run: Rc::new(|app, win, cx| reflog_dialog::open_apply_patch_dialog(app, win, cx)),
        },
        PaletteItem {
            category: "View",
            label: "Toggle Whitespace Diff (-w)".into(),
            hint: "Ignore whitespace changes".into(),
            icon: IconName::FileDiff,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.toggle_ignore_whitespace(cx))),
        },
        PaletteItem {
            category: "View",
            label: "Toggle Diff Split / Unified".into(),
            hint: "Switch inspector diff layout".into(),
            icon: IconName::Columns2,
            run: Rc::new(|app, _, cx| {
                app.update(cx, |a, cx| {
                    let next = if a.diff_mode == DiffMode::Split {
                        DiffMode::Unified
                    } else {
                        DiffMode::Split
                    };
                    a.set_diff_mode(next, cx);
                })
            }),
        },
        PaletteItem {
            category: "View",
            label: "Toggle Current Branch Only / All Branches".into(),
            hint: "Filter commit graph".into(),
            icon: IconName::GitBranch,
            run: Rc::new(|app, _, cx| app.update(cx, |a, cx| a.toggle_current_branch_only(cx))),
        },
        PaletteItem {
            category: "View",
            label: "Toggle Command Log Drawer".into(),
            hint: "Ctrl+`".into(),
            icon: IconName::Terminal,
            run: Rc::new(|app, _, cx| {
                app.update(cx, |a, cx| {
                    a.show_cmd_log = !a.show_cmd_log;
                    cx.notify();
                })
            }),
        },
        PaletteItem {
            category: "View",
            label: "Open Repository…".into(),
            hint: "Ctrl+O".into(),
            icon: IconName::FolderOpen,
            run: Rc::new(|app, win, cx| dialogs::open_repo_dialog(app, win, cx)),
        },
    ];

    {
        let app_ref = app_entity.read(cx);
        if let Some(snap) = app_ref.snapshot.as_ref() {
            for b in &snap.local_branches {
                if !b.is_head {
                    let b_name = b.name.clone();
                    items.push(PaletteItem {
                        category: "Checkout",
                        label: format!("Checkout branch `{b_name}`"),
                        hint: b.target_hash.chars().take(7).collect(),
                        icon: IconName::GitBranch,
                        run: Rc::new(move |app, _, cx| {
                            let n = b_name.clone();
                            app.update(cx, |a, cx| a.checkout_target(n, cx));
                        }),
                    });
                }
            }
            for t in &snap.tags {
                let t_name = t.name.clone();
                items.push(PaletteItem {
                    category: "Tag",
                    label: format!("Checkout tag `{t_name}`"),
                    hint: t.target_hash.chars().take(7).collect(),
                    icon: IconName::Tag,
                    run: Rc::new(move |app, _, cx| {
                        let n = t_name.clone();
                        app.update(cx, |a, cx| a.checkout_target(n, cx));
                    }),
                });
            }
        }
        for repo in &app_ref.config.recent_repos {
            let name = repo
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo")
                .to_string();
            let p = repo.clone();
            items.push(PaletteItem {
                category: "Repo",
                label: format!("Switch to repo `{name}`"),
                hint: repo.display().to_string(),
                icon: IconName::FolderOpen,
                run: Rc::new(move |app, _, cx| {
                    let path = p.clone();
                    app.update(cx, |a, cx| a.open_repository(path, cx));
                }),
            });
        }
    }

    let items = Rc::new(items);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            query_input.focus_handle(cx).focus(window, cx);
        }

        let q = query_input.read(cx).value().trim().to_lowercase();
        let filtered: Vec<&PaletteItem> = items
            .iter()
            .filter(|it| {
                q.is_empty()
                    || it.label.to_lowercase().contains(&q)
                    || it.hint.to_lowercase().contains(&q)
                    || it.category.to_lowercase().contains(&q)
            })
            .take(50)
            .collect();

        let fg = hex_color(gh_colors::FG);
        let fg_muted = hex_color(gh_colors::FG_MUTED);
        let border = hex_color(gh_colors::BORDER);
        let card_bg = hex_color(gh_colors::CANVAS_INSET);
        let blue = hex_color(gh_colors::BLUE);

        dialog
            .title("Command Palette (Ctrl+K)")
            .w(px(580.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Input::new(&query_input)
                            .id("palette-query")
                            .small()
                            .prefix(Icon::new(IconName::Command).xsmall().text_color(blue))
                            .w_full(),
                    )
                    .child(
                        div()
                            .id("palette-list")
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
                                                .child("No matching commands."),
                                        )
                                    })
                                    .children(filtered.into_iter().enumerate().map(|(i, it)| {
                                        let app_run = app_entity.clone();
                                        let run_cb = it.run.clone();
                                        h_flex()
                                            .id(("pal-item", i))
                                            .w_full()
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded(px(5.))
                                            .justify_between()
                                            .items_center()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                            .on_click(move |_, window, cx| {
                                                window.close_dialog(cx);
                                                run_cb(app_run.clone(), window, cx);
                                            })
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(it.icon)
                                                            .xsmall()
                                                            .text_color(blue),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_1p5()
                                                            .py(px(1.))
                                                            .rounded(px(3.))
                                                            .bg(border)
                                                            .text_xs()
                                                            .text_color(fg_muted)
                                                            .child(it.category),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(
                                                                gpui_kit::FontWeight::MEDIUM,
                                                            )
                                                            .text_color(fg)
                                                            .child(it.label.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family("monospace")
                                                    .text_color(fg_muted)
                                                    .truncate()
                                                    .child(it.hint.clone()),
                                            )
                                    })),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    Button::new("pal-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}
