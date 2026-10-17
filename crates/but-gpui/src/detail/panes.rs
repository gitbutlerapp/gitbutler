//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, Hsla, MouseButton, MouseDownEvent, Pixels,
    Point, SharedString, Stateful, Toggled, Window, div, prelude::*, px, uniform_list,
};

use super::file_tree::{self, TreeRow};
use super::{Detail, FilterFiles, checkbox_box, keeps_path};
use crate::app::StartCommit;
use crate::app::{AppView, MenuItem};
use crate::backend::{self, CommitState, DiffSource, FileChange, FileDiff, PushState, Selected};
use crate::state::{Cut, Overlay, ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::diff;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::InputEvent;
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, button, dot, empty_state, h_flex, icon, icon_button,
    list_row, plural, relative_time, shortcut, small_icon, v_flex,
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
                Selection::Project(_) => {
                    let needs_update = workspace
                        .and_then(|ws| ws.target.as_ref())
                        .filter(|target| target.needs_update)
                        .map(|target| target.name.clone());
                    actions.push(
                        button("fetch", "Fetch")
                            .icon(Icon::Refresh)
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key(
                                "Fetch from the project's remotes",
                                shortcut(&crate::app::Fetch, cx),
                            )
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(crate::app::Fetch), cx)
                            })
                            .into_any_element(),
                    );
                    if let Some(target) = needs_update {
                        actions.push(
                            button("update", format!("Update from {target}"))
                                .icon(Icon::ArrowDown)
                                .style(ButtonStyle::Pop)
                                .tooltip_with_key(
                                    "Rebase every stack onto the target's new commits",
                                    shortcut(&crate::app::UpdateFromTarget, cx),
                                )
                                .on_click(|_, window, cx| {
                                    window
                                        .dispatch_action(Box::new(crate::app::UpdateFromTarget), cx)
                                })
                                .into_any_element(),
                        );
                    }
                }
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
                    let can_create = workspace
                        .and_then(|ws| ws.find_branch(ref_name))
                        .is_some_and(|b| b.review.is_none() && !b.commits.is_empty())
                        && crate::pr::on_forge(store, selection);
                    if can_create {
                        actions.push(
                            button("create-pr", "Create pull request")
                                .icon(Icon::Pr)
                                .style(ButtonStyle::Ghost)
                                .on_click(|_, window, cx| {
                                    window
                                        .dispatch_action(Box::new(crate::pr::CreatePullRequest), cx)
                                })
                                .into_any_element(),
                        );
                    }
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
                Selection::Branches(project) => {
                    crumbs.push(("Branches".into(), Some(selection.clone())));
                    if let Some(branch) = self.selected_other_branch(project) {
                        crumbs.push((branch.name.clone().into(), None));
                        let local = branch.local;
                        if let Some(summary) = self.other_branch_summary(project) {
                            actions.push(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.text_2)
                                    .child(summary)
                                    .into_any_element(),
                            );
                        }
                        if local {
                            let project = project.clone();
                            actions.push(
                                button("delete-other", "Delete…")
                                    .style(ButtonStyle::Ghost)
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.delete_other_branch(&project, window, cx)
                                        },
                                    ))
                                    .into_any_element(),
                            );
                        }
                        let project = project.clone();
                        actions.push(
                            button("apply-branch", "Apply")
                                .style(ButtonStyle::Pop)
                                .tooltip("Add it to the workspace, beside your other branches")
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.apply_branch(&project, cx)
                                }))
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

    /// A file list, flat or as a tree as the user chose, drawn only as far as it's on
    /// screen: an agent can leave thousands of files.
    fn file_list(&self, id: &'static str, source: FileSource, cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let tree = store.file_tree;
        let terms = self.filter_terms(cx);
        // Each shown file's position in its source, as files a pending operation takes away
        // aren't shown.
        let (index, paths): (Vec<usize>, Vec<&str>) = match &source {
            FileSource::Uncommitted(project) => store
                .workspace(project)
                .map(|ws| {
                    ws.uncommitted
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            !store.hides_file(project, &f.path) && keeps_path(&terms, &f.path)
                        })
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
                        .filter(|(_, f)| keeps_path(&terms, &f.file.path))
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
                        dir @ TreeRow::Dir { .. } => this.render_dir_row(dir, &source, &theme, cx),
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
        scrolled(list, "files-scrollbar", Scroll::Uniform(handle), theme).into_any_element()
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

    /// The files under the folder `dir` in `source`, to act on together.
    fn folder_payload(&self, source: &FileSource, dir: &str, cx: &gpui::App) -> Option<Cut> {
        let prefix = format!("{dir}/");
        // Only what the folder shows: not files the filter or a pending operation hides.
        let terms = self.filter_terms(cx);
        let under = |path: &str| path.starts_with(&prefix) && keeps_path(&terms, path);
        let store = self.store.read(cx);
        let (project, from, files): (_, _, Vec<FileChange>) = match source {
            FileSource::Uncommitted(project) => (
                project.clone(),
                None,
                (store.workspace(project)?.uncommitted.iter())
                    .filter(|f| under(&f.path) && !store.hides_file(project, &f.path))
                    .cloned()
                    .collect(),
            ),
            FileSource::Diff(project, Some(commit)) => (
                project.clone(),
                Some(*commit),
                self.diff
                    .as_ref()?
                    .files
                    .iter()
                    .map(|f| f.file.clone())
                    .filter(|f| under(&f.path))
                    .collect(),
            ),
            FileSource::Diff(_, None) => return None,
        };
        let files: Vec<Selected> = files
            .into_iter()
            .map(|file| Selected { file, hunks: None })
            .collect();
        (!files.is_empty()).then(|| Cut::Files {
            project,
            from,
            files: files.into(),
        })
    }

    fn render_dir_row(
        &self,
        row: &TreeRow,
        source: &FileSource,
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
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let (source, path) = (source.clone(), path.clone());
                    move |this, event: &MouseDownEvent, _, cx| {
                        if let Some(payload) = this.folder_payload(&source, &path, cx) {
                            // A folder: no one file to open.
                            this.changes_menu(payload, None, event.position, cx)
                        }
                    }
                }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed_dirs.remove(&path) {
                    this.collapsed_dirs.insert(path.clone());
                }
                cx.notify();
            }));
        Some(row.into_any_element())
    }

    /// A file list's header while the file filter is open: the filter, how many of the
    /// list's `paths` it keeps, and a way to close it.
    fn filter_header<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let terms = self.filter_terms(cx);
        let (kept, total) = paths.fold((0, 0), |(kept, total), path| {
            (kept + usize::from(keeps_path(&terms, path)), total + 1)
        });
        h_flex()
            .h(px(36.))
            .pl(px(16.))
            .pr(px(8.))
            .gap(px(8.))
            .flex_none()
            .text_size(px(12.))
            .child(small_icon(Icon::Search).text_color(theme.text_3))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_size(px(13.))
                    .child(self.file_filter.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(theme.text_3)
                    .child(format!("{kept} of {total}")),
            )
            .child(
                icon_button("close-filter", Icon::Cross)
                    .small()
                    .tooltip("Close the filter")
                    .on_click(cx.listener(|this, _, window, cx| this.close_filter(window, cx))),
            )
    }

    fn filter_button(&self, cx: &Context<Self>) -> impl IntoElement {
        icon_button("filter-files", Icon::Search)
            .small()
            .tooltip_with_key("Filter files", shortcut(&FilterFiles, cx))
            .on_click(cx.listener(|this, _, window, cx| this.open_filter(window, cx)))
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
            .child(if file.conflicted {
                small_icon(Icon::Warning)
                    .size(px(10.))
                    .text_color(theme.danger)
                    .into_any_element()
            } else {
                div()
                    .flex_none()
                    .w(px(10.))
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(diff::change_color(file.kind, theme))
                    .child(file.kind.letter())
                    .into_any_element()
            })
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
            // A conflicted file says so where the others say when they changed.
            .children(
                match (file.conflicted, file.modified) {
                    (true, _) => Some((theme.danger, "conflicted".to_owned())),
                    (false, Some(ms)) => Some((theme.text_3, relative_time(ms / 1000))),
                    (false, None) => None,
                }
                .map(|(color, text)| {
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(color)
                        .child(text)
                }),
            )
            .when_some(extras.review, |el, review| {
                el.child(match review {
                    Review::Reviewed => small_icon(Icon::Tick)
                        .text_color(theme.pop)
                        .into_any_element(),
                    Review::Changed => dot(theme.warn).mx(px(3.)).into_any_element(),
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
                project,
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
                        this.changes_menu(menu_payload.clone(), Some(None), event.position, cx)
                    }),
                );
        }
        row.into_any_element()
    }

    /// What right-clicking files, a folder or a hunk offers: moving them, taking them out,
    /// and for a single file (`line` where an editor should open it) opening or copying
    /// its path; a folder's menu has no `line`.
    pub(super) fn changes_menu(
        &mut self,
        payload: Cut,
        line: Option<Option<u32>>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Cut::Files {
            project,
            from,
            files,
        } = payload.clone()
        else {
            return;
        };
        let hunk = files.iter().any(|f| f.hunks.is_some());
        let what = match (hunk, files.len()) {
            (true, _) => "hunk",
            (false, 1) => "file",
            (false, _) => "files",
        };
        let cut = payload.clone();
        let mut items = vec![MenuItem::new("Cut", move |this: &mut AppView, _, cx| {
            this.cut(cut.clone(), cx)
        })];
        match from {
            None => {
                let conflicted: Vec<String> = (files.iter())
                    .filter(|f| f.file.conflicted)
                    .map(|f| f.file.path.clone())
                    .collect();
                if !conflicted.is_empty() {
                    let project = project.clone();
                    items.insert(
                        0,
                        MenuItem::new("Mark resolved", move |this, _, cx| {
                            let files = conflicted.clone();
                            this.store.update(cx, |store, cx| {
                                store.run(
                                    &project,
                                    "Mark resolved",
                                    Overlay::default(),
                                    move |path, _| backend::mark_resolved(path, files),
                                    |store, (), cx| store.snack_undoable("Marked resolved", cx),
                                    cx,
                                )
                            })
                        }),
                    );
                }
                let (review_project, review_files) = (project.clone(), files.clone());
                let (project, files) = (project.clone(), files.clone());
                items.extend([
                    MenuItem::new("Mark reviewed", move |this, _, cx| {
                        let changes: Vec<FileChange> =
                            review_files.iter().map(|f| f.file.clone()).collect();
                        this.store.update(cx, |store, cx| {
                            store.set_reviewed(&review_project, &changes, true, cx)
                        })
                    }),
                    MenuItem::new("Discard changes…", move |this, window, cx| {
                        this.discard(project.clone(), files.to_vec(), window, cx)
                    })
                    .danger(),
                ]);
            }
            Some(commit) => {
                let uncommitted = Selection::Uncommitted(project.clone());
                let (split_project, split_files) = (project.clone(), files.clone());
                let (drop_project, drop_files) = (project.clone(), files.clone());
                items.extend([
                    MenuItem::new(format!("Uncommit {what}"), move |this, window, cx| {
                        this.transfer(&payload, &uncommitted, Default::default(), window, cx);
                    }),
                    MenuItem::new("Split into a new commit", move |this, _, cx| {
                        this.split_off(split_project.clone(), commit, split_files.clone(), cx)
                    }),
                    MenuItem::new(
                        format!("Discard {what} from commit…"),
                        move |this, window, cx| {
                            this.discard_from_commit(
                                drop_project.clone(),
                                commit,
                                drop_files.clone(),
                                window,
                                cx,
                            )
                        },
                    )
                    .danger(),
                ]);
            }
        }
        if let [file] = &*files
            && let Some(line) = line
            && let Some(root) = self.store.read(cx).path(&project)
        {
            let relative = file.file.path.clone();
            let absolute = root.join(&relative);
            let full = absolute.display().to_string();
            let (open_project, open_file) = (project.clone(), relative.clone());
            let label = self
                .app
                .upgrade()
                .map(|app| app.read(cx).open_in_editor_label(cx))
                .unwrap_or_default();
            items.extend([
                MenuItem::new(label, move |this, _, cx| {
                    this.open_in_editor(&open_project, Some(open_file.clone()), line, position, cx)
                })
                .separated(),
                MenuItem::new("Copy path", move |this, _, cx| {
                    this.copy_text(relative.clone(), cx)
                }),
                MenuItem::new("Copy full path", move |this, _, cx| {
                    this.copy_text(full.clone(), cx)
                }),
                MenuItem::new("Reveal in Finder", move |_, _, cx| {
                    cx.reveal_path(&absolute)
                }),
            ]);
        }
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

        let paths = workspace.uncommitted.iter().map(|f| f.path.as_str());
        let list_header = if self.filtering {
            self.filter_header(paths, theme, cx)
        } else {
            h_flex()
                .h(px(36.))
                .px(px(16.))
                .gap(px(8.))
                .flex_none()
                .text_size(px(12.))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(
                    match workspace.uncommitted.len() {
                        0 => "No changes".to_owned(),
                        n => plural(n, "file"),
                    },
                ))
                .child(diff::stats(added, removed, theme))
                .child(div().flex_1())
                .child(self.filter_button(cx))
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
                })
        };

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
                        el.child(conflict_bar(
                            "This commit has conflicts".into(),
                            "Resolve…".into(),
                            theme,
                            |_, window, cx| {
                                window
                                    .dispatch_action(Box::new(crate::resolve::ResolveConflicts), cx)
                            },
                        ))
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
        // Resolving the oldest conflict first, even in a branch below, rebases the rest onto
        // its result. Commits list newest first, and a stack's branches top first.
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        let from_bottom: Vec<&backend::Branch> = (store.workspace(project))
            .and_then(|ws| ws.stack_of(ref_name))
            .map(|(stack, at)| stack.branches[at..].iter().rev().collect())
            .unwrap_or_default();
        let oldest = from_bottom.into_iter().find_map(|holder| {
            let commit = holder.commits.iter().rev().find(|c| c.conflicted)?;
            Some((holder, commit))
        });
        let resolve_oldest = oldest.filter(|_| conflicted > 0).map(|(holder, commit)| {
            let (store, oldest) = (
                self.store.clone(),
                Selection::Commit(project.clone(), commit.id),
            );
            conflict_bar(
                match conflicted {
                    1 => "A commit has conflicts".to_owned(),
                    n => format!("{n} commits have conflicts"),
                },
                match conflicted {
                    _ if holder.ref_name != ref_name => format!("Resolve from {}…", holder.name),
                    1 => "Resolve…".to_owned(),
                    _ => "Resolve the oldest…".to_owned(),
                },
                theme,
                move |_, window, cx| {
                    store.update(cx, |store, cx| store.select(Some(oldest.clone()), cx));
                    window.dispatch_action(Box::new(crate::resolve::ResolveConflicts), cx)
                },
            )
        });
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
                            .child(format!(
                                "{} · {push}",
                                plural(branch.commits.len(), "commit")
                            )),
                    )
                    .children(resolve_oldest),
            )
            .when_some(branch.review.as_ref(), |el, review| {
                let feedback = (self.feedback.as_ref())
                    .filter(|(p, number, _)| p == project && *number == review.number)
                    .map(|(.., feedback)| feedback);
                el.child(render_review(
                    review,
                    feedback.map(|f| &f.verdicts),
                    theme,
                    cx,
                ))
                .children(feedback.and_then(|feedback| {
                    self.render_threads(project, ref_name, &feedback.threads, theme, cx)
                }))
            })
            .when(!branch.incoming.is_empty(), |el| {
                let stacked =
                    (store.workspace(project)).is_some_and(|ws| ws.has_branches_above(ref_name));
                el.child(render_incoming(&branch.incoming, !stacked, theme, cx))
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
        let header = if self.filtering {
            let paths = diff.files.iter().map(|f| f.file.path.as_str());
            self.filter_header(paths, theme, cx)
        } else {
            h_flex()
                .h(px(36.))
                .px(px(16.))
                .gap(px(4.))
                .flex_none()
                .text_size(px(12.))
                .child(
                    div()
                        .flex_1()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{} changed", plural(diff.files.len(), "file"))),
                )
                .child(self.filter_button(cx))
                .child(self.tree_toggle(cx))
        };
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(header)
            .child(rows)
            .into_any_element()
    }
}

/// The commits only the branch's remote has, as an agent or a teammate pushed them, and
/// a way to bring them in.
fn render_incoming(
    titles: &[String],
    can_pull: bool,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
    v_flex()
        .flex_none()
        .px(px(16.))
        .py(px(10.))
        .gap(px(4.))
        .border_b_1()
        .border_color(theme.border_3)
        .text_size(px(12.))
        .child(
            h_flex()
                .gap(px(8.))
                .child(div().flex_1().text_color(theme.warn).child(format!(
                    "{} on the remote, not here",
                    plural(titles.len(), "commit")
                )))
                .when(can_pull, |el| {
                    el.child(
                        button("pull-remote", "Pull")
                            .small()
                            .tooltip("Bring them in, with this branch's own commits on top")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.app
                                    .update(cx, |app, cx| app.pull_remote(false, window, cx))
                                    .ok();
                            })),
                    )
                }),
        )
        .when(!can_pull, |el| {
            el.child(
                div()
                    .text_color(theme.text_3)
                    .child("Pulling into a branch with others stacked on it isn't possible yet."),
            )
        })
        .children(titles.iter().map(|title| {
            h_flex()
                .gap(px(6.))
                .text_color(theme.text_2)
                .child(small_icon(Icon::ArrowDown).text_color(theme.text_3))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .child(match title.as_str() {
                            "" => "No message".to_owned(),
                            title => title.to_owned(),
                        }),
                )
        }))
        .into_any_element()
}

impl Detail {
    /// Ask the forge where the review of `ref_name`'s open pull request stands, for its card.
    /// Once per visit to the branch, rather than on every reload.
    pub(super) fn load_feedback(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        cx: &mut Context<Self>,
    ) {
        use backend::ReviewState as S;
        let store = self.store.read(cx);
        let number = (store.workspace(project))
            .and_then(|ws| ws.find_branch(ref_name)?.review.as_ref())
            .filter(|review| matches!(review.state, S::Open | S::Draft))
            .map(|review| review.number);
        let (Some(number), Some(path)) = (number, store.path(project)) else {
            return;
        };
        let project = project.clone();
        self._feedback_load = cx.spawn(async move |this, cx| {
            let feedback = backend::run(move || backend::reviews::feedback(&path, number)).await;
            // A forge that can't say, or no account for it, leaves the card as it was.
            let Ok(feedback) = feedback else {
                return;
            };
            this.update(cx, |this, cx| {
                this.feedback = Some((project, number, feedback));
                cx.notify();
            })
            .ok();
        });
    }

    /// Mark a conversation on the pull request resolved, then ask again where it stands.
    fn resolve_thread(
        &mut self,
        project: ProjectId,
        ref_name: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let resolved = backend::run(move || backend::reviews::resolve_thread(&path, id)).await;
            this.update(cx, |this, cx| match resolved {
                Ok(()) => {
                    this.store
                        .update(cx, |store, cx| store.snack("Resolved", cx));
                    this.load_feedback(&project, &ref_name, cx)
                }
                Err(err) => this.store.update(cx, |store, cx| {
                    store.error("Couldn't resolve the conversation", format!("{err:#}"), cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    /// The conversations still open on the branch's pull request: where each is and how it
    /// opened. A click shows its line in the diff; its menu opens it on the forge or
    /// resolves it.
    fn render_threads(
        &self,
        project: &ProjectId,
        ref_name: &str,
        threads: &[backend::reviews::Thread],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        /// More than this many are better read on the forge.
        const SHOWN: usize = 8;
        if threads.is_empty() {
            return None;
        }
        let rows = threads.iter().take(SHOWN).enumerate().map(|(ix, thread)| {
            let (name, _) = diff::split_path(&thread.path);
            let place = match thread.line {
                Some(line) => format!("{name}:{line}"),
                None => name.to_owned(),
            };
            let (path, line, url) = (thread.path.clone(), thread.line, thread.url.clone());
            let menu_url = url.clone();
            let (project, ref_name, id) = (project.clone(), ref_name.to_owned(), thread.id.clone());
            list_row(("thread", ix), false, theme)
                .text_size(px(12.))
                .gap(px(6.))
                .child(
                    div()
                        .flex_none()
                        .font_family(theme.mono_font.clone())
                        .text_color(theme.text_3)
                        .child(place),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .child(format!("{}: {}", thread.author, thread.opening)),
                )
                .when(thread.replies > 0, |el| {
                    el.child(div().flex_none().text_color(theme.text_3).child(
                        match thread.replies {
                            1 => "1 reply".to_owned(),
                            n => format!("{n} replies"),
                        },
                    ))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.reveal_line(&path, line, cx) {
                        cx.open_url(&url);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let url = menu_url.clone();
                        let (project, ref_name, id) =
                            (project.clone(), ref_name.clone(), id.clone());
                        let items = vec![
                            MenuItem::new("Open on the forge", move |_, _, cx| cx.open_url(&url)),
                            MenuItem::new("Mark resolved", move |app, _, cx| {
                                let (project, ref_name, id) =
                                    (project.clone(), ref_name.clone(), id.clone());
                                app.detail.update(cx, |detail, cx| {
                                    detail.resolve_thread(project, ref_name, id, cx)
                                })
                            }),
                        ];
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }),
                )
        });
        let more = threads.len().saturating_sub(SHOWN);
        Some(
            v_flex()
                .flex_none()
                .pb(px(8.))
                .border_b_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .px(px(16.))
                        .pt(px(10.))
                        .pb(px(4.))
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text_3)
                        .child(format!("{} open", plural(threads.len(), "conversation"))),
                )
                .children(rows)
                .when(more > 0, |el| {
                    el.child(
                        div()
                            .px(px(16.))
                            .text_size(px(12.))
                            .text_color(theme.text_3)
                            .child(format!("And {more} more on the forge")),
                    )
                })
                .into_any_element(),
        )
    }
}

/// Where a pull request's review stands: who approved, who asked for changes, and whose
/// review it's still waiting on, a line each. Nothing when there's nothing to say.
fn review_line(
    review: &backend::Review,
    verdicts: Option<&backend::reviews::Verdicts>,
    theme: &Theme,
) -> Option<AnyElement> {
    // Asked, and not decided yet as far as the forge has said.
    let waiting: Vec<String> = (review.reviewers.iter())
        .filter(|login| {
            verdicts
                .is_none_or(|v| !v.approved.contains(login) && !v.changes_requested.contains(login))
        })
        .cloned()
        .collect();
    let parts = [
        (
            verdicts.map_or(&[][..], |v| &v.approved),
            "Approved by",
            theme.safe,
        ),
        (
            verdicts.map_or(&[][..], |v| &v.changes_requested),
            "Changes requested by",
            theme.danger,
        ),
        (&waiting[..], "Waiting on", theme.text_3),
    ];
    let parts: Vec<_> = (parts.into_iter())
        .filter(|(logins, ..)| !logins.is_empty())
        .map(|(logins, label, color)| {
            div()
                .text_color(color)
                .child(format!("{label} {}", logins.join(", ")))
        })
        .collect();
    // One to a line: the pane is narrow, and logins run long.
    (!parts.is_empty()).then(|| {
        v_flex()
            .pl(px(24.))
            .text_size(px(12.))
            .children(parts)
            .into_any_element()
    })
}

/// What's conflicted, on a red tint, with a button that starts resolving it.
fn conflict_bar(
    text: String,
    action: String,
    theme: &Theme,
    resolve: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(theme.danger.opacity(0.08))
        .text_size(px(12.))
        .text_color(theme.danger)
        .child(small_icon(Icon::Warning).text_color(theme.danger))
        .child(div().flex_1().child(text))
        .child(
            button("resolve", action)
                .small()
                .style(ButtonStyle::Gray)
                .on_click(resolve),
        )
}

/// A branch's pull request in two lines, as t3code shows one: what it is, then its number,
/// state and checks, with a way to open it.
fn render_review(
    review: &backend::Review,
    verdicts: Option<&backend::reviews::Verdicts>,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
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
                })
                .when(review.auto_merge, |el| {
                    el.child("·").child("merges when checks pass")
                }),
        )
        .children(review_line(review, verdicts, theme))
        .into_any_element()
}

/// Where a file list's files come from.
#[derive(Clone)]
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
