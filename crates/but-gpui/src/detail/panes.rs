//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, Hsla, MouseButton, MouseDownEvent,
    SharedString, Stateful, Toggled, Window, div, prelude::*, px, uniform_list,
};

use super::file_tree::{self, TreeRow};
use super::{Detail, checkbox_box};
use crate::app::StartCommit;
use crate::app::{AppView, MenuItem};
use crate::backend::{self, CommitState, DiffSource, FileChange, FileDiff, PushState, Selected};
use crate::state::{Cut, ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::diff;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::text_input::InputEvent;
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, button, empty_state, h_flex, icon, icon_button, list_row,
    relative_time, shortcut, small_icon, v_flex,
};

impl Detail {
    pub(super) fn render_header(
        &self,
        selection: Option<&Selection>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let store = self.store.read(cx);
        // Each crumb but the last links to what it names.
        let mut crumbs: Vec<(SharedString, Option<Selection>)> = Vec::new();
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(selection) = selection
            && let Some(entry) = store.project(selection.project())
        {
            crumbs.push((
                entry.project.name.clone().into(),
                Some(Selection::Project(entry.id.clone())),
            ));
            let workspace = entry.workspace.as_ref();
            match selection {
                Selection::Project(_) => {}
                Selection::Uncommitted(project) => {
                    crumbs.push(("Uncommitted changes".into(), None));
                    if store.to_review(project) > 0 {
                        let project = project.clone();
                        let store_handle = self.store.clone();
                        actions.push(
                            button("review-all", "Mark all reviewed")
                                .icon(Icon::Tick)
                                .style(ButtonStyle::Ghost)
                                .on_click(move |_, _, cx| {
                                    store_handle.update(cx, |store, cx| {
                                        let unreviewed: Vec<FileChange> = store
                                            .workspace(&project)
                                            .map(|ws| ws.uncommitted.clone())
                                            .unwrap_or_default()
                                            .into_iter()
                                            .filter(|f| {
                                                store.review(&project, f) != Review::Reviewed
                                            })
                                            .collect();
                                        store.set_reviewed(&project, &unreviewed, true, cx)
                                    })
                                })
                                .into_any_element(),
                        );
                    }
                }
                Selection::Branch(_, ref_name) => {
                    crumbs.push((short_ref(ref_name).to_owned().into(), None));
                    if let Some(branch) = workspace.and_then(|ws| ws.find_branch(ref_name))
                        && branch.push.can_push()
                    {
                        let label = if branch.push == PushState::NeedsForce {
                            "Force push"
                        } else {
                            "Push"
                        };
                        actions.push(
                            button("push", label)
                                .icon(Icon::ArrowUp)
                                .style(ButtonStyle::Pop)
                                .tooltip_with_key(
                                    "Push this branch and the ones below it",
                                    shortcut(&crate::app::Push, cx),
                                )
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(crate::app::Push), cx)
                                })
                                .into_any_element(),
                        );
                    }
                }
                Selection::History(project) => {
                    crumbs.push(("History".into(), None));
                    let snapshot = cx.listener({
                        let project = project.clone();
                        move |this, _: &ClickEvent, _, cx| this.take_snapshot(&project, cx)
                    });
                    actions.push(
                        button("snapshot", "Take snapshot")
                            .style(ButtonStyle::Ghost)
                            .tooltip("Save how the project is now, to come back to later")
                            .on_click(snapshot)
                            .into_any_element(),
                    );
                    if self.selected_operation(project).is_some() {
                        let project = project.clone();
                        actions.push(
                            button("restore", "Restore to before this")
                                .style(ButtonStyle::Gray)
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.restore_selected(&project, window, cx)
                                }))
                                .into_any_element(),
                        );
                    }
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        let branch_selection =
                            Selection::Branch(project.clone(), branch.ref_name.clone());
                        crumbs.push((branch.name.clone().into(), Some(branch_selection)));
                        crumbs.push((commit.id.short().into(), None));
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
            .children(
                crumbs
                    .into_iter()
                    .enumerate()
                    .flat_map(|(ix, (crumb, target))| {
                        let sep = (ix > 0)
                            .then(|| div().text_color(theme.text_3).child("/").into_any_element());
                        let el = match target.filter(|_| ix != last) {
                            Some(target) => self
                                .link(("crumb", ix), target, theme.text_1)
                                .text_color(theme.text_2)
                                .truncate()
                                .child(crumb)
                                .into_any_element(),
                            None => div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_1)
                                .truncate()
                                .child(crumb)
                                .into_any_element(),
                        };
                        sep.into_iter().chain([el])
                    }),
            )
            .child(div().flex_1())
            .children(actions)
    }

    /// Something that selects `target` when clicked, underlined and in `hover` under the
    /// pointer.
    fn link(&self, id: impl Into<ElementId>, target: Selection, hover: Hsla) -> Stateful<Div> {
        let app = self.app.clone();
        div()
            .id(id)
            .cursor_pointer()
            .hover(move |el| el.underline().text_color(hover))
            .on_click(move |_, window, cx| {
                app.update(cx, |app, cx| app.select(target.clone(), window, cx))
                    .ok();
            })
    }

    /// A project's name, path and load error. Its counts and actions are on its row in the
    /// navigator.
    pub(super) fn render_project(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(entry) = self.store.read(cx).project(project) else {
            return empty_state("Project not found", "", theme);
        };
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
            .into_any_element()
    }

    /// A file list, flat or as a tree as the user chose, drawn only as far as it's on
    /// screen: an agent can leave thousands of files.
    fn file_list(&self, id: &'static str, source: FileSource, cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let tree = store.file_tree;
        // Each shown file's position in its source, as files a pending operation takes away
        // aren't shown.
        let (index, paths): (Vec<usize>, Vec<&str>) = match &source {
            FileSource::Uncommitted(project) => store
                .workspace(project)
                .map(|ws| {
                    ws.uncommitted
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| !store.hides_file(project, &f.path))
                        .map(|(pos, f)| (pos, f.path.as_str()))
                        .unzip()
                })
                .unwrap_or_default(),
            FileSource::Diff(..) => self
                .diff
                .as_ref()
                .map(|diff| {
                    diff.files
                        .iter()
                        .enumerate()
                        .map(|(pos, f)| (pos, f.file.path.as_str()))
                        .unzip()
                })
                .unwrap_or_default(),
        };
        let rows: Vec<TreeRow> = if tree {
            file_tree::tree_rows(&paths, &self.collapsed_dirs)
        } else {
            (0..paths.len())
                .map(|pos| TreeRow::File { pos, depth: 0 })
                .collect()
        };
        let handle = match source {
            FileSource::Uncommitted(_) => self.uncommitted_files.clone(),
            FileSource::Diff(..) => self.diff_files.clone(),
        };
        let list = uniform_list(
            id,
            rows.len(),
            cx.processor(move |this, range: Range<usize>, _, cx| {
                let theme = Theme::get(cx).clone();
                range
                    .filter_map(|ix| match &rows[ix] {
                        dir @ TreeRow::Dir { .. } => this.render_dir_row(dir, &theme, cx),
                        TreeRow::File { pos, depth } => {
                            let depth = tree.then_some(*depth);
                            this.render_source_file(&source, index[*pos], depth, &theme, cx)
                        }
                    })
                    // A list item is as wide as its content unless a column stretches it.
                    .map(|row| v_flex().w_full().child(row))
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&handle)
        .size_full();
        let theme = Theme::get(cx);
        div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .child(list)
            .children(scrollbar("files-scrollbar", Scroll::Uniform(handle), theme))
            .into_any_element()
    }

    /// The `pos`th file of `source`, with what its list shows beside it.
    fn render_source_file(
        &self,
        source: &FileSource,
        pos: usize,
        depth: Option<usize>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        match source {
            FileSource::Uncommitted(project) => {
                let store = self.store.read(cx);
                let file = store.workspace(project)?.uncommitted.get(pos)?;
                let stats = self
                    .diff
                    .as_ref()
                    .filter(|diff| diff.key.source == DiffSource::Worktree)
                    .and_then(|diff| diff.files.iter().find(|f| f.file.path == file.path))
                    .map(FileDiff::stats);
                let extras = FileRow {
                    stats,
                    checkbox: self.composing.then(|| self.file_check(&file.path)),
                    review: Some(store.review(project, file)),
                    drag_from: Some((project.clone(), None)),
                    depth,
                };
                Some(self.render_file_row(pos, file, extras, theme, cx))
            }
            FileSource::Diff(project, commit) => {
                let file = self.diff.as_ref()?.files.get(pos)?;
                let extras = FileRow {
                    stats: Some(file.stats()),
                    drag_from: commit.map(|id| (project.clone(), Some(id))),
                    depth,
                    ..Default::default()
                };
                Some(self.render_file_row(pos, &file.file, extras, theme, cx))
            }
        }
    }

    fn render_dir_row(
        &self,
        row: &TreeRow,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let TreeRow::Dir {
            path,
            name,
            depth,
            collapsed,
            files,
        } = row.clone()
        else {
            return None;
        };
        let row = list_row(ElementId::Name(format!("dir-{path}").into()), false, theme)
            .pl(px(10. + TREE_INDENT * depth as f32))
            .gap(px(6.))
            .text_color(theme.text_2)
            .child(
                small_icon(if collapsed {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                })
                .text_color(theme.text_3),
            )
            .child(small_icon(Icon::Folder).text_color(theme.text_3))
            .child(div().flex_1().min_w(px(0.)).truncate().child(name))
            .when(collapsed, |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_3)
                        .child(files.to_string()),
                )
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed_dirs.remove(&path) {
                    this.collapsed_dirs.insert(path.clone());
                }
                cx.notify();
            }));
        Some(row.into_any_element())
    }

    fn tree_toggle(&self, cx: &Context<Self>) -> impl IntoElement {
        let tree = self.store.read(cx).file_tree;
        let store = self.store.clone();
        icon_button(
            "file-tree",
            if tree { Icon::List } else { Icon::FolderTree },
        )
        .small()
        .tooltip(if tree { "Show as list" } else { "Show as tree" })
        .on_click(move |_, _, cx| {
            store.update(cx, |store, cx| store.set_file_tree(!tree, cx));
        })
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
        // In a tree a file lines up with its directory's name, and the path is the tree.
        let indent = extras
            .depth
            .map_or(0., |depth| TREE_INDENT * depth as f32 + 18.);
        let dir = if extras.depth.is_some() { "" } else { dir };
        let mut row = list_row(("file", ix), current, theme)
            .aria_label(file.path.clone())
            .aria_description(file.kind.word())
            .pl(px(10. + indent))
            .when_some(extras.checkbox, |el, check| {
                let path = path.clone();
                el.child(
                    checkbox_box(("check", ix), check, theme)
                        // Checking a file doesn't scroll to it.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.toggle_checked(&path, cx)
                        })),
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
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.scroll_to_file(diff_ix, cx)
                    }),
                )
            });
        if let Some((project, from)) = extras.drag_from {
            let payload = Cut::Files {
                project: project.clone(),
                from,
                files: vec![Selected {
                    file: file.clone(),
                    hunks: None,
                }]
                .into(),
            };
            let label: SharedString = file.path.clone().into();
            let menu_payload = payload.clone();
            row = row
                .on_drag(
                    payload,
                    crate::navigator::drag_preview(self.store.clone(), label),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.file_menu(menu_payload.clone(), event, cx)
                    }),
                );
        }
        row.into_any_element()
    }

    fn file_menu(&mut self, payload: Cut, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let Cut::Files {
            project,
            from,
            files,
        } = payload.clone()
        else {
            return;
        };
        let cut = payload.clone();
        let mut items = vec![MenuItem::new("Cut", move |this: &mut AppView, _, cx| {
            this.cut(cut.clone(), cx)
        })];
        items.push(match from {
            None => MenuItem::new("Discard changes…", move |this, window, cx| {
                this.discard(project.clone(), files.to_vec(), window, cx)
            })
            .danger(),
            Some(_) => MenuItem::new("Uncommit file", move |this, window, cx| {
                let uncommitted = Selection::Uncommitted(project.clone());
                this.transfer(&payload, &uncommitted, false, window, cx);
            }),
        });
        let position = event.position;
        self.app
            .update(cx, |app, cx| app.open_menu(position, items, cx))
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
        let files = self.file_list(
            "uncommitted-files",
            FileSource::Uncommitted(project.clone()),
            cx,
        );
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
            .child(self.tree_toggle(cx))
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
            .child(v_flex().flex_1().min_h(px(0.)).child(files).when(
                workspace.uncommitted.is_empty(),
                |el| {
                    el.child(
                        div()
                            .px(px(16.))
                            .py(px(8.))
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child("Your working directory matches the last commit"),
                    )
                },
            ))
            .when(!workspace.uncommitted.is_empty(), |el| el.child(footer))
            .into_any_element()
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
            // The title alone, as in Lite: the body opens on request.
            None => v_flex()
                .id("commit-message")
                .gap(px(8.))
                .child(
                    h_flex()
                        .items_start()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
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
                            let open = self.message_open;
                            el.child(
                                icon_button("full-message", Icon::TextBlock)
                                    .small()
                                    .tooltip(if open {
                                        "Hide full message"
                                    } else {
                                        "Full message"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message_open = !this.message_open;
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .when(self.message_open && !body.is_empty(), |el| {
                    el.child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
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
                                self.link(
                                    "commit-branch",
                                    Selection::Branch(project.clone(), branch.ref_name.clone()),
                                    theme.blue,
                                )
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
            .when_some(branch.review.as_ref(), |el, review| {
                el.child(render_review(review, theme, cx))
            })
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
        let Some(diff) = self.shown_diff() else {
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
        let rows = self.file_list("diff-files", FileSource::Diff(project.clone(), commit), cx);
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(16.))
                    .flex_none()
                    .text_size(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(match diff.files.len() {
                                1 => "1 file changed".to_owned(),
                                n => format!("{n} files changed"),
                            }),
                    )
                    .child(self.tree_toggle(cx)),
            )
            .child(rows)
            .into_any_element()
    }
}

/// A branch's pull request in two lines, as t3code shows one: what it is, then its number,
/// state and checks, with a way to open it.
fn render_review(review: &backend::Review, theme: &Theme, cx: &Context<Detail>) -> AnyElement {
    let (glyph, color) = ui_review::state_glyph(review.state, theme);
    let title = if review.title.is_empty() {
        format!("Pull request #{}", review.number)
    } else {
        review.title.clone()
    };
    v_flex()
        .flex_none()
        .px(px(16.))
        .py(px(12.))
        .gap(px(4.))
        .border_b_1()
        .border_color(theme.border_3)
        .child(
            h_flex()
                .gap(px(8.))
                .child(icon(glyph).text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .when_some(review.url.clone(), |el, url| {
                    el.child(
                        button("open-review", "Open")
                            .icon(Icon::Link)
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key(
                                "Open the pull request",
                                shortcut(&crate::app::OpenPullRequest, cx),
                            )
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    )
                }),
        )
        .child(
            h_flex()
                .pl(px(24.))
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(format!("#{}", review.number))
                .child("·")
                .child(ui_review::state_label(review.state))
                .when_some(review.checks, |el, checks| {
                    let (glyph, color, text) = ui_review::checks_summary(checks, theme);
                    el.child("·")
                        .child(small_icon(glyph).text_color(color))
                        .child(text)
                }),
        )
        .into_any_element()
}

/// Where a file list's files come from.
enum FileSource {
    Uncommitted(ProjectId),
    /// The loaded diff's files: a commit's, or a branch's with no commit.
    Diff(ProjectId, Option<backend::CommitId>),
}

/// What a file row shows besides the file itself.
#[derive(Default)]
struct FileRow {
    stats: Option<(u32, u32)>,
    /// Shown while composing a commit.
    checkbox: Option<Toggled>,
    /// Shown for uncommitted files.
    review: Option<Review>,
    /// Lets the row be dragged and right-clicked as these changes.
    drag_from: Option<(ProjectId, Option<backend::CommitId>)>,
    /// How deep the file sits in a tree; it then shows only its name.
    depth: Option<usize>,
}

/// How far each level of a file tree steps in.
const TREE_INDENT: f32 = 14.;
