//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use gpui::{
    AnyElement, ClickEvent, Context, Focusable as _, MouseButton, MouseDownEvent, SharedString,
    Window, div, prelude::*, px,
};

use super::{Check, Detail, checkbox_box};
use crate::app::StartCommit;
use crate::app::{AppView, MenuItem};
use crate::backend::{self, CommitState, DiffSource, FileChange, FileDiff, PushState, Selected};
use crate::state::{Cut, ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::diff;
use crate::ui::text_input::InputEvent;
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, button, empty_state, h_flex, icon, relative_time, shortcut,
    small_icon, v_flex,
};

impl Detail {
    pub(super) fn render_header(
        &self,
        selection: Option<&Selection>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let store = self.store.read(cx);
        let mut crumbs: Vec<SharedString> = Vec::new();
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(selection) = selection
            && let Some(entry) = store.project(selection.project())
        {
            crumbs.push(entry.project.name.clone().into());
            let workspace = entry.workspace.as_ref();
            match selection {
                Selection::Project(_) => {}
                Selection::Uncommitted(project) => {
                    crumbs.push("Uncommitted changes".into());
                    let unreviewed: Vec<FileChange> = workspace
                        .map(|ws| {
                            ws.uncommitted
                                .iter()
                                .filter(|file| store.review(project, file) != Review::Reviewed)
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    if !unreviewed.is_empty() {
                        let project = project.clone();
                        let store_handle = self.store.clone();
                        actions.push(
                            button("review-all", "Mark all reviewed")
                                .icon(Icon::Tick)
                                .style(ButtonStyle::Ghost)
                                .on_click(move |_, _, cx| {
                                    store_handle.update(cx, |store, cx| {
                                        store.set_reviewed(&project, &unreviewed, true, cx)
                                    })
                                })
                                .into_any_element(),
                        );
                    }
                }
                Selection::Branch(project, ref_name) => {
                    crumbs.push(short_ref(ref_name).to_owned().into());
                    if let Some(branch) = workspace.and_then(|ws| ws.find_branch(ref_name)) {
                        let (project, ref_name) = (project.clone(), ref_name.clone());
                        if branch.push.can_push() {
                            let label = if branch.push == PushState::NeedsForce {
                                "Force push"
                            } else {
                                "Push"
                            };
                            let app = self.app.clone();
                            actions.push(
                                button("push", label)
                                    .icon(Icon::ArrowUp)
                                    .style(ButtonStyle::Pop)
                                    .disabled(store.busy.is_some())
                                    .tooltip_with_key(
                                        "Push this branch and the ones below it",
                                        shortcut(&crate::app::Push, cx),
                                    )
                                    .on_click(move |_, _, cx| {
                                        app.update(cx, |app, cx| {
                                            app.push(project.clone(), ref_name.clone(), cx)
                                        })
                                        .ok();
                                    })
                                    .into_any_element(),
                            );
                        }
                    }
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        crumbs.push(branch.name.clone().into());
                        crumbs.push(commit.id.short().into());
                        let (p, id) = (project.clone(), *id);
                        let app = self.app.clone();
                        actions.push(
                            button("uncommit", "Uncommit")
                                .style(ButtonStyle::Ghost)
                                .disabled(store.busy.is_some())
                                .tooltip("Turn this commit back into uncommitted changes")
                                .on_click(move |_, _, cx| {
                                    app.update(cx, |app, cx| app.uncommit(p.clone(), id, cx))
                                        .ok();
                                })
                                .into_any_element(),
                        );
                        actions.push(
                            button("reword", "Edit message")
                                .style(ButtonStyle::Ghost)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.start_reword(window, cx)
                                    }),
                                )
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        let last = crumbs.len().saturating_sub(1);
        h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px(px(16.))
            .when(self.beside_traffic_lights, |el| el.pl(px(84.)))
            .gap(px(6.))
            .border_b_1()
            .border_color(theme.border_3)
            .children(crumbs.into_iter().enumerate().flat_map(|(ix, crumb)| {
                let sep =
                    (ix > 0).then(|| div().text_color(theme.text_3).child("/").into_any_element());
                let el = div()
                    .when(ix == last, |el| {
                        el.font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                    })
                    .when(ix != last, |el| el.text_color(theme.text_2))
                    .truncate()
                    .child(crumb)
                    .into_any_element();
                sep.into_iter().chain([el])
            }))
            .child(div().flex_1())
            .children(actions)
    }

    pub(super) fn render_project(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return empty_state("Project not found", "", theme);
        };
        let workspace = entry.workspace.as_ref();
        let expanded = entry.expanded;
        let id = entry.id.clone();
        let stat = |label: &str, value: String, color| {
            v_flex()
                .gap(px(2.))
                .px(px(14.))
                .py(px(10.))
                .min_w(px(120.))
                .rounded(px(6.))
                .border_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label.to_owned()),
                )
                .child(div().text_size(px(20.)).text_color(color).child(value))
        };
        let mut stats = h_flex().gap(px(10.));
        if let Some(ws) = workspace {
            let branches = ws.branches().count();
            stats = stats
                .child(stat("Branches", branches.to_string(), theme.text_1))
                .child(stat(
                    "Uncommitted",
                    ws.uncommitted.len().to_string(),
                    if ws.uncommitted.is_empty() {
                        theme.text_1
                    } else {
                        theme.warn
                    },
                ))
                .child(stat(
                    "Unpushed",
                    ws.unpushed_commits().to_string(),
                    if ws.unpushed_commits() == 0 {
                        theme.text_1
                    } else {
                        theme.blue
                    },
                ));
            if let Some(target) = &ws.target {
                stats = stats.child(stat(
                    &format!("Behind {}", target.name),
                    target.behind.to_string(),
                    if target.behind == 0 {
                        theme.text_1
                    } else {
                        theme.warn
                    },
                ));
            }
        }
        v_flex()
            .p(px(24.))
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(entry.project.name.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(entry.project.path.display().to_string()),
                    ),
            )
            .when_some(entry.error.clone(), |el, err| {
                el.child(
                    h_flex()
                        .gap(px(8.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(theme.danger.opacity(0.08))
                        .text_color(theme.danger)
                        .child(icon(Icon::Warning))
                        .child(div().flex_1().child(err)),
                )
            })
            .child(stats)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        button(
                            "toggle-project",
                            if expanded {
                                "Collapse"
                            } else {
                                "Show branches"
                            },
                        )
                        .style(if expanded {
                            ButtonStyle::Outline
                        } else {
                            ButtonStyle::Pop
                        })
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, _, cx| {
                                this.store
                                    .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx))
                            }
                        })),
                    )
                    .when(expanded, |el| {
                        let app = self.app.clone();
                        let fetch_id = id.clone();
                        el.child(
                            button("fetch", "Fetch")
                                .icon(Icon::Refresh)
                                .disabled(store.busy.is_some())
                                .on_click(move |_, _, cx| {
                                    app.update(cx, |app, cx| {
                                        app.fetch_project(fetch_id.clone(), cx)
                                    })
                                    .ok();
                                }),
                        )
                    })
                    .when_some(
                        workspace
                            .and_then(|ws| ws.target.as_ref())
                            .filter(|target| expanded && target.needs_update)
                            .map(|target| target.name.clone()),
                        |el, target| {
                            let app = self.app.clone();
                            let update_id = id.clone();
                            el.child(
                                button("update", format!("Update from {target}"))
                                    .icon(Icon::ArrowDown)
                                    .style(ButtonStyle::Pop)
                                    .disabled(store.busy.is_some())
                                    .on_click(move |_, _, cx| {
                                        app.update(cx, |app, cx| {
                                            app.update_project(update_id.clone(), cx)
                                        })
                                        .ok();
                                    }),
                            )
                        },
                    ),
            )
            .into_any_element()
    }

    fn render_file_row(
        &self,
        ix: usize,
        file: &FileChange,
        extras: FileRow,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (name, dir) = diff::split_path(&file.path);
        let path = file.path.clone();
        let diff_ix = self.diff_index(&file.path);
        let current = diff_ix.is_some_and(|d| d == self.cursor.file);
        let reviewed = extras.review == Some(Review::Reviewed);
        let mut row =
            h_flex()
                .id(("file", ix))
                .h(px(28.))
                .pl(px(10.))
                .pr(px(8.))
                .mx(px(6.))
                .gap(px(8.))
                .rounded(px(4.))
                .when(current, |el| el.bg(theme.selected_unfocused()))
                .when(!current, |el| el.hover(|el| el.bg(theme.hover())))
                .when_some(extras.checkbox, |el, check| {
                    let path = path.clone();
                    el.child(
                        checkbox_box(("check", ix), check, theme).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_checked(&path, cx)
                            },
                        )),
                    )
                })
                .child(
                    div()
                        .flex_none()
                        .w(px(10.))
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(diff::change_color(file.kind, theme))
                        .child(file.kind.letter()),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(6.))
                        .overflow_hidden()
                        // Reviewed files step back, as in Lite's file list.
                        .when(reviewed, |el| el.opacity(0.5))
                        .child(
                            div()
                                .flex_none()
                                .max_w(px(160.))
                                .truncate()
                                .child(name.to_owned()),
                        )
                        .child(
                            div()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(12.))
                                .text_color(theme.text_3)
                                .child(dir.to_owned()),
                        ),
                )
                .when_some(extras.stats, |el, (added, removed)| {
                    el.child(diff::stats(added, removed, theme))
                })
                .when_some(file.modified, |el, ms| {
                    el.child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(relative_time(ms / 1000)),
                    )
                })
                .when_some(extras.review, |el, review| {
                    el.child(match review {
                        Review::Reviewed => small_icon(Icon::Tick)
                            .text_color(theme.pop)
                            .into_any_element(),
                        Review::Changed => div()
                            .flex_none()
                            .size(px(6.))
                            .mx(px(3.))
                            .rounded_full()
                            .bg(theme.warn)
                            .into_any_element(),
                        Review::NotReviewed => div().flex_none().w(px(12.)).into_any_element(),
                    })
                })
                .when_some(diff_ix, |el, diff_ix| {
                    el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.scroll_to_file(diff_ix, cx)
                    }))
                });
        if let Some((project, from)) = extras.drag_from {
            let payload = Cut::Files {
                project: project.clone(),
                from,
                files: vec![file.clone()],
            };
            let label: SharedString = file.path.clone().into();
            let menu_payload = payload.clone();
            row = row
                .on_drag(payload, move |payload, _, _, cx| {
                    let label = label.clone();
                    cx.new(|_| crate::navigator::DragPreview {
                        label,
                        payload: payload.clone(),
                    })
                })
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.file_menu(menu_payload.clone(), event, window, cx)
                    }),
                );
        }
        row.into_any_element()
    }

    fn file_menu(
        &mut self,
        payload: Cut,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let Cut::Files {
            project,
            from,
            files,
        } = payload.clone()
        else {
            return;
        };
        let mut items = vec![MenuItem::new("Cut", {
            let (project, files) = (project.clone(), files.clone());
            move |this: &mut AppView, _, cx| {
                this.cut_files(project.clone(), from, files.clone(), cx)
            }
        })];
        match from {
            None => items.push(
                MenuItem::new("Discard changes…", {
                    let (project, files) = (project.clone(), files.clone());
                    move |this, window, cx| {
                        let changes = files
                            .iter()
                            .map(|file| Selected {
                                file: file.clone(),
                                hunks: None,
                            })
                            .collect();
                        this.discard(project.clone(), changes, window, cx)
                    }
                })
                .danger(),
            ),
            Some(_) => items.push(MenuItem::new("Uncommit file", move |this, window, cx| {
                this.transfer(
                    &payload,
                    &Selection::Uncommitted(project.clone()),
                    false,
                    window,
                    cx,
                );
            })),
        }
        let position = event.position;
        app.update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// What an agent left uncommitted, laid out for reviewing first. The commit box
    /// stays closed until asked for, and only then do checkboxes appear.
    pub(super) fn render_uncommitted_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return empty_state("Loading…", "", theme);
        };
        let stats_by_path = |path: &str| {
            self.diff
                .as_ref()
                .filter(|d| d.key.source == DiffSource::Worktree)
                .and_then(|d| d.files.iter().find(|f| f.file.path == path))
                .map(FileDiff::stats)
        };
        let files: Vec<AnyElement> = workspace
            .uncommitted
            .iter()
            .enumerate()
            .map(|(ix, file)| {
                let extras = FileRow {
                    stats: stats_by_path(&file.path),
                    checkbox: self.composing.then(|| self.file_check(&file.path)),
                    review: Some(store.review(project, file)),
                    drag_from: Some((project.clone(), None)),
                };
                self.render_file_row(ix, file, extras, theme, cx)
            })
            .collect();
        let (added, removed) = self.diff.as_ref().map_or((0, 0), |d| {
            d.files
                .iter()
                .map(FileDiff::stats)
                .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr))
        });
        let to_review = store.to_review(project);

        let list_header = h_flex()
            .h(px(36.))
            .px(px(16.))
            .gap(px(8.))
            .flex_none()
            .text_size(px(12.))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(
                match workspace.uncommitted.len() {
                    0 => "No changes".to_owned(),
                    1 => "1 file".to_owned(),
                    n => format!("{n} files"),
                },
            ))
            .child(diff::stats(added, removed, theme))
            .child(div().flex_1())
            .when(!workspace.uncommitted.is_empty(), |el| {
                el.child(
                    div()
                        .text_color(if to_review > 0 {
                            theme.warn
                        } else {
                            theme.text_3
                        })
                        .child(match to_review {
                            0 => "All reviewed".to_owned(),
                            n => format!("{n} to review"),
                        }),
                )
            });

        let footer = if !self.composing {
            let target = self
                .effective_target(project, cx)
                .map(|target| short_ref(&target).to_owned())
                .unwrap_or_else(|| "a new branch".to_owned());
            h_flex()
                .flex_none()
                .h(px(44.))
                .px(px(12.))
                .gap(px(8.))
                .border_t_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(12.))
                        .text_color(theme.text_3)
                        .child(format!("Commit to {target}")),
                )
                .child(
                    button("open-composer", "Commit…")
                        .small()
                        .tooltip_with_key("Write a commit message", shortcut(&StartCommit, cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                        ),
                )
                .into_any_element()
        } else {
            self.render_composer(project, theme, window, cx)
        };

        v_flex()
            .size_full()
            .child(list_header)
            .child(
                div()
                    .id("uncommitted-files")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(files)
                    .when(workspace.uncommitted.is_empty(), |el| {
                        el.child(
                            div()
                                .px(px(16.))
                                .py(px(8.))
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child("Your working directory matches the last commit"),
                        )
                    }),
            )
            .when(!workspace.uncommitted.is_empty(), |el| el.child(footer))
            .into_any_element()
    }

    fn render_composer(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return div().into_any_element();
        };
        let checked = workspace
            .uncommitted
            .iter()
            .filter(|f| self.file_check(&f.path) != Check::None)
            .count();
        let target = self.effective_target(project, cx);
        let message_focused = self.message.focus_handle(cx).is_focused(window);
        let branches: Vec<(String, String)> = workspace
            .branches()
            .filter(|b| !b.ref_name.is_empty())
            .map(|b| (b.ref_name.clone(), b.name.clone()))
            .collect();
        v_flex()
            .flex_none()
            .p(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child("Commit to")
                    .child(self.render_target_picker(project.clone(), target, branches, theme, cx)),
            )
            .child(
                div()
                    .id("message-box")
                    .min_h(px(84.))
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if message_focused {
                        theme.pop
                    } else {
                        theme.border_2
                    })
                    .bg(theme.bg_1)
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .cursor_text()
                    .on_click(cx.listener(|this, _, window, cx| this.focus_message(window, cx)))
                    .child(self.message.clone()),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(if checked == 0 {
                                "Check files to commit".to_owned()
                            } else {
                                format!("{checked} of {} files", workspace.uncommitted.len())
                            }),
                    )
                    .child(
                        button("close-composer", "Cancel")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key("Close without committing", Some("esc".into()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_composer(cx);
                                window.focus(&this.focus, cx);
                            })),
                    )
                    .child(
                        button("commit", "Commit")
                            .small()
                            .style(ButtonStyle::Pop)
                            .disabled(store.busy.is_some() || checked == 0)
                            .tooltip_with_key("Commit the checked changes", Some("⌘↩".into()))
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_target_picker(
        &self,
        project: ProjectId,
        target: Option<String>,
        branches: Vec<(String, String)>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = target
            .as_deref()
            .map(short_ref)
            .map(str::to_owned)
            .unwrap_or_else(|| "a new branch".to_owned());
        let detail = cx.entity().downgrade();
        let app = self.app.clone();
        h_flex()
            .id("commit-target")
            .h(px(22.))
            .px(px(8.))
            .gap(px(4.))
            .rounded(px(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch))
            .child(label)
            .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
            .on_click(move |event, _, cx| {
                let mut items: Vec<MenuItem> = branches
                    .iter()
                    .map(|(ref_name, name)| {
                        let detail = detail.clone();
                        let ref_name = ref_name.clone();
                        MenuItem::new(name.clone(), move |_, _, cx| {
                            detail
                                .update(cx, |detail, cx| {
                                    detail.commit_target = Some(ref_name.clone());
                                    cx.notify();
                                })
                                .ok();
                        })
                    })
                    .collect();
                let project = project.clone();
                items.push(MenuItem::new("New branch…", move |this, window, cx| {
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.select(Some(Selection::Uncommitted(project)), cx)
                    });
                    this.new_branch(&crate::app::NewBranch, window, cx);
                }));
                let position = event.position();
                app.update(cx, |app, cx| app.open_menu(position, items, cx))
                    .ok();
            })
    }

    pub(super) fn render_commit_side(
        &self,
        project: &ProjectId,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((branch, commit)) = store.workspace(project).and_then(|ws| ws.find_commit(id))
        else {
            return empty_state("Commit not found", "", theme);
        };
        let body = commit
            .message
            .split_once('\n')
            .map(|(_, body)| body.trim().to_owned())
            .unwrap_or_default();
        let state = match commit.state {
            CommitState::Local => ("Not pushed", theme.blue),
            CommitState::Pushed => ("Pushed", theme.text_2),
            CommitState::Integrated => ("Integrated", theme.safe),
        };
        let files = self.render_diff_file_list(project, Some(id), theme, cx);
        let message: AnyElement = match &self.rewording {
            Some((input, _)) => v_flex()
                .gap(px(8.))
                .child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(theme.pop)
                        .line_height(px(19.))
                        .child(input.clone()),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .justify_end()
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.))
                                .text_color(theme.text_3)
                                .child("⌘↩ to save, esc to cancel"),
                        )
                        .child(
                            button("cancel-reword", "Cancel")
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.rewording = None;
                                    cx.notify();
                                })),
                        )
                        .child({
                            let input = input.clone();
                            button("save-reword", "Save")
                                .small()
                                .style(ButtonStyle::Pop)
                                .on_click(move |_, _, cx| {
                                    input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                                })
                        }),
                )
                .into_any_element(),
            None => v_flex()
                .id("commit-message")
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(21.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(if commit.title.is_empty() {
                            "No message".to_owned()
                        } else {
                            commit.title.clone()
                        }),
                )
                .when(!body.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(theme.text_2)
                            .child(body),
                    )
                })
                .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.start_reword(window, cx)
                    }
                }))
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .flex_none()
                    .p(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(message)
                    .when(commit.conflicted, |el| {
                        el.child(
                            h_flex()
                                .gap(px(8.))
                                .px(px(10.))
                                .py(px(8.))
                                .rounded(px(6.))
                                .bg(theme.danger.opacity(0.08))
                                .text_size(px(12.))
                                .text_color(theme.danger)
                                .child(small_icon(Icon::Warning))
                                .child(div().flex_1().child(
                                    "This commit has conflicts. Resolve them with `but resolve` or the GitButler app, or undo with ⌘Z.",
                                )),
                        )
                    })
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .flex_wrap()
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(commit.author.clone())
                            .child("·")
                            .child(relative_time(commit.time))
                            .child("·")
                            .child(div().text_color(state.1).child(state.0))
                            .when(commit.conflicted, |el| {
                                el.child("·")
                                    .child(div().text_color(theme.danger).child("Conflicted"))
                            }),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .child(small_icon(Icon::Branch).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_color(theme.blue)
                                    .child(branch.name.clone()),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_color(theme.text_3)
                                    .child(commit.id.short()),
                            ),
                    ),
            )
            .child(files)
            .into_any_element()
    }

    pub(super) fn render_branch_side(
        &self,
        project: &ProjectId,
        ref_name: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(branch) = store
            .workspace(project)
            .and_then(|ws| ws.find_branch(ref_name))
        else {
            return empty_state("Branch not found", "", theme);
        };
        let push = match branch.push {
            PushState::NothingToPush => "Up to date with its remote",
            PushState::Unpushed => "Has unpushed commits",
            PushState::NeedsForce => "Rewritten, needs a force push",
            PushState::NeverPushed => "Not pushed yet",
            PushState::Integrated => "Integrated",
        };
        let commits: Vec<AnyElement> = branch
            .commits
            .iter()
            .enumerate()
            .map(|(ix, commit)| {
                let selection = Selection::Commit(project.clone(), commit.id);
                let store = self.store.clone();
                h_flex()
                    .id(("branch-commit", ix))
                    .h(px(28.))
                    .px(px(10.))
                    .mx(px(6.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(
                        small_icon(match commit.state {
                            CommitState::Local => Icon::Commit,
                            CommitState::Pushed => Icon::CommitFill,
                            CommitState::Integrated => Icon::Tick,
                        })
                        .text_color(
                            if commit.state == CommitState::Local {
                                theme.blue
                            } else {
                                theme.text_3
                            },
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(relative_time(commit.time)),
                    )
                    .on_click(move |_, _, cx| {
                        store.update(cx, |store, cx| store.select(Some(selection.clone()), cx))
                    })
                    .into_any_element()
            })
            .collect();
        let files = self.render_diff_file_list(project, None, theme, cx);
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .flex_none()
                    .p(px(16.))
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(icon(Icon::Branch).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_size(px(14.))
                                    .text_color(theme.blue)
                                    .truncate()
                                    .child(branch.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(format!("{} commits · {push}", branch.commits.len())),
                    ),
            )
            .child(
                div()
                    .id("branch-commits")
                    .flex_none()
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .py(px(4.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .children(commits)
                    .when(branch.commits.is_empty(), |el| {
                        el.child(div().px(px(16.)).py(px(6.)).text_size(px(12.)).text_color(theme.text_2).child(
                            "No commits yet. Drop uncommitted files here to commit to this branch.",
                        ))
                    }),
            )
            .child(files)
            .into_any_element()
    }

    /// The files of the loaded commit or branch diff, each a jump target and draggable
    /// when it belongs to a single commit.
    pub(super) fn render_diff_file_list(
        &self,
        project: &ProjectId,
        commit: Option<backend::CommitId>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(diff) = &self.diff else {
            return div()
                .px(px(16.))
                .py(px(10.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(if self.loading.is_some() {
                    "Loading…"
                } else {
                    ""
                })
                .into_any_element();
        };
        let rows: Vec<AnyElement> = diff
            .files
            .iter()
            .enumerate()
            .map(|(ix, file)| {
                let extras = FileRow {
                    stats: Some(file.stats()),
                    drag_from: commit.map(|id| (project.clone(), Some(id))),
                    ..Default::default()
                };
                self.render_file_row(ix, &file.file, extras, theme, cx)
            })
            .collect();
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(16.))
                    .flex_none()
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(match diff.files.len() {
                        1 => "1 file changed".to_owned(),
                        n => format!("{n} files changed"),
                    }),
            )
            .child(
                div()
                    .id("diff-files")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }
}

/// What a file row shows besides the file itself.
#[derive(Default)]
struct FileRow {
    stats: Option<(u32, u32)>,
    /// Shown while composing a commit.
    checkbox: Option<Check>,
    /// Shown for uncommitted files.
    review: Option<Review>,
    /// Lets the row be dragged and right-clicked as these changes.
    drag_from: Option<(ProjectId, Option<backend::CommitId>)>,
}
