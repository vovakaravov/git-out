use super::{ActiveTab, GitOutApp, Selection, dialogs, reflog_dialog};
use crate::gh::CiState;
use crate::theme::{gh_colors, hex_color};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

pub fn render_sidebar(app: &GitOutApp, cx: &mut Context<GitOutApp>) -> impl IntoElement {
    let sidebar_bg = hex_color(gh_colors::CANVAS_SUBTLE);
    let border = hex_color(gh_colors::BORDER);
    let fg = hex_color(gh_colors::FG);
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    let blue = hex_color(gh_colors::BLUE);
    let green = hex_color(gh_colors::GREEN_FG);
    let purple = hex_color(gh_colors::PURPLE);
    let yellow = hex_color(gh_colors::YELLOW);
    let red = hex_color(gh_colors::RED_FG);
    let orange = hex_color(gh_colors::ORANGE);

    let Some(snap) = app.snapshot.as_ref() else {
        return v_flex()
            .size_full()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border)
            .p_3()
            .child(
                div()
                    .text_xs()
                    .text_color(fg_muted)
                    .child("Open a Git repository to browse branches, tags, stashes, worktrees, and submodules."),
            )
            .into_any_element();
    };

    let head_branch = snap.head_branch.clone();
    let app_entity = cx.entity();

    div()
        .id("sidebar-scroll")
        .size_full()
        .bg(sidebar_bg)
        .border_r_1()
        .border_color(border)
        .overflow_y_scrollbar()
        .p_2()
        .child(
            v_flex()
                .gap_3()
                // LOCAL BRANCHES
                .child(render_section_header(
                    "local-sec",
                    "LOCAL",
                    snap.local_branches.len(),
                    app.sec_local_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_local_open = !this.sec_local_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_local_open, |col| {
                    col.child(
                        v_flex()
                            .gap_0p5()
                            .children(snap.local_branches.iter().enumerate().map(|(i, b)| {
                                let name_jump = b.name.clone();
                                let hash_jump = b.target_hash.clone();
                                let name_co = b.name.clone();
                                let name_merge = b.name.clone();
                                let name_del = b.name.clone();
                                let name_menu = b.name.clone();
                                let is_head = b.is_head;
                                let ahead = b.ahead;
                                let behind = b.behind;
                                let app_for_del = app_entity.clone();
                                let app_menu = app_entity.clone();
                                let can_merge = head_branch.as_deref() != Some(b.name.as_str());
                                let cur_head = head_branch.clone().unwrap_or_else(|| "HEAD".into());

                                h_flex()
                                    .id(("local-br", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(5.))
                                    .gap_1()
                                    .items_center()
                                    .justify_between()
                                    .bg(if is_head {
                                        blue.opacity(0.12)
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .context_menu(move |menu, _, _| {
                                        let app_co = app_menu.clone();
                                        let app_mg = app_menu.clone();
                                        let app_rb = app_menu.clone();
                                        let app_cmp = app_menu.clone();
                                        let app_br = app_menu.clone();
                                        let app_ren = app_menu.clone();
                                        let app_del_m = app_menu.clone();
                                        let app_fdel_m = app_menu.clone();
                                        let n_co = name_menu.clone();
                                        let n_mg = name_menu.clone();
                                        let n_rb = name_menu.clone();
                                        let n_cmp = name_menu.clone();
                                        let h_cmp = cur_head.clone();
                                        let n_br = name_menu.clone();
                                        let n_ren = name_menu.clone();
                                        let n_del = name_menu.clone();
                                        let n_fdel = name_menu.clone();
                                        let n_copy = name_menu.clone();

                                        menu.label(format!("Branch {name_menu}"))
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Checkout '{name_menu}'"
                                                ))
                                                .icon(IconName::Check)
                                                .disabled(is_head)
                                                .on_click(move |_, _, cx| {
                                                    let n = n_co.clone();
                                                    app_co.update(cx, |app, cx| {
                                                        app.checkout_target(n, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Merge '{name_menu}' into current"
                                                ))
                                                .icon(IconName::GitMerge)
                                                .disabled(!can_merge)
                                                .on_click(move |_, _, cx| {
                                                    let n = n_mg.clone();
                                                    app_mg.update(cx, |app, cx| {
                                                        app.merge_branch(n, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Rebase current onto '{name_menu}'"
                                                ))
                                                .icon(IconName::GitCommitHorizontal)
                                                .disabled(!can_merge)
                                                .on_click(move |_, _, cx| {
                                                    let n = n_rb.clone();
                                                    app_rb.update(cx, |app, cx| {
                                                        app.rebase_onto(n, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Compare '{name_menu}'..HEAD"
                                                ))
                                                .icon(IconName::GitCompare)
                                                .disabled(!can_merge)
                                                .on_click(move |_, _, cx| {
                                                    let b = n_cmp.clone();
                                                    let hd = h_cmp.clone();
                                                    app_cmp.update(cx, |app, cx| {
                                                        app.compare_commits(b, hd, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Create branch from here…")
                                                    .icon(IconName::GitBranchPlus)
                                                    .on_click(move |_, window, cx| {
                                                        dialogs::open_create_branch_dialog(
                                                            app_br.clone(),
                                                            Some(n_br.clone()),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Rename '{name_menu}'…"
                                                ))
                                                .icon(IconName::GitBranch)
                                                .on_click(move |_, window, cx| {
                                                    reflog_dialog::open_rename_branch_dialog(
                                                        app_ren.clone(),
                                                        n_ren.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .separator()
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Delete '{name_menu}' (-d)…"
                                                ))
                                                .icon(IconName::Trash)
                                                .disabled(is_head)
                                                .on_click(move |_, window, cx| {
                                                    let app_d = app_del_m.clone();
                                                    let target = n_del.clone();
                                                    dialogs::open_confirm_danger(
                                                        format!("Delete branch `{target}`?"),
                                                        "Runs `git branch -d`.".to_string(),
                                                        "Delete".to_string(),
                                                        window,
                                                        cx,
                                                        Rc::new(move |_, cx| {
                                                            let bname = target.clone();
                                                            app_d.update(cx, |app, cx| {
                                                                app.delete_branch(bname, false, cx);
                                                            });
                                                        }),
                                                    );
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Force Delete '{name_menu}' (-D)…"
                                                ))
                                                .icon(IconName::TriangleAlert)
                                                .disabled(is_head)
                                                .on_click(move |_, window, cx| {
                                                    let app_d = app_fdel_m.clone();
                                                    let target = n_fdel.clone();
                                                    dialogs::open_confirm_danger(
                                                        format!("Force delete `{target}`?"),
                                                        "Runs `git branch -D`, discarding any unmerged commits on this branch.".to_string(),
                                                        "Force Delete".to_string(),
                                                        window,
                                                        cx,
                                                        Rc::new(move |_, cx| {
                                                            let bname = target.clone();
                                                            app_d.update(cx, |app, cx| {
                                                                app.delete_branch(bname, true, cx);
                                                            });
                                                        }),
                                                    );
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Copy branch name")
                                                    .icon(IconName::Copy)
                                                    .on_click(move |_, window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                n_copy.clone(),
                                                            ),
                                                        );
                                                        super::notify_info(
                                                            window,
                                                            cx,
                                                            "Copied branch name",
                                                        );
                                                    }),
                                            )
                                    })
                                    .child(
                                        h_flex()
                                            .id(("local-br-jump", i))
                                            .flex_1()
                                            .min_w_0()
                                            .gap_1p5()
                                            .items_center()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, event: &gpui_kit::ClickEvent, _, cx| {
                                                if event.click_count() >= 2 && !is_head {
                                                    this.checkout_target(name_jump.clone(), cx);
                                                } else {
                                                    this.jump_to_commit(&hash_jump, cx);
                                                }
                                            }))
                                            .child(
                                                Icon::new(IconName::GitBranch)
                                                    .xsmall()
                                                    .text_color(if is_head { green } else { blue }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(if is_head {
                                                        gpui_kit::FontWeight::BOLD
                                                    } else {
                                                        gpui_kit::FontWeight::NORMAL
                                                    })
                                                    .text_color(if is_head { fg } else { fg_muted })
                                                    .truncate()
                                                    .child(b.name.clone()),
                                            )
                                            .when(ahead > 0 || behind > 0, |row| {
                                                row.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(fg_muted)
                                                        .flex_shrink_0()
                                                        .child(format!("↑{ahead}↓{behind}")),
                                                )
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_0p5()
                                            .flex_shrink_0()
                                            .when(!is_head, |acts| {
                                                acts.child(
                                                    Button::new(("co-br", i))
                                                        .ghost()
                                                        .xsmall()
                                                        .icon(IconName::Check)
                                                        .tooltip("Checkout branch")
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.checkout_target(
                                                                    name_co.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                )
                                                .when(can_merge, |acts| {
                                                    acts.child(
                                                        Button::new(("mg-br", i))
                                                            .ghost()
                                                            .xsmall()
                                                            .icon(IconName::GitMerge)
                                                            .tooltip("Merge into current branch")
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    this.merge_branch(
                                                                        name_merge.clone(),
                                                                        cx,
                                                                    );
                                                                },
                                                            )),
                                                    )
                                                })
                                                .child(
                                                    Button::new(("del-br", i))
                                                        .ghost()
                                                        .xsmall()
                                                        .icon(IconName::Trash)
                                                        .tooltip("Delete branch")
                                                        .on_click(move |_, window, cx| {
                                                            let app_del = app_for_del.clone();
                                                            let target = name_del.clone();
                                                            dialogs::open_confirm_danger(
                                                                format!("Delete branch `{target}`?"),
                                                                "Runs `git branch -d`. Unmerged branches will not be deleted unless forced from context menu.".to_string(),
                                                                "Delete".to_string(),
                                                                window,
                                                                cx,
                                                                Rc::new(move |_, cx| {
                                                                    let bname = target.clone();
                                                                    app_del.update(cx, |app, cx| {
                                                                        app.delete_branch(bname, false, cx);
                                                                    });
                                                                }),
                                                            );
                                                        }),
                                                )
                                            }),
                                    )
                            })),
                    )
                })
                // REMOTE BRANCHES
                .child(render_section_header(
                    "remote-sec",
                    "REMOTE BRANCHES",
                    snap.remote_branches.len(),
                    app.sec_remotes_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_remotes_open = !this.sec_remotes_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_remotes_open, |col| {
                    col.child(
                        v_flex()
                            .gap_0p5()
                            .children(snap.remote_branches.iter().enumerate().map(|(i, b)| {
                                let hash_jump = b.target_hash.clone();
                                let co_target = b.name.clone();
                                let co_dbl = b.name.clone();
                                let rem_menu = b.name.clone();
                                let app_rem_menu = app_entity.clone();
                                let short_rem = rem_menu
                                    .split_once('/')
                                    .map(|(_, rest)| rest.to_string())
                                    .unwrap_or_else(|| rem_menu.clone());

                                h_flex()
                                    .id(("rem-br", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(5.))
                                    .justify_between()
                                    .items_center()
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .context_menu(move |menu, _, _| {
                                        let app_co = app_rem_menu.clone();
                                        let app_mg = app_rem_menu.clone();
                                        let app_rb = app_rem_menu.clone();
                                        let app_del = app_rem_menu.clone();
                                        let s_co = short_rem.clone();
                                        let r_mg = rem_menu.clone();
                                        let r_rb = rem_menu.clone();
                                        let r_del = rem_menu.clone();
                                        let r_copy = rem_menu.clone();
                                        menu.label(format!("Remote {rem_menu}"))
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Checkout '{short_rem}'"
                                                ))
                                                .icon(IconName::Check)
                                                .on_click(move |_, _, cx| {
                                                    let s = s_co.clone();
                                                    app_co.update(cx, |app, cx| {
                                                        app.checkout_target(s, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Merge '{rem_menu}' into current"
                                                ))
                                                .icon(IconName::GitMerge)
                                                .on_click(move |_, _, cx| {
                                                    let r = r_mg.clone();
                                                    app_mg.update(cx, |app, cx| {
                                                        app.merge_branch(r, cx);
                                                    });
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Rebase current onto '{rem_menu}'"
                                                ))
                                                .icon(IconName::GitCommitHorizontal)
                                                .on_click(move |_, _, cx| {
                                                    let r = r_rb.clone();
                                                    app_rb.update(cx, |app, cx| {
                                                        app.rebase_onto(r, cx);
                                                    });
                                                }),
                                            )
                                            .separator()
                                            .item(
                                                PopupMenuItem::new(format!(
                                                    "Delete remote '{rem_menu}'…"
                                                ))
                                                .icon(IconName::Trash)
                                                .on_click(move |_, window, cx| {
                                                    let app_d = app_del.clone();
                                                    let target = r_del.clone();
                                                    dialogs::open_confirm_danger(
                                                        format!("Delete remote branch `{target}`?"),
                                                        "Runs `git push <remote> --delete <branch>`.".to_string(),
                                                        "Delete Remote".to_string(),
                                                        window,
                                                        cx,
                                                        Rc::new(move |_, cx| {
                                                            let t = target.clone();
                                                            app_d.update(cx, |app, cx| {
                                                                app.delete_remote_branch(t, cx);
                                                            });
                                                        }),
                                                    );
                                                }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Copy remote branch name")
                                                    .icon(IconName::Copy)
                                                    .on_click(move |_, window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                r_copy.clone(),
                                                            ),
                                                        );
                                                        super::notify_info(
                                                            window,
                                                            cx,
                                                            "Copied branch name",
                                                        );
                                                    }),
                                            )
                                    })
                                    .child(
                                        h_flex()
                                            .id(("rem-br-jump", i))
                                            .flex_1()
                                            .min_w_0()
                                            .gap_1p5()
                                            .items_center()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, event: &gpui_kit::ClickEvent, _, cx| {
                                                if event.click_count() >= 2 {
                                                    let short = co_dbl
                                                        .split_once('/')
                                                        .map(|(_, rest)| rest.to_string())
                                                        .unwrap_or_else(|| co_dbl.clone());
                                                    this.checkout_target(short, cx);
                                                } else {
                                                    this.jump_to_commit(&hash_jump, cx);
                                                }
                                            }))
                                            .child(
                                                Icon::new(IconName::GitBranch)
                                                    .xsmall()
                                                    .text_color(purple),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(fg_muted)
                                                    .truncate()
                                                    .child(b.name.clone()),
                                            ),
                                    )
                                    .child(
                                        Button::new(("co-rem", i))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Check)
                                            .tooltip("Checkout remote branch")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let short = co_target
                                                    .split_once('/')
                                                    .map(|(_, rest)| rest.to_string())
                                                    .unwrap_or_else(|| co_target.clone());
                                                this.checkout_target(short, cx);
                                            })),
                                    )
                            })),
                    )
                })
                // TAGS
                .child(render_section_header(
                    "tags-sec",
                    "TAGS",
                    snap.tags.len(),
                    app.sec_tags_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_tags_open = !this.sec_tags_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_tags_open, |col| {
                    col.child(
                        v_flex()
                            .gap_0p5()
                            .when(snap.tags.is_empty(), |c| {
                                c.child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child("No tags"),
                                )
                            })
                            .children(snap.tags.iter().enumerate().map(|(i, t)| {
                                let hash_jump = t.target_hash.clone();
                                let tag_menu = t.name.clone();
                                let app_tag_menu = app_entity.clone();
                                h_flex()
                                    .id(("tag-row", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .gap_1p5()
                                    .rounded(px(5.))
                                    .items_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.jump_to_commit(&hash_jump, cx);
                                    }))
                                    .context_menu(move |menu, _, _| {
                                        let t_co = tag_menu.clone();
                                        let t_push = tag_menu.clone();
                                        let t_del = tag_menu.clone();
                                        let t_rdel = tag_menu.clone();
                                        let t_cp = tag_menu.clone();
                                        let app_co = app_tag_menu.clone();
                                        let app_p = app_tag_menu.clone();
                                        let app_d = app_tag_menu.clone();
                                        let app_rd = app_tag_menu.clone();
                                        menu.label(format!("Tag {tag_menu}"))
                                            .item(
                                                PopupMenuItem::new(format!("Checkout '{tag_menu}'"))
                                                    .icon(IconName::Check)
                                                    .on_click(move |_, _, cx| {
                                                        let n = t_co.clone();
                                                        app_co.update(cx, |a, cx| {
                                                            a.checkout_target(n, cx);
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Push tag to origin")
                                                    .icon(IconName::ArrowUp)
                                                    .on_click(move |_, _, cx| {
                                                        let n = t_push.clone();
                                                        app_p.update(cx, |a, cx| {
                                                            a.push_tag(n, cx);
                                                        });
                                                    }),
                                            )
                                            .separator()
                                            .item(
                                                PopupMenuItem::new("Delete local tag…")
                                                    .icon(IconName::Trash)
                                                    .on_click(move |_, window, cx| {
                                                        let n = t_del.clone();
                                                        let a = app_d.clone();
                                                        dialogs::open_confirm_danger(
                                                            format!("Delete local tag `{n}`?"),
                                                            "Runs `git tag -d`.".to_string(),
                                                            "Delete Tag".to_string(),
                                                            window,
                                                            cx,
                                                            Rc::new(move |_, cx| {
                                                                let tg = n.clone();
                                                                a.update(cx, |app, cx| {
                                                                    app.delete_tag(tg, cx);
                                                                });
                                                            }),
                                                        );
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Delete tag on origin…")
                                                    .icon(IconName::Trash)
                                                    .on_click(move |_, window, cx| {
                                                        let n = t_rdel.clone();
                                                        let a = app_rd.clone();
                                                        dialogs::open_confirm_danger(
                                                            format!("Delete remote tag `{n}`?"),
                                                            "Runs `git push origin --delete`.".to_string(),
                                                            "Delete Remote Tag".to_string(),
                                                            window,
                                                            cx,
                                                            Rc::new(move |_, cx| {
                                                                let tg = n.clone();
                                                                a.update(cx, |app, cx| {
                                                                    app.delete_remote_tag(tg, cx);
                                                                });
                                                            }),
                                                        );
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Copy tag name")
                                                    .icon(IconName::Copy)
                                                    .on_click(move |_, window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(t_cp.clone()),
                                                        );
                                                        super::notify_info(
                                                            window,
                                                            cx,
                                                            "Copied tag name",
                                                        );
                                                    }),
                                            )
                                    })
                                    .child(Icon::new(IconName::Tag).xsmall().text_color(yellow))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(fg_muted)
                                            .truncate()
                                            .child(t.name.clone()),
                                    )
                            })),
                    )
                })
                // STASHES
                .child(render_section_header(
                    "stash-sec",
                    "STASHES",
                    snap.stashes.len(),
                    app.sec_stashes_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_stashes_open = !this.sec_stashes_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_stashes_open, |col| {
                    col.child(
                        v_flex()
                            .gap_1()
                            .when(snap.stashes.is_empty(), |c| {
                                c.child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child("No stashes"),
                                )
                            })
                            .children(snap.stashes.iter().enumerate().map(|(i, st)| {
                                let ref_sel = st.reference.clone();
                                let msg_sel = st.message.clone();
                                let ref_pop = st.reference.clone();
                                let ref_apply = st.reference.clone();
                                let ref_br = st.reference.clone();
                                let ref_drop = st.reference.clone();
                                let app_br = app_entity.clone();
                                let app_drop = app_entity.clone();
                                let is_sel = matches!(
                                    &app.selection,
                                    Selection::Stash(r) if r == &st.reference
                                );

                                v_flex()
                                    .id(("stash-card", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .gap_1()
                                    .rounded(px(5.))
                                    .cursor_pointer()
                                    .bg(if is_sel {
                                        blue.opacity(0.14)
                                    } else {
                                        hex_color(gh_colors::CANVAS_DEFAULT)
                                    })
                                    .border_1()
                                    .border_color(if is_sel {
                                        blue.opacity(0.5)
                                    } else {
                                        hex_color(gh_colors::BORDER_MUTED)
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_stash(ref_sel.clone(), msg_sel.clone(), cx);
                                    }))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::Archive)
                                                    .xsmall()
                                                    .text_color(yellow),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(fg)
                                                    .truncate()
                                                    .child(st.message.clone()),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(
                                                Button::new(("stash-apply", i))
                                                    .xsmall()
                                                    .label("Apply")
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.stash_apply(
                                                                ref_apply.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new(("stash-pop", i))
                                                    .xsmall()
                                                    .label("Pop")
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.stash_pop(
                                                                Some(ref_pop.clone()),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new(("stash-branch", i))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::GitBranchPlus)
                                                    .tooltip("Branch from stash")
                                                    .on_click(move |_, window, cx| {
                                                        cx.stop_propagation();
                                                        dialogs::open_branch_from_stash_dialog(
                                                            app_br.clone(),
                                                            ref_br.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                            .child(
                                                Button::new(("stash-drop", i))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Trash)
                                                    .tooltip("Drop stash")
                                                    .on_click(move |_, window, cx| {
                                                        cx.stop_propagation();
                                                        let app_d = app_drop.clone();
                                                        let r = ref_drop.clone();
                                                        dialogs::open_confirm_danger(
                                                            format!("Drop {r}?"),
                                                            "This permanently deletes the stashed changes.".to_string(),
                                                            "Drop Stash".to_string(),
                                                            window,
                                                            cx,
                                                            Rc::new(move |_, cx| {
                                                                let target = r.clone();
                                                                app_d.update(cx, |app, cx| {
                                                                    app.stash_drop(target, cx);
                                                                });
                                                            }),
                                                        );
                                                    }),
                                            ),
                                    )
                            })),
                    )
                })
                // WORKTREES
                .child(render_section_header(
                    "wt-sec",
                    "WORKTREES",
                    snap.worktrees.len(),
                    app.sec_worktrees_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_worktrees_open = !this.sec_worktrees_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_worktrees_open, |col| {
                    let app_add_wt = app_entity.clone();
                    col.child(
                        v_flex()
                            .gap_1()
                            .child(
                                Button::new("wt-add-btn")
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .label("Add Worktree…")
                                    .on_click(move |_, window, cx| {
                                        reflog_dialog::open_add_worktree_dialog(
                                            app_add_wt.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                            .children(snap.worktrees.iter().enumerate().map(|(i, wt)| {
                                let label = wt
                                    .branch
                                    .clone()
                                    .unwrap_or_else(|| wt.head.clone());
                                let wt_open = wt.path.clone();
                                let wt_rm = wt.path.clone();
                                let app_rm = app_entity.clone();
                                let path_str = wt.path.display().to_string();

                                h_flex()
                                    .id(("wt-row", i))
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .gap_1()
                                    .rounded(px(5.))
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_repository(wt_open.clone(), cx);
                                    }))
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(fg)
                                                    .truncate()
                                                    .child(label),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family("monospace")
                                                    .text_color(fg_muted)
                                                    .truncate()
                                                    .child(path_str),
                                            ),
                                    )
                                    .when(!wt.is_main, |row| {
                                        row.child(
                                            Button::new(("wt-del", i))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::Trash)
                                                .tooltip("Remove worktree")
                                                .on_click(move |_, window, cx| {
                                                    cx.stop_propagation();
                                                    let p = wt_rm.clone();
                                                    let a = app_rm.clone();
                                                    dialogs::open_confirm_danger(
                                                        "Remove Worktree?".to_string(),
                                                        format!(
                                                            "Runs `git worktree remove` on `{}`.",
                                                            p.display()
                                                        ),
                                                        "Remove".to_string(),
                                                        window,
                                                        cx,
                                                        Rc::new(move |_, cx| {
                                                            let path = p.clone();
                                                            a.update(cx, |app, cx| {
                                                                app.worktree_remove(path, false, cx);
                                                            });
                                                        }),
                                                    );
                                                }),
                                        )
                                    })
                            })),
                    )
                })
                // SUBMODULES
                .child(render_section_header(
                    "sm-sec",
                    "SUBMODULES",
                    snap.submodules.len(),
                    app.sec_submodules_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_submodules_open = !this.sec_submodules_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_submodules_open, |col| {
                    col.child(
                        v_flex()
                            .gap_1()
                            .when(!snap.submodules.is_empty(), |c| {
                                c.child(
                                    Button::new("sm-update-btn")
                                        .xsmall()
                                        .icon(IconName::RefreshCw)
                                        .label("Update All (--init --recursive)")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.submodule_update(cx);
                                        })),
                                )
                            })
                            .when(snap.submodules.is_empty(), |c| {
                                c.child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .text_color(fg_muted)
                                        .child("No submodules"),
                                )
                            })
                            .children(snap.submodules.iter().enumerate().map(|(i, sm)| {
                                let (badge, col_b) = match sm.status {
                                    '-' => ("-", yellow),
                                    '+' => ("+", orange),
                                    'U' => ("U", red),
                                    _ => ("✓", green),
                                };
                                h_flex()
                                    .id(("sm-row", i))
                                    .w_full()
                                    .px_2()
                                    .py_0p5()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(col_b)
                                            .child(badge),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(fg_muted)
                                            .truncate()
                                            .child(sm.path.clone()),
                                    )
                            })),
                    )
                })
                // REMOTES CONFIG
                .child(render_section_header(
                    "rm-cfg-sec",
                    "REMOTES",
                    snap.remotes.len(),
                    app.sec_remote_configs_open,
                    cx.listener(|this, _, _, cx| {
                        this.sec_remote_configs_open = !this.sec_remote_configs_open;
                        cx.notify();
                    }),
                ))
                .when(app.sec_remote_configs_open, |col| {
                    let app_rm_dlg = app_entity.clone();
                    let remotes_clone = snap.remotes.clone();
                    col.child(
                        v_flex()
                            .gap_1()
                            .child(
                                Button::new("rm-manage-btn")
                                    .xsmall()
                                    .label("Manage Remotes…")
                                    .on_click(move |_, window, cx| {
                                        reflog_dialog::open_remotes_dialog(
                                            app_rm_dlg.clone(),
                                            remotes_clone.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                            .children(snap.remotes.iter().enumerate().map(|(i, rm)| {
                                v_flex()
                                    .id(("rm-cfg", i))
                                    .px_2()
                                    .py_0p5()
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
                                    )
                            })),
                    )
                })
                // OPEN PRS QUICK-PEEK
                .when(!app.prs.is_empty(), |col| {
                    col.child(render_section_header(
                        "prs-sec",
                        "PULL REQUESTS",
                        app.prs.len(),
                        app.sec_prs_open,
                        cx.listener(|this, _, _, cx| {
                            this.sec_prs_open = !this.sec_prs_open;
                            cx.notify();
                        }),
                    ))
                    .when(app.sec_prs_open, |col| {
                        col.child(
                            v_flex()
                                .gap_0p5()
                                .children(app.prs.iter().take(12).enumerate().map(|(i, pr)| {
                                    let num = pr.number;
                                    let dot_color = match pr.ci_state() {
                                        CiState::Success => green,
                                        CiState::Failure => red,
                                        CiState::Pending => yellow,
                                        CiState::None => fg_muted,
                                    };
                                    h_flex()
                                        .id(("sb-pr", i))
                                        .w_full()
                                        .px_2()
                                        .py_1()
                                        .gap_1p5()
                                        .rounded(px(5.))
                                        .items_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(hex_color(gh_colors::BTN_BG)))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.active_tab = ActiveTab::PullRequests;
                                            this.select_pr(num, cx);
                                        }))
                                        .child(div().size(px(6.)).rounded_full().bg(dot_color))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .text_color(blue)
                                                .flex_shrink_0()
                                                .child(format!("#{num}")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(fg_muted)
                                                .truncate()
                                                .child(pr.title.clone()),
                                        )
                                })),
                        )
                    })
                }),
        )
        .into_any_element()
}

fn render_section_header(
    id: &'static str,
    title: &'static str,
    count: usize,
    open: bool,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    let fg_muted = hex_color(gh_colors::FG_MUTED);
    h_flex()
        .id(id)
        .w_full()
        .px_1()
        .py_0p5()
        .justify_between()
        .items_center()
        .cursor_pointer()
        .on_click(on_click)
        .child(
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .xsmall()
                    .text_color(fg_muted),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(fg_muted)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(hex_color(gh_colors::FG_SUBTLE))
                .child(format!("{count}")),
        )
}
