//! The file lists beside the diff: uncommitted files, or the loaded diff's, flat or as a
//! tree, with what right-clicking them offers.

use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, MouseButton, MouseDownEvent, Pixels,
    Point, ScrollStrategy, SharedString, Stateful, Toggled, div, img, prelude::*, px,
};

use super::file_tree::{self, TreeRow};
use super::panes::side_list;
use super::{Detail, FilterFiles, keeps_path};
use crate::app::AppView;
use crate::backend::{self, CommitId, DiffSource, FileChange, FileDiff, HunkKey, Selected};
use crate::overlays::MenuItem;
use crate::state::{Cut, Overlay, ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::file_icons::file_icon;
use crate::ui::{
    Icon, checkbox, dot, faint, h_flex, icon_button, list_row, muted, relative_time, shortcut,
    small_icon, v_flex,
};

/// Where a file list's files come from.
#[derive(Clone)]
enum FileSource {
    Uncommitted(ProjectId),
    /// The loaded diff's files: a commit's, a branch's or a linked worktree's.
    Diff,
}

/// What right-clicking a file or hunk offers, and for which changes.
#[derive(Clone)]
pub(super) enum FileMenu {
    /// Changes as they'd be cut, to move or take out: see [`Detail::changes_menu`].
    Cut(Cut),
    /// A change of the linked worktree with this name. It doesn't move into the workspace,
    /// so it can only be thrown away.
    Linked(ProjectId, Arc<str>, Selected),
}

/// What a file row shows besides the file itself.
#[derive(Default)]
struct FileRow {
    /// Where the file is in the loaded diff: a click scrolls there, and the diff's cursor
    /// there marks the row.
    diff_ix: Option<usize>,
    /// Shown while composing a commit.
    checkbox: Option<Toggled>,
    /// Shown for uncommitted files.
    review: Option<Review>,
    /// What right-clicking the row offers. Changes it can cut, it drags as well.
    menu: Option<FileMenu>,
    /// How deep the file sits in a tree; it then shows only its name.
    depth: Option<usize>,
}

/// How far each level of a file tree steps in.
const STEP: f32 = 12.;

/// The group a file row's hover reaches its checkbox through.
const ROW_GROUP: &str = "file-row";

/// A row of a file list, inset in the panel as GitButler's other file lists are.
fn file_row(id: impl Into<ElementId>, selected: bool, theme: &Theme) -> Stateful<Div> {
    list_row(id, selected, theme)
        .mx(px(12.))
        .pl(px(8.))
        .pr(px(4.))
        .text_size(px(13.))
}

/// A tree row's indent: a step per level, each with a guide that joins the rows above and
/// below into one line down its directory.
fn steps(depth: usize, theme: &Theme) -> Div {
    h_flex().flex_none().h_full().children((0..depth).map(|_| {
        div()
            .w(px(STEP))
            .h_full()
            .flex_none()
            .flex()
            .justify_center()
            .child(div().w(px(1.)).h_full().bg(theme.text_2.opacity(0.2)))
    }))
}

/// The panel's heading: Files, how many, and their lines added and removed.
fn files_heading(count: usize, (added, removed): (u32, u32), theme: &Theme) -> Div {
    h_flex()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::BOLD)
                .child("Files"),
        )
        .child(
            div()
                .px(px(6.))
                .rounded_full()
                .bg(theme.fill_gray.opacity(0.1))
                .text_size(px(11.))
                .text_color(theme.text_2)
                .child(count.to_string()),
        )
        .child(diff::stats(added, removed, theme))
}

/// A file list's header row, as GitButler's other file lists lay it out.
fn header_row() -> Div {
    h_flex()
        .flex_none()
        .pt(px(12.))
        .pb(px(10.))
        .pl(px(16.))
        .pr(px(12.))
        .gap(px(8.))
        .text_size(px(12.))
}

impl Detail {
    /// The uncommitted `files` of `project`, under how many there are, their line counts
    /// and how many still want a look.
    pub(super) fn render_uncommitted_files(
        &self,
        project: &ProjectId,
        files: &[FileChange],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let rows = self.file_list(
            "uncommitted-files",
            FileSource::Uncommitted(project.clone()),
            cx,
        );
        let diffs = self.worktree_diff().map_or(&[][..], |diff| &diff.files[..]);
        let header = self.list_header(
            files.iter().map(|f| f.path.as_str()),
            diffs,
            self.store.read(cx).unreviewed(project, files),
            theme,
            cx,
        );
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(header)
            .child(v_flex().flex_1().min_h(px(0.)).child(rows))
            .into_any_element()
    }

    /// The files of the loaded diff, each a jump target, and draggable when the diff has an
    /// owner ([`Self::diff_owner`]).
    pub(super) fn render_diff_file_list(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let Some(diff) = self.shown_diff() else {
            let note = if self.loading.is_some() {
                "Loading…"
            } else {
                ""
            };
            return muted(note, theme)
                .px(px(16.))
                .py(px(10.))
                .into_any_element();
        };
        let rows = self.file_list("diff-files", FileSource::Diff, cx);
        let paths = diff.files.iter().map(|f| f.file.path.as_str());
        let header = self.list_header(paths, &diff.files, 0, theme, cx);
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(header)
            .child(rows)
            .into_any_element()
    }

    /// A file list, flat or as a tree as the user chose, drawn only as far as it's on
    /// screen: an agent can leave thousands of files.
    fn file_list(&self, id: &'static str, source: FileSource, cx: &Context<Self>) -> Div {
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
            FileSource::Diff => self
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
        let scroll = match &source {
            FileSource::Uncommitted(_) => &self.uncommitted_files,
            FileSource::Diff => &self.diff_files,
        };
        // The list follows the cursor to another file, as j and ↓ move it, or to the folded
        // directory the file is in.
        let cursor = self
            .cursor_path()
            .filter(|path| paths.contains(&path.as_str()));
        let cursor_row = cursor.as_deref().and_then(|cursor| {
            rows.iter().position(|row| match row {
                TreeRow::File { pos, .. } => paths[*pos] == cursor,
                TreeRow::Dir {
                    path,
                    collapsed: true,
                    ..
                } => (cursor.strip_prefix(path.as_str())).is_some_and(|rest| rest.starts_with('/')),
                TreeRow::Dir { .. } => false,
            })
        });
        if *self.listed_cursor.borrow() != cursor
            && let Some(row) = cursor_row
        {
            scroll.scroll_to_item(row, ScrollStrategy::Nearest);
            *self.listed_cursor.borrow_mut() = cursor;
        }
        side_list(
            id,
            rows.len(),
            scroll,
            "files-scrollbar",
            cx,
            move |this, ix, theme, window, cx| match rows.get(ix)? {
                dir @ TreeRow::Dir { .. } => {
                    this.render_dir_row(dir, cursor_row == Some(ix), &source, theme, cx)
                }
                TreeRow::File { pos, depth } => {
                    let depth = tree.then_some(*depth);
                    // The current file is solid while the files and the diff hold the focus,
                    // as the navigator's selection is while it does.
                    let solid = this.focus.is_focused(window);
                    this.render_source_file(&source, index[*pos], depth, solid, theme, cx)
                }
            },
        )
    }

    /// The `pos`th file of `source`, with what its list shows beside it.
    fn render_source_file(
        &self,
        source: &FileSource,
        pos: usize,
        depth: Option<usize>,
        solid: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        match source {
            FileSource::Uncommitted(project) => {
                let store = self.store.read(cx);
                let file = store.workspace(project)?.uncommitted.get(pos)?;
                let whole = Selected {
                    file: file.clone(),
                    hunks: None,
                };
                let extras = FileRow {
                    diff_ix: (self.worktree_diff())
                        .and_then(|diff| diff.files.iter().position(|f| f.file.path == file.path)),
                    checkbox: self.composing.then(|| self.checks.file_check(&file.path)),
                    review: Some(store.review(project, file)),
                    menu: Some(FileMenu::Cut(Cut::Files {
                        project: project.clone(),
                        from: None,
                        files: vec![whole].into(),
                    })),
                    depth,
                };
                Some(self.render_file_row(pos, file, extras, solid, theme, cx))
            }
            FileSource::Diff => {
                let diff = self.diff.as_ref()?;
                let file = diff.files.get(pos)?;
                // A linked worktree's files are reviewed too.
                let review = (diff.key.review_scope())
                    .map(|scope| self.store.read(cx).review(&scope, &file.file));
                let extras = FileRow {
                    diff_ix: Some(pos),
                    review,
                    menu: self.diff_file_menu(pos, None, cx),
                    depth,
                    ..Default::default()
                };
                Some(self.render_file_row(pos, &file.file, extras, solid, theme, cx))
            }
        }
    }

    /// The project the loaded diff's changes can be moved or taken out of, with the commit
    /// they're in, or `None` when they're uncommitted. A branch's diff spans commits, a
    /// linked worktree's changes and commits change only there, and a commit of the target's
    /// history, on the project page, isn't the workspace's to change: those have no owner.
    pub(super) fn diff_owner(&self, cx: &App) -> Option<(ProjectId, Option<CommitId>)> {
        let key = &self.diff.as_ref()?.key;
        let from = match key.source {
            DiffSource::Worktree => None,
            DiffSource::Commit(id)
                if (self.store.read(cx).workspace(&key.project))
                    .is_some_and(|ws| ws.own_commits().any(|commit| commit.id == id)) =>
            {
                Some(id)
            }
            _ => return None,
        };
        Some((key.project.clone(), from))
    }

    /// The uncommitted file under the cursor, to cut, as the diff's other keys act on it.
    pub fn cursor_cut(&self, cx: &App) -> Option<Cut> {
        // Right after selecting Uncommitted changes, the diff can still be the last one, a
        // commit's or another project's.
        let project = self.selected_project(cx);
        let uncommitted = (self.diff.as_ref()).is_some_and(|d| {
            d.key.source == DiffSource::Worktree && Some(&d.key.project) == project.as_ref()
        });
        (uncommitted && self.shown_files().contains(&self.cursor.file))
            .then(|| self.diff_cut(self.cursor.file, None, cx))
            .flatten()
    }

    /// The loaded diff's file at `file`, or `hunks` of it, to drag or cut.
    pub(super) fn diff_cut(
        &self,
        file: usize,
        hunks: Option<Vec<HunkKey>>,
        cx: &App,
    ) -> Option<Cut> {
        let (project, from) = self.diff_owner(cx)?;
        let file = self.diff.as_ref()?.files.get(file)?.file.clone();
        Some(Cut::Files {
            project,
            from,
            files: vec![Selected { file, hunks }].into(),
        })
    }

    /// What right-clicking the loaded diff's file at `file`, or `hunks` of it, offers. A
    /// linked worktree's changes don't move into the workspace, so they can only be thrown
    /// away.
    pub(super) fn diff_file_menu(
        &self,
        file: usize,
        hunks: Option<Vec<HunkKey>>,
        cx: &App,
    ) -> Option<FileMenu> {
        let diff = self.diff.as_ref()?;
        match diff.key.linked() {
            Some(worktree) => {
                let file = diff.files.get(file)?.file.clone();
                let change = Selected { file, hunks };
                Some(FileMenu::Linked(diff.key.project.clone(), worktree, change))
            }
            None => self.diff_cut(file, hunks, cx).map(FileMenu::Cut),
        }
    }

    /// The files under the folder `dir` in `source`, to act on together.
    fn folder_payload(&self, source: &FileSource, dir: &str, cx: &App) -> Option<Cut> {
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
            FileSource::Diff => {
                let (project, from) = self.diff_owner(cx)?;
                let files = (self.diff.as_ref()?.files.iter())
                    .map(|f| &f.file)
                    .filter(|f| under(&f.path))
                    .cloned()
                    .collect();
                (project, from, files)
            }
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
        holds_cursor: bool,
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
        let row = file_row(
            ElementId::Name(format!("dir-{path}").into()),
            holds_cursor,
            theme,
        )
        .when(depth > 0, |el| el.child(steps(depth, theme)))
        .child(
            div().w(px(STEP)).flex_none().flex().justify_center().child(
                small_icon(if collapsed {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                })
                .text_color(theme.text_3),
            ),
        )
        // In colour, as ui-react's FolderIcon draws it.
        .child(
            img(Icon::FolderFill.path())
                .flex_none()
                .w(px(16.))
                .h(px(13.)),
        )
        .child(div().flex_1().min_w(px(0.)).truncate().child(name))
        .when(collapsed, |el| el.child(faint(files.to_string(), theme)))
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

    /// A file list's header: how many `paths` it has, the lines `diffs` add and remove, how
    /// many files are still `to_review`, and ways to filter it or show it as a tree. While
    /// the filter is open: the filter, how many of `paths` it keeps, and a way to close it.
    fn list_header<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        diffs: &[FileDiff],
        to_review: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        if self.filtering {
            let terms = self.filter_terms(cx);
            let (kept, total) = paths.fold((0, 0), |(kept, total), path| {
                (kept + usize::from(keeps_path(&terms, path)), total + 1)
            });
            return header_row()
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
                );
        }
        let stats =
            (diffs.iter().map(FileDiff::stats)).fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
        let tree = self.store.read(cx).file_tree;
        let store = self.store.clone();
        header_row()
            .child(files_heading(paths.count(), stats, theme))
            .when(to_review > 0, |el| {
                el.child(
                    div()
                        .text_color(theme.warn)
                        .child(format!("{to_review} to review")),
                )
            })
            .child(div().flex_1())
            .child(
                icon_button("filter-files", Icon::Search)
                    .small()
                    .tooltip_with_key("Filter files", shortcut(&FilterFiles, cx))
                    .on_click(cx.listener(|this, _, window, cx| this.open_filter(window, cx))),
            )
            .child(
                icon_button(
                    "file-tree",
                    if tree { Icon::List } else { Icon::FolderTree },
                )
                .small()
                .tooltip(if tree { "Show as list" } else { "Show as tree" })
                .on_click(move |_, _, cx| {
                    store.update(cx, |store, cx| store.set_file_tree(!tree, cx));
                }),
            )
    }

    fn render_file_row(
        &self,
        ix: usize,
        file: &FileChange,
        extras: FileRow,
        solid: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (name, dir) = diff::split_path(&file.path);
        let path = file.path.clone();
        let diff_ix = extras.diff_ix;
        let stats = (diff_ix.and_then(|ix| self.diff.as_ref()?.files.get(ix))).map(FileDiff::stats);
        let current = diff_ix == Some(self.cursor.file);
        let solid = current && solid;
        // On the solid ground, as ui-react's FileList inverts a selected row's contents.
        let muted = match solid {
            true => theme.text_inverted.opacity(0.7),
            false => theme.text_2,
        };
        let reviewed = extras.review == Some(Review::Reviewed);
        // In a tree a file sits under its directory, which names its path.
        let dir = if extras.depth.is_some() { "" } else { dir };
        // A reviewed file is done with, so what the row says about it recedes.
        let fade = |el: Div| el.when(reviewed, |el| el.opacity(0.5));
        // While composing, the checkbox takes the icon's place once it's checked, or while
        // the pointer is on the row.
        let lead = match extras.checkbox {
            None => (file_icon(&file.path, cx))
                .when(reviewed, |el| el.opacity(0.5))
                .into_any_element(),
            Some(check) => {
                let on = check != Toggled::False;
                let path = path.clone();
                let check = checkbox(("check", ix), check, theme)
                    // Checking a file doesn't scroll to it.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.toggle_checked(&path, cx)
                    }));
                div()
                    .relative()
                    .flex_none()
                    .size(px(14.))
                    .child(file_icon(&file.path, cx).absolute().map(|el| match on {
                        true => el.opacity(0.),
                        false => el.group_hover(ROW_GROUP, |s| s.opacity(0.)),
                    }))
                    .child(
                        div()
                            .absolute()
                            .map(|el| match on {
                                true => el,
                                false => el.opacity(0.).group_hover(ROW_GROUP, |s| s.opacity(1.)),
                            })
                            .child(check),
                    )
                    .into_any_element()
            }
        };
        // The ⋯ takes the mark's place while the pointer is on the row or the row is current.
        // A plain glyph rather than a button: a list draws it on every row.
        let more = extras.menu.clone().map(|menu| {
            div()
                .id(("file-menu", ix))
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .cursor_pointer()
                .hover(|el| match solid {
                    true => el.bg(theme.text_inverted.opacity(0.1)),
                    false => el.bg(theme.hover()),
                })
                .when(!current, |el| {
                    el.invisible().group_hover(ROW_GROUP, |s| s.visible())
                })
                .child(small_icon(Icon::Kebab).text_color(muted))
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.file_menu(menu.clone(), None, event.position(), cx)
                }))
        });
        let has_menu = more.is_some();
        let mark = div()
            .when(has_menu && current, |el| el.invisible())
            .when(has_menu && !current, |el| {
                el.group_hover(ROW_GROUP, |s| s.invisible())
            })
            .map(|el| match (file.conflicted, reviewed) {
                (true, _) => el.child(small_icon(Icon::Warning).text_color(theme.danger)),
                (false, true) => el.child(small_icon(Icon::Tick).text_color(theme.pop)),
                (false, false) => el
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(diff::change_color(file.kind, theme))
                    .child(file.kind.letter()),
            });
        let end = div()
            .relative()
            .flex_none()
            .w(px(22.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(mark)
            .children(more);
        let mut row = file_row(("file", ix), current, theme)
            .when(solid, |el| {
                el.bg(theme.fill_gray).text_color(theme.text_inverted)
            })
            .aria_label(file.path.clone())
            .aria_description(file.kind.word())
            .group(ROW_GROUP)
            .children(
                extras
                    .depth
                    .filter(|&d| d > 0)
                    .map(|depth| steps(depth, theme)),
            )
            .child(lead)
            // The name whole where it fits, then its folder dimmed and cut in the middle, as the
            // folder's end tells two files of one name apart.
            .child(fade(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .max_w_full()
                            .truncate()
                            .child(name.to_owned()),
                    )
                    .when(!dir.is_empty(), |el| {
                        el.child(
                            div()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_middle()
                                .text_color(muted)
                                .child(dir.to_owned()),
                        )
                    }),
            ))
            .when_some(stats, |el, (added, removed)| {
                el.child(fade(diff::stats(added, removed, theme)))
            })
            // When it last changed, faint, for watching what an agent touches.
            .children(
                (!file.conflicted)
                    .then_some(file.modified)
                    .flatten()
                    .map(|ms| {
                        faint(relative_time(ms / 1000), theme)
                            .text_size(px(11.))
                            .when(solid, |el| el.text_color(muted))
                    }),
            )
            .when(extras.review == Some(Review::Changed), |el| {
                el.child(dot(theme.warn))
            })
            .child(end)
            .when_some(diff_ix, |el, diff_ix| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.scroll_to_file(diff_ix, cx)
                    }),
                )
            });
        // A file partly checked drags as its checked hunks and lines. Not at all while
        // they're unknown, rather than as all of it.
        let dragged = match &extras.menu {
            Some(FileMenu::Cut(Cut::Files { project, from, .. }))
                if extras.checkbox == Some(Toggled::Mixed) =>
            {
                (self.selected_change(file)).map(|change| Cut::Files {
                    project: project.clone(),
                    from: *from,
                    files: vec![change].into(),
                })
            }
            Some(FileMenu::Cut(cut)) => Some(cut.clone()),
            _ => None,
        };
        if let Some(dragged) = dragged {
            let label: SharedString = file.path.clone().into();
            row = row.on_drag(
                dragged,
                crate::transfer::drag_preview(self.store.clone(), label),
            );
        }
        if let Some(menu) = extras.menu {
            row = row.on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.file_menu(menu.clone(), None, event.position, cx)
                }),
            );
        }
        row.into_any_element()
    }

    /// Open `menu` at `position`, with `line` where an editor should open the file.
    pub(super) fn file_menu(
        &mut self,
        menu: FileMenu,
        line: Option<u32>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        match menu {
            FileMenu::Cut(payload) => self.changes_menu(payload, Some(line), position, cx),
            FileMenu::Linked(project, worktree, change) => {
                let items = vec![
                    MenuItem::new("Discard changes…", move |this, window, cx| {
                        let (worktree, changes) = (Some(worktree.clone()), vec![change.clone()]);
                        this.discard(project.clone(), worktree, changes, window, cx)
                    })
                    .danger(),
                ];
                self.app
                    .update(cx, |app, cx| app.open_menu(position, items, cx))
                    .ok();
            }
        }
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
                    let editing = (self.store.read(cx).workspace(&project))
                        .is_some_and(|ws| ws.editing.is_some());
                    items.insert(
                        0,
                        MenuItem::new("Mark resolved", move |this, _, cx| {
                            let files = conflicted.clone();
                            this.store.update(cx, |store, cx| {
                                store.run(
                                    &project,
                                    "Mark resolved",
                                    Overlay {
                                        files_only: true,
                                        ..Overlay::default()
                                    },
                                    move |path, _| backend::mark_resolved(path, files),
                                    // Undo can't bring back a commit checked out for editing.
                                    move |store, (), cx| match editing {
                                        true => store.snack("Marked resolved", cx),
                                        false => store.snack_undoable("Marked resolved", cx),
                                    },
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
                        this.discard(project.clone(), None, files.to_vec(), window, cx)
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
            let label = crate::open_in::editor_label(self.store.read(cx));
            items.extend([
                MenuItem::new(label, move |this, _, cx| {
                    let file = Some(open_file.clone());
                    this.open_in_editor(&open_project, None, file, line, position, cx)
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
}
