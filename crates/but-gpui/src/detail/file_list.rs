//! The file lists beside the diff: uncommitted files, or the loaded diff's, flat or as a
//! tree, with what right-clicking them offers.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, MouseButton, MouseDownEvent,
    Pixels, Point, SharedString, Stateful, Toggled, Window, actions, div, prelude::*,
};

use super::blame::Blamed;
use super::file_tree::{self, TreeRow};
use super::panes::{rows_under, side_list};
use super::{
    CheckAll, Copied, Detail, FilterFiles, LoadedDiff, Messages, PageFold, UncheckAll, keeps_path,
    merged, ranked,
};
actions!(changes, [MarkResolved, UncommitChanges, SplitIntoNewCommit]);

/// Where [`Detail::take_from_commit`] takes the changes.
#[derive(Clone, Copy)]
enum Taken {
    Uncommit,
    Split,
}

/// What of a file Discard takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscardPart {
    File,
    Hunk,
}

/// What of the file with the keys Discard takes, with nothing picked: all of it from its row in
/// the list, `in_list`, whatever the diff's cursor says. In the diff, the hunk the cursor is on,
/// unless the file is `conflicted`, whose hunks don't go alone; all of it only from its header.
/// Either only while it's `in_sight`, not scrolled away from what's being looked at. The box
/// above a hunk stands for lines that didn't change, so nothing goes from there.
fn discard_part(in_list: bool, at: Stop, in_sight: bool, conflicted: bool) -> Option<DiscardPart> {
    match (in_list, at) {
        (true, _) => Some(DiscardPart::File),
        (false, Stop::File) => in_sight.then_some(DiscardPart::File),
        (false, Stop::Hunk(_)) => (in_sight && !conflicted).then_some(DiscardPart::Hunk),
        (false, Stop::Gap(_) | Stop::Message) => None,
    }
}

/// What `files` are called, as picked lines when `lines`: lines, a hunk, a file or files.
fn change_kind(lines: bool, files: &[Selected]) -> &'static str {
    match (lines, files.iter().any(|f| f.hunks.is_some()), files.len()) {
        (true, ..) => "lines",
        (false, true, _) => "hunk",
        (false, false, 1) => "file",
        (false, false, _) => "files",
    }
}
use crate::backend::{self, Checkout, DiffSource, FileChange, FileDiff, HunkKey, Selected};
use crate::detail::Stop;
use crate::navigator::upstream_label;
use crate::overlays::{MenuItem, has_keys};
use crate::state::{FileSort, FileView, Overlay, ProjectId, Rereads, Selection, Store};
use crate::theme::Theme;
use crate::transfer::{Changes, Chip, DragPreview, Dragged, Moving};
use crate::ui::diff;
use crate::ui::file_icons::{file_icon, folder_icon};
use crate::ui::fold;
use crate::ui::illustration::Illustration;
use crate::ui::{
    Icon, ROW_HEIGHT, Tooltip, blink_once, checkbox, commit_title, count_badge, dp, empty_state,
    faint, fixed, h_flex, icon_button, list_row, matched, muted, plural, popover, shortcut,
    side_row, small_icon, v_flex,
};

/// Where a file list's files come from.
#[derive(Clone)]
enum FileSource {
    /// A checkout's uncommitted files.
    Uncommitted(ProjectId, Checkout),
    /// The loaded diff's files: a commit's or a branch's.
    Diff,
}

/// What a menu's path items are about.
pub(super) enum At {
    /// A file by its path, which an editor opens at the line when there's one.
    File(String, Option<u32>),
    /// A folder of a file list, by its path.
    Folder(String),
}

/// What a file row shows besides the file itself.
#[derive(Default)]
struct FileRow {
    /// Where the file is in the loaded diff: a click scrolls there, and the diff's cursor
    /// there marks the row.
    diff_ix: Option<usize>,
    /// Shown while composing a commit.
    checkbox: Option<Toggled>,
    /// The changes right-clicking or dragging the row takes: see [`Detail::changes_menu`].
    menu: Option<(ProjectId, Changes)>,
    /// How deep the file sits in a tree; it then shows only its name.
    depth: Option<usize>,
    /// The tree has folders: the row keeps a step empty where theirs have the chevron, so a
    /// file sits a step in from its folder and lines up with the folders beside it.
    folders: bool,
    /// Picked with other files, which its menu and a drag take along.
    picked: bool,
    /// The rows above and below are picked with it: see [`crate::ui::joined`].
    joined: (bool, bool),
    /// The byte ranges of its path the filter found, to underline.
    found: Vec<Range<usize>>,
    /// The characters the list's widest added and removed counts take, to line its own up
    /// with the other rows': see [`diff::column_stats`].
    columns: (usize, usize),
}

/// The room a row keeps for its file's change letter, which a folder's keeps empty, so their
/// counts end in one place.
const MARK_WIDTH: f32 = 22.;

/// How far each level of a file tree steps in.
const STEP: f32 = 12.;

/// The group a row's hover reaches a file's checkbox or a folder's line counts through.
const ROW_GROUP: &str = "file-row";

/// A row of a file list, inset in the panel as GitButler's other file lists are, its
/// selection `solid` while the list holds the focus.
fn file_row(id: impl Into<ElementId>, selected: bool, solid: bool, theme: &Theme) -> Stateful<Div> {
    side_row(id, selected, solid, theme)
        .mx(dp(12.))
        .pl(dp(8.))
        .pr(dp(4.))
        .text_size(dp(13.))
}

/// A tree row's indent, as but-ui-react's FileListItem draws it: a step per level, each with a
/// guide that joins the rows above and below into one line down its directory, and a
/// directory's `chevron` in a step of its own, which a file's row keeps empty. A file's icon so
/// sits a step in from its directory's, under its name.
fn steps(depth: usize, chevron: Option<AnyElement>, theme: &Theme) -> Div {
    let step = || {
        div()
            .w(dp(STEP))
            .h_full()
            .flex_none()
            .flex()
            .justify_center()
    };
    h_flex()
        .flex_none()
        .h_full()
        .gap(dp(4.))
        .children(
            (0..depth)
                .map(|_| step().child(div().w(fixed(1.)).h_full().bg(theme.text_2.opacity(0.2)))),
        )
        .children(chevron.map(|chevron| step().items_center().child(chevron)))
}

/// Those of `found`, byte ranges of a path, in its part `part`, as ranges of that part.
fn found_in(found: &[Range<usize>], part: Range<usize>) -> Vec<Range<usize>> {
    (found.iter())
        .filter(|range| range.start < part.end && range.end > part.start)
        .map(|range| range.start.max(part.start) - part.start..range.end.min(part.end) - part.start)
        .collect()
}

/// What each of `rows` underlines of what the filter `found` in `paths`, by position: a file
/// row what was found in its path, and a folder row what was found in its name for any file
/// under it, as a tree shows only a file's name.
fn underlined(
    rows: &[TreeRow],
    paths: &[&str],
    mut found: Vec<Vec<Range<usize>>>,
) -> Vec<Vec<Range<usize>>> {
    let dirs: HashMap<&str, usize> = (rows.iter().enumerate())
        .filter_map(|(ix, row)| match row {
            TreeRow::Dir { path, .. } => Some((path.as_str(), ix)),
            TreeRow::File { .. } => None,
        })
        .collect();
    let mut underlined = vec![Vec::new(); rows.len()];
    // Each folder a file is under, as its path has them.
    for (path, found) in paths.iter().zip(&found).filter(|_| !dirs.is_empty()) {
        for (end, _) in path.match_indices('/') {
            let Some(&ix) = path.get(..end).and_then(|dir| dirs.get(dir)) else {
                continue;
            };
            if let (Some(TreeRow::Dir { name, .. }), Some(row)) =
                (rows.get(ix), underlined.get_mut(ix))
            {
                row.extend(found_in(found, end - name.len()..end));
            }
        }
    }
    for (row, underlined) in rows.iter().zip(&mut underlined) {
        *underlined = match row {
            TreeRow::File { pos, .. } => (found.get_mut(*pos)).map(std::mem::take),
            TreeRow::Dir { .. } => Some(merged(std::mem::take(underlined))),
        }
        .unwrap_or_default();
    }
    underlined
}

/// The panel's heading: Files, how many, and their lines added and removed.
fn files_heading(count: usize, (added, removed): (u32, u32), theme: &Theme) -> Div {
    h_flex()
        .gap(dp(8.))
        .child(
            div()
                .text_size(dp(14.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("Files"),
        )
        .child(count_badge(count, theme))
        .child(diff::stats(added, removed, false, theme))
}

/// A file list's header row, as GitButler's other file lists lay it out.
fn header_row() -> Div {
    h_flex()
        .flex_none()
        .pt(dp(12.))
        .pb(dp(10.))
        .pl(dp(16.))
        .pr(dp(12.))
        .gap(dp(8.))
        .text_size(dp(12.))
}

impl Detail {
    /// The uncommitted `files` of `project`'s `checkout`, under how many there are and their
    /// line counts.
    pub(super) fn render_uncommitted_files(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        files: &[FileChange],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let source = FileSource::Uncommitted(project.clone(), checkout.clone());
        let (rows, _) = self.file_list(source, cx);
        // Counted as listed: without the files a pending operation is taking.
        let store = self.store.read(cx);
        let shown = |path: &str| !store.hides_file(project, checkout, path);
        let diffs = self.worktree_diff().map_or(&[][..], |diff| &diff.files[..]);
        let header = self.list_header(
            files
                .iter()
                .map(|f| f.path.as_str())
                .filter(|path| shown(path)),
            diffs.iter().filter(|diff| shown(&diff.file.path)),
            self.check_all_box(theme, cx),
            theme,
            cx,
        );
        // A clean checkout is good news, as but.dev greets it, here as its overview is beside.
        let clean = !self.filtering
            && !files.iter().any(|file| shown(&file.path))
            && self.opened_file(project, checkout, files).is_none();
        let rows = match clean {
            true => empty_state(
                Some(Illustration::Waving),
                "No uncommitted changes",
                "Edits show up here once saved",
                theme,
                cx,
            ),
            false => rows.into_any_element(),
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .child(header)
            .children(self.find_in_all_files(theme, cx))
            .child(v_flex().flex_1().min_h_0().child(rows))
            .into_any_element()
    }

    /// Above the files the filter keeps, a way to look for what's typed among all the files
    /// the checkout tracks, to edit one with no changes.
    fn find_in_all_files(&self, theme: &Theme, cx: &Context<Self>) -> Option<Stateful<Div>> {
        let query = self.file_filter.read(cx).text().trim().to_owned();
        if !self.filtering || query.is_empty() || self.openable_checkout().is_none() {
            return None;
        }
        let label = format!("Find \"{query}\" in all files");
        let row = file_row("find-in-all-files", false, false, theme)
            .flex_none()
            .mb(dp(2.))
            .gap(dp(8.))
            .text_color(theme.text_2)
            .aria_label(label.clone())
            .child(small_icon(Icon::Search).text_color(theme.text_3))
            .child(div().min_w_0().truncate().child(label))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let query = query.clone();
                (this.app).update_later(window, cx, move |app, window, cx| {
                    app.open_file_picker(query, window, cx)
                });
            }));
        Some(row)
    }

    /// The files of the loaded diff, each a jump target, and draggable when the diff has an
    /// owner ([`Self::diff_owner`]).
    pub(super) fn render_diff_file_list(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(diff) = self.shown_diff() else {
            let note = if self.loading.is_some() {
                "Loading…"
            } else {
                ""
            };
            return muted(note, theme)
                .px(dp(16.))
                .py(dp(10.))
                .into_any_element();
        };
        let (rows, _) = self.file_list(FileSource::Diff, cx);
        // Counted as listed: without the files a pending operation is taking out.
        let leaving = diff.key.leaving(self.store.read(cx));
        let paths = (diff.files.iter())
            .map(|f| f.file.path.as_str())
            .filter(|path| !leaving(path));
        let diffs = diff.files.iter().filter(|f| !leaving(&f.file.path));
        let header = self.list_header(paths, diffs, None, theme, cx);
        v_flex()
            .flex_1()
            .min_h_0()
            .child(header)
            .children(self.render_message_entry(theme, window, cx))
            .child(rows)
            .into_any_element()
    }

    /// The messages as the list's first entry, where the diff lists them: a commit's title, a
    /// branch's name, or where a remote's new commits are, which shows them in the diff. It
    /// stays put above the files as they scroll.
    fn render_message_entry(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        // A commit by its title, a branch or a remote's new commits by their name and count.
        let (glyph, title, count) = match self.loaded_messages(cx)? {
            Messages::Commit(project, id) => {
                let (_, commit) = store.find_commit(&project, id)?;
                let title = commit_title(store.message_of(&project, commit).0);
                (Icon::Commit, title, None)
            }
            Messages::Branch(project, ref_name) => {
                let branch = store.workspace(&project)?.find_branch(ref_name.as_ref())?;
                let name = SharedString::from(store.name_of(&project, branch).to_owned());
                (Icon::Branch, name, Some(branch.commits.len()))
            }
            Messages::Upstream(project, ref_name) => {
                let workspace = store.workspace(&project);
                let (label, count, _) = upstream_label(workspace, &ref_name);
                (Icon::ArrowDown, label.into(), Some(count))
            }
            Messages::TargetCommit(project) => {
                let commit = self.target_message_commit(&project, cx)?;
                (Icon::Commit, super::upstream::label(commit), None)
            }
            Messages::OtherBranch(project) => {
                let (glyph, name) = self.other_branch_entry(&project)?;
                (glyph, name.into(), None)
            }
            Messages::UpstreamCommit(project, ref_name, id) => {
                let ws = store.workspace(&project)?;
                let title = match ws.upstream(&ref_name).iter().find(|c| c.id == id) {
                    Some(c) => &c.title,
                    None => &store.merged_upstream(&project, &ref_name, id)?.1.title,
                };
                (Icon::ArrowDown, commit_title(title), None)
            }
        };
        let current =
            self.current_entry() == super::Entry::Message && self.cursor_folder(cx).is_none();
        let solid = current && has_keys(&self.files_focus, window, cx);
        let muted = match solid {
            true => theme.muted_on_solid(),
            false => theme.text_2,
        };
        let label = match count {
            None => "Commit message",
            Some(_) => "Commit messages",
        };
        let row = file_row("message-entry", current, solid, theme)
            .flex_none()
            .gap(dp(8.))
            .aria_label(label)
            .child(small_icon(glyph).text_color(muted))
            .child(div().flex_1().min_w_0().truncate().child(title))
            .children(count.map(|n| {
                (div().flex_none().text_size(dp(12.)).text_color(muted)).child(n.to_string())
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.clear_file_picks(cx);
                    this.scroll_to_message(true, cx)
                }),
            );
        Some(row.into_any_element())
    }

    /// The files beside the diff, in the popover the files button opens on hover while
    /// they aren't beside it: the uncommitted ones of a checkout's page, else the diff's.
    /// A file picked in it scrolls the diff to it and leaves it open, to pick the next.
    pub(super) fn render_files_popover(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let source = (self.selected_checkout()).map_or(FileSource::Diff, |(project, worktree)| {
            FileSource::Uncommitted(project, worktree)
        });
        let (rows, count) = self.file_list(source, cx);
        popover(theme)
            .w(dp(340.))
            // The rows, the padding and the border.
            .h(dp((count as f32 * ROW_HEIGHT).min(400.) + 10.))
            .py(dp(4.))
            .child(rows)
            .into_any_element()
    }

    /// A file list, flat or as a tree as the user chose, drawn only as far as it's on
    /// screen: an agent can leave thousands of files. Also how many rows it has.
    fn file_list(&self, source: FileSource, cx: &Context<Self>) -> (Div, usize) {
        let store = self.store.read(cx);
        let tree = store.prefs.file_view == FileView::Tree;
        let terms = self.filter_terms(cx);
        // Each shown file's position in its source, as files a pending operation takes away
        // aren't shown.
        let shown: Vec<(usize, &str)> = match &source {
            FileSource::Uncommitted(project, worktree) => {
                let files = (store.workspace(project))
                    .and_then(|ws| ws.uncommitted_in(worktree))
                    .unwrap_or_default();
                let mut kept: Vec<(usize, &FileChange)> = (files.iter().enumerate())
                    .filter(|(_, f)| !store.hides_file(project, worktree, &f.path))
                    .collect();
                // A file opened to edit with no changes, just past them.
                kept.extend(
                    (self.opened_file(project, worktree, files))
                        .map(|opened| (files.len(), opened)),
                );
                // In the order picked, as the diff beside it has them.
                let sort = store.prefs.file_sort;
                kept.sort_by(|(_, a), (_, b)| match tree {
                    true => file_tree::tree_order(sort, a, b),
                    false => file_tree::file_order(sort, a, b),
                });
                (kept.into_iter())
                    .map(|(pos, f)| (pos, f.path.as_str()))
                    .collect()
            }
            FileSource::Diff => self
                .diff
                .as_ref()
                .map(|diff| {
                    let leaving = diff.key.leaving(store);
                    diff.files
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| !leaving(&f.file.path))
                        .map(|(pos, f)| (pos, f.file.path.as_str()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        // While the filter's open, only what it keeps, the best matches first.
        let listed = ranked(&terms, shown, |(_, path)| *path);
        let (index, paths): (Vec<usize>, Vec<&str>) = (listed.iter())
            .map(|((pos, path), _)| (*pos, *path))
            .unzip();
        let found = listed.into_iter().map(|(_, found)| found).collect();
        let rows: Vec<TreeRow> = if tree {
            // A folder closing stays open till it's shut.
            let closing = |dir: &String| self.page_folds.closing(&PageFold::Dir(dir.clone()));
            let none = HashSet::new();
            let collapsed = (self.page.files()).map_or(&none, |files| &files.collapsed_dirs);
            let shut: Cow<HashSet<String>> = match self.page_folds.any() {
                true => Cow::Owned(
                    (collapsed.iter())
                        .filter(|dir| !closing(dir))
                        .cloned()
                        .collect(),
                ),
                false => Cow::Borrowed(collapsed),
            };
            file_tree::tree_rows(&paths, &shut)
        } else {
            (0..paths.len())
                .map(|pos| TreeRow::File { pos, depth: 0 })
                .collect()
        };
        let folders = tree && rows.iter().any(|row| matches!(row, TreeRow::Dir { .. }));
        let depth = |row: &TreeRow| match row {
            TreeRow::Dir { depth, .. } | TreeRow::File { depth, .. } => *depth,
        };
        let scroll = match &source {
            FileSource::Uncommitted(..) => &self.uncommitted_files,
            FileSource::Diff => &self.diff_files,
        };
        let theme = Theme::get(cx);
        let (rows, places) = self.page_folds.place(
            rows,
            |row| match row {
                TreeRow::Dir { path, .. } => Some(PageFold::Dir(path.clone())),
                TreeRow::File { .. } => None,
            },
            |dir, row| depth(row) > depth(dir),
            |dir| rows_under(scroll, dir, theme),
        );
        let found = underlined(&rows, &paths, found);
        // Room for the widest counts any row shows: a file's own, or in a tree a folder's,
        // which all the files' together bound.
        let counted = self
            .counted_diff(&source)
            .map_or(&[][..], |diff| &diff.files[..]);
        let counts = counted.iter().map(FileDiff::stats);
        let columns = match tree {
            true => {
                let folders = diff::widest_up_to(diff::total_stats(counted));
                diff::stats_columns(counts.chain(folders))
            }
            false => diff::stats_columns(counts),
        };
        // The list follows the cursor to another file, as j and ↓ move it, or to the folded
        // directory the file is in, or to the folder the keys stepped onto.
        let folder = self.cursor_folder(cx).map(str::to_owned);
        let cursor = match &folder {
            Some(dir) => Some(dir.clone()),
            None => self
                .cursor_path()
                .filter(|path| paths.contains(&path.as_str())),
        };
        let cursor_row = cursor.as_deref().and_then(|cursor| {
            rows.iter().position(|row| match row {
                TreeRow::Dir { path, .. } if folder.is_some() => path == cursor,
                _ if folder.is_some() => false,
                TreeRow::File { pos, .. } => paths.get(*pos) == Some(&cursor),
                TreeRow::Dir {
                    path,
                    collapsed: true,
                    ..
                } => (cursor.strip_prefix(path.as_str())).is_some_and(|rest| rest.starts_with('/')),
                TreeRow::Dir { .. } => false,
            })
        });
        *self.listed_cursor_row.borrow_mut() = cursor_row.map(|row| (scroll.clone(), row));
        let follow = (*self.listed_cursor.borrow() != cursor)
            .then_some(cursor_row)
            .flatten();
        if follow.is_some() {
            *self.listed_cursor.borrow_mut() = cursor;
        }
        // Which rows are picked, while some are, so picked files next to each other join up.
        let picks = self.picked_paths();
        // A picked file's menu is all the picks'.
        let picked = (!picks.is_empty())
            .then(|| self.owned_changes(self.picked_changes(), cx))
            .flatten();
        let lit: Vec<bool> = match picks.is_empty() {
            true => Vec::new(),
            false => (rows.iter())
                .map(|row| {
                    matches!(row, TreeRow::File { pos, .. }
                        if paths.get(*pos).is_some_and(|path| picks.contains(path)))
                })
                .collect(),
        };
        // What's being dragged out of the list steps back: a file, and a folder once all it
        // holds is going.
        let carried = self.dragged_paths(&source, store, cx);
        let leaving: Vec<bool> = match carried.is_empty() {
            true => Vec::new(),
            false => {
                let stays: HashSet<&str> = (paths.iter().copied())
                    .filter(|path| !carried.contains(path))
                    .flat_map(|path| {
                        (path.match_indices('/')).filter_map(move |(end, _)| path.get(..end))
                    })
                    .collect();
                (rows.iter())
                    .map(|row| match row {
                        TreeRow::File { pos, .. } => {
                            paths.get(*pos).is_some_and(|path| carried.contains(path))
                        }
                        TreeRow::Dir { path, .. } => !stays.contains(path.as_str()),
                    })
                    .collect()
            }
        };
        let count = rows.len();
        let list = side_list(
            count,
            scroll,
            "files-scrollbar",
            cx,
            move |this, ix| (this.page_folds).moved(places.get(ix).and_then(Option::as_ref)),
            move |this, ix, theme, window, cx| {
                let row = match rows.get(ix)? {
                    dir @ TreeRow::Dir { .. } => {
                        let found = found.get(ix)?;
                        let holds_cursor = cursor_row == Some(ix);
                        // Solid as a file's, once the keys stepped onto it.
                        let solid = holds_cursor
                            && this.cursor_folder(cx).is_some()
                            && has_keys(&this.files_focus, window, cx);
                        let held = (holds_cursor, solid);
                        this.render_dir_row(dir, held, &source, found, columns, theme, cx)
                    }
                    TreeRow::File { pos, depth } => {
                        let lit = |ix: usize| lit.get(ix).copied().unwrap_or(false);
                        let row = FileRow {
                            depth: tree.then_some(*depth),
                            folders,
                            joined: match lit(ix) {
                                true => (ix > 0 && lit(ix - 1), lit(ix + 1)),
                                false => (false, false),
                            },
                            picked: lit(ix),
                            menu: picked.clone().filter(|_| lit(ix)),
                            found: found.get(ix)?.clone(),
                            columns,
                            ..Default::default()
                        };
                        // The current file is solid while the list holds the focus, as the
                        // navigator's selection is while it does.
                        let solid = has_keys(&this.files_focus, window, cx);
                        this.render_source_file(&source, *index.get(*pos)?, row, solid, theme, cx)
                    }
                }?;
                Some(match leaving.get(ix) {
                    Some(true) => div().opacity(0.5).child(row).into_any_element(),
                    _ => row,
                })
            },
        );
        // Once the list has its rows.
        if let Some(row) = follow {
            scroll.scroll_to_reveal_item(row);
        }
        (list, count)
    }

    /// The `pos`th file of `source`, with what its list shows beside it, beyond where it sits
    /// in the list and whether it's picked, `row`. Its menu is its own unless `row` has one.
    fn render_source_file(
        &self,
        source: &FileSource,
        pos: usize,
        row: FileRow,
        solid: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        match source {
            FileSource::Uncommitted(project, worktree) => {
                let store = self.store.read(cx);
                let workspace = store.workspace(project)?;
                let files = workspace.uncommitted_in(worktree)?;
                let Some(file) = files.get(pos) else {
                    // Opened to edit, with no changes to check, move or take.
                    let opened = self.opened_file(project, worktree, files)?;
                    let extras = FileRow {
                        diff_ix: (self.worktree_diff()).and_then(|diff| {
                            diff.files.iter().position(|f| f.file.path == opened.path)
                        }),
                        checkbox: None,
                        menu: None,
                        ..row
                    };
                    return Some(self.render_file_row(pos, opened, extras, solid, theme, cx));
                };
                let whole = Selected::whole(file);
                let extras = FileRow {
                    diff_ix: (self.worktree_diff())
                        .and_then(|diff| diff.files.iter().position(|f| f.file.path == file.path)),
                    checkbox: self.composing.then(|| self.checks.file_check(&file.path)),
                    menu: row.menu.or_else(|| {
                        let changes = Changes {
                            from: None,
                            worktree: worktree.clone(),
                            files: vec![whole].into(),
                        };
                        Some((project.clone(), changes))
                    }),
                    ..row
                };
                Some(self.render_file_row(pos, file, extras, solid, theme, cx))
            }
            FileSource::Diff => {
                let diff = self.diff.as_ref()?;
                let file = diff.files.get(pos)?;
                let extras = FileRow {
                    diff_ix: Some(pos),
                    menu: row.menu.or_else(|| self.diff_changes(pos, None, cx)),
                    ..row
                };
                Some(self.render_file_row(pos, &file.file, extras, solid, theme, cx))
            }
        }
    }

    /// The paths of the files of `source` being dragged, whole: some of a file leaves the rest
    /// behind.
    fn dragged_paths<'a>(
        &self,
        source: &FileSource,
        store: &'a Store,
        cx: &App,
    ) -> HashSet<&'a str> {
        let Some(Dragged {
            project,
            what:
                Moving::Files(Changes {
                    from,
                    worktree,
                    files,
                }),
        }) = &store.dragging
        else {
            return HashSet::new();
        };
        let here = match source {
            FileSource::Uncommitted(p, w) => from.is_none() && (p, w) == (project, worktree),
            FileSource::Diff => matches!(
                self.owned_changes(Vec::new(), cx),
                Some((p, Changes { from: f, worktree: w, .. }))
                    if (&p, &f, &w) == (project, from, worktree)
            ),
        };
        (files.iter())
            .filter(|file| here && file.hunks.is_none())
            .map(|file| file.file.path.as_str())
            .collect()
    }

    /// `files` of the loaded diff as changes, to move or take out of where they are:
    /// a commit listed in the navigator, or a checkout's uncommitted changes. A branch's diff
    /// spans commits, and a commit of the target's history, on the project page, isn't the
    /// workspace's to change: their files aren't.
    pub(super) fn owned_changes(
        &self,
        files: Vec<Selected>,
        cx: &App,
    ) -> Option<(ProjectId, Changes)> {
        let key = &self.diff.as_ref()?.key;
        let (from, worktree) = match &key.source {
            DiffSource::Worktree => (None, Checkout::Main),
            DiffSource::Linked(name) => (None, Checkout::Linked(name.clone())),
            // Not a conflicted commit's, which nothing takes from until it's resolved.
            DiffSource::Commit(id)
                if (self.store.read(cx).workspace(&key.project)).is_some_and(|ws| {
                    ws.commits()
                        .any(|commit| commit.id == *id && !commit.conflicted)
                }) =>
            {
                (Some(*id), Checkout::Main)
            }
            _ => return None,
        };
        let changes = Changes {
            from,
            worktree,
            files: files.into(),
        };
        Some((key.project.clone(), changes))
    }

    /// What the keys in the diff act on: the picked lines, or the picked files, or else the hunk
    /// under the cursor, or its file.
    pub fn cursor_changes(&self, cx: &App) -> Option<(ProjectId, Changes)> {
        // Right after selecting something else, the diff can still be the last one, another
        // project's.
        let project = self.selected_project();
        if self.diff.as_ref().map(|d| &d.key.project) != project.as_ref() {
            return None;
        }
        if self.page.line_picks().is_some() {
            return self.line_picks_changes(cx);
        }
        if self.page.has_file_picks() {
            return self.owned_changes(self.picked_changes(), cx);
        }
        let at = self.cursor.in_file()?;
        if !self.shown_files().contains(&at) {
            return None;
        }
        let file = self.diff.as_ref()?.files.get(at)?;
        let hunk = (self.cursor.at.hunk()).and_then(|hunk| Some(file.hunks().get(hunk)?.key));
        self.diff_changes(at, hunk.map(|key| vec![key]), cx)
    }

    /// What Discard takes where the keys are, in the diff or the file list: what's picked, else
    /// as [`discard_part`] says. `None` with the keys anywhere else, or nothing to take.
    pub(crate) fn discard_changes(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<(ProjectId, Changes)> {
        let in_list = self.files_focus.is_focused(window);
        if !in_list && !self.focus.is_focused(window) {
            return None;
        }
        let changes = self.cursor_changes(cx)?;
        if self.page.line_picks().is_some() || self.page.has_file_picks() {
            return Some(changes);
        }
        if self.cursor_folder(cx).is_some() {
            return None;
        }
        let file = self.current_file()?;
        let at = self.cursor.in_file()?;
        let cursor_file = self.diff.as_ref()?.files.get(at)?;
        // A header stays drawn over its file, so the file being looked at says that one.
        let in_sight = match self.cursor.at {
            Stop::Hunk(hunk) => {
                // Never widen a hunk discard to the whole file.
                cursor_file.hunks().get(hunk)?;
                self.cursor_in_sight(cx)
            }
            Stop::Message | Stop::File | Stop::Gap(_) => file == at,
        };
        let conflicted = cursor_file.file.conflicted;
        match discard_part(in_list, self.cursor.at, in_sight, conflicted)? {
            DiscardPart::Hunk => Some(changes),
            DiscardPart::File if self.shown_files().contains(&file) => {
                self.diff_changes(file, None, cx)
            }
            DiscardPart::File => None,
        }
    }

    /// Discard what [`Self::discard_changes`] takes, from its commit or its checkout.
    pub(super) fn discard_here(&mut self, window: &mut Window, cx: &mut Context<Detail>) {
        let Some((
            project,
            Changes {
                from,
                worktree,
                files,
            },
        )) = self.discard_changes(window, cx)
        else {
            return;
        };
        self.app
            .update_later(window, cx, move |app, window, cx| match from {
                Some(commit) => app.discard_from_commit(project, commit, files, window, cx),
                None => app.discard(project, worktree, files.to_vec(), window, cx),
            });
    }

    /// What Discard throws away with the keys here, in the words a menu has for it: what
    /// [`Self::discard_changes`] takes.
    pub(crate) fn discard_phrase(&self, window: &Window, cx: &App) -> Option<String> {
        let (_, Changes { from, files, .. }) = self.discard_changes(window, cx)?;
        let kind = change_kind(self.takes_lines(&files), &files);
        Some(match from {
            Some(_) => format!("Discard {kind} from commit"),
            None => format!("Discard {kind}"),
        })
    }

    /// The loaded diff's file at `file`, or `hunks` of it, to drag or squash.
    pub(super) fn diff_changes(
        &self,
        file: usize,
        hunks: Option<Vec<HunkKey>>,
        cx: &App,
    ) -> Option<(ProjectId, Changes)> {
        let part = self.diff.as_ref()?.files.get(file)?.part(hunks);
        self.owned_changes(vec![part], cx)
    }

    /// The loaded diff's picked files, whole, in its order: not those it doesn't show.
    pub(super) fn picked_changes(&self) -> Vec<Selected> {
        let picks = self.picked_paths();
        (self.shown_changes().into_iter())
            .filter(|s| picks.contains(s.file.path.as_str()))
            .collect()
    }

    /// The paths of the files picked.
    fn picked_paths(&self) -> HashSet<&str> {
        (self.page.files().into_iter())
            .flat_map(|files| files.file_picks.iter())
            .map(String::as_str)
            .collect()
    }

    /// The whole of each file the diff shows, as a filter leaves them.
    pub(super) fn shown_changes(&self) -> Vec<Selected> {
        let Some(diff) = &self.diff else {
            return Vec::new();
        };
        (self.shown_files().into_iter())
            .filter_map(|ix| diff.files.get(ix))
            .map(|f| Selected::whole(&f.file))
            .collect()
    }

    /// ⌘-click (`run` false) or ⇧-click on the loaded diff's file at `file`.
    fn pick_file(&mut self, file: usize, run: bool, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file) else {
            return;
        };
        let order = self.listed_paths(cx);
        // A cursor the list doesn't show, as when a filter hides it, isn't picked along.
        let cursor = (self.cursor_path())
            .filter(|cursor| order.contains(cursor))
            .unwrap_or_else(|| path.clone());
        let Some(files) = self.page.files_mut() else {
            return;
        };
        let next = match run {
            true => files.file_picks.run(&order, &cursor, &path),
            false => Some(files.file_picks.toggle(&cursor, &path)),
        };
        self.picked_file(next, cx);
    }

    /// Step `delta` files from the cursor, picking the run as ⇧-click does.
    pub(super) fn pick_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = match self.file_step(delta, false, cx) {
            Some(super::ListStop::Entry(super::Entry::File(file))) => self.diff_path(file),
            _ => None,
        };
        let (Some(cursor), Some(next)) = (self.cursor_path(), next) else {
            return;
        };
        let order = self.listed_paths(cx);
        let Some(files) = self.page.files_mut() else {
            return;
        };
        let next = files.file_picks.run(&order, &cursor, &next);
        self.picked_file(next, cx);
    }

    pub(super) fn pick_all_files(&mut self, cx: &mut Context<Self>) {
        let order = self.listed_paths(cx);
        if let Some(files) = self.page.files_mut() {
            files.file_picks.all(&order);
        }
        self.picked_file(None, cx);
    }

    /// After a pick, bring the cursor to `next`. Picks are of one list at a time, so commits
    /// picked in the navigator go.
    fn picked_file(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        if self.page.has_file_picks() && !self.store.read(cx).picked.is_empty() {
            self.store.update(cx, |store, cx| {
                store.picked.clear();
                cx.notify()
            });
        }
        let file = next.and_then(|next| {
            (self.diff.as_ref())?
                .files
                .iter()
                .position(|f| f.file.path == next)
        });
        if let Some(file) = file {
            self.scroll_to_file(file, true, cx);
        }
        cx.notify();
    }

    /// Let the picked files go. Whether any were.
    pub fn clear_file_picks(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(files) = (self.page.files_mut()).filter(|files| !files.file_picks.is_empty())
        else {
            return false;
        };
        files.file_picks.clear();
        cx.notify();
        true
    }

    /// Put the cursor on `cursor`, changes a right-click is on that aren't picked, and let
    /// every pick go, so the commands of its menu act on those changes alone.
    pub(super) fn right_click_on(&mut self, cursor: super::Cursor, cx: &mut Context<Self>) {
        self.clear_file_picks(cx);
        self.clear_line_picks(cx);
        self.cursor = cursor;
        cx.notify();
    }

    /// Pick the files of `folder`, as a right-click on it does, so the commands of its menu
    /// act on them: the cursor alone on one. Picks that hold them all stay, as a picked
    /// file's right-click keeps them.
    fn right_click_folder(&mut self, folder: Option<&Changes>, cx: &mut Context<Self>) {
        self.clear_line_picks(cx);
        let Some(folder) = folder else {
            return;
        };
        let paths: Vec<String> = (folder.files.iter()).map(|f| f.file.path.clone()).collect();
        let Some(state) = self.page.files_mut() else {
            return;
        };
        if !paths.iter().all(|path| state.file_picks.contains(path)) {
            state.file_picks.all(&paths);
            self.picked_file(paths.first().cloned(), cx);
        }
    }

    /// How many files are picked.
    pub fn picked_files(&self) -> usize {
        (self.page.files()).map_or(0, |files| files.file_picks.len())
    }

    pub(super) fn diff_path(&self, file: usize) -> Option<String> {
        Some(self.diff.as_ref()?.files.get(file)?.file.path.clone())
    }

    /// The paths of the files the list shows, in its order.
    pub(super) fn listed_paths(&self, cx: &App) -> Vec<String> {
        (self.listed_files(cx).into_iter())
            .filter_map(|file| self.diff_path(file))
            .collect()
    }

    /// The files under the folder `dir` in `source`, to act on together.
    fn folder_payload(
        &self,
        source: &FileSource,
        dir: &str,
        cx: &App,
    ) -> Option<(ProjectId, Changes)> {
        let prefix = format!("{dir}/");
        // Only what the folder shows: not files the filter or a pending operation hides.
        let terms = self.filter_terms(cx);
        let under = |path: &str| path.starts_with(&prefix) && keeps_path(&terms, path);
        let store = self.store.read(cx);
        let (project, changes) = match source {
            FileSource::Uncommitted(project, worktree) => {
                let changes = Changes {
                    from: None,
                    worktree: worktree.clone(),
                    files: (store.workspace(project)?.uncommitted_in(worktree)?.iter())
                        .filter(|f| under(&f.path) && !store.hides_file(project, worktree, &f.path))
                        .map(Selected::whole)
                        .collect(),
                };
                (project.clone(), changes)
            }
            FileSource::Diff => {
                let diff = self.diff.as_ref()?;
                let leaving = diff.key.leaving(store);
                let files = (diff.files.iter())
                    .map(|f| &f.file)
                    .filter(|f| under(&f.path) && !leaving(&f.path))
                    .map(Selected::whole)
                    .collect();
                self.owned_changes(files, cx)?
            }
        };
        (!changes.files.is_empty()).then_some((project, changes))
    }

    /// What dragging the folder `dir` in `source` carries: its [`Selected::movable`] files,
    /// each as its row would drag it. Not at all while what's checked of one is unknown.
    fn folder_drag(&self, source: &FileSource, dir: &str, cx: &App) -> Option<Dragged> {
        let (project, changes) = self.folder_payload(source, dir, cx)?;
        let Changes {
            from,
            worktree,
            files,
        } = changes.movable()?;
        let checked = self.composing && matches!(source, FileSource::Uncommitted(..));
        let files = (files.iter())
            .map(|whole| {
                let path = &whole.file.path;
                match checked && self.checks.file_check(path) == Toggled::Mixed {
                    true => self.selected_change(&whole.file, self.loaded_file(path)),
                    false => Some(whole.clone()),
                }
            })
            .collect::<Option<_>>()?;
        let changes = Changes {
            from,
            worktree,
            files,
        };
        Some(Dragged::from((project, changes)))
    }

    /// `files` of `source` as changes, if they're the workspace's to change.
    fn changes_from(
        &self,
        source: &FileSource,
        files: Vec<Selected>,
        cx: &App,
    ) -> Option<(ProjectId, Changes)> {
        match source {
            FileSource::Uncommitted(project, worktree) => {
                let changes = Changes {
                    from: None,
                    worktree: worktree.clone(),
                    files: files.into(),
                };
                Some((project.clone(), changes))
            }
            FileSource::Diff => self.owned_changes(files, cx),
        }
    }

    /// The preview of a drag of the folder `dir` in `source`, which works out what it carries
    /// as it starts: every row drawn would otherwise copy all its files, for a drag that rarely
    /// comes. What GPUI holds goes unread, as drops read [`Store::dragging`].
    fn folder_preview(
        &self,
        source: &FileSource,
        dir: &str,
        chip: Chip,
        cx: &Context<Self>,
    ) -> impl Fn(&Dragged, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static
    {
        let detail = cx.entity().downgrade();
        let preview = self.drag_preview(chip, false, detail.clone());
        let (source, dir) = (source.clone(), dir.to_owned());
        move |nothing, offset, window, cx| {
            let carried = (detail
                .read_with(cx, |detail, cx| detail.folder_drag(&source, &dir, cx)))
            .ok()
            .flatten();
            preview(carried.as_ref().unwrap_or(nothing), offset, window, cx)
        }
    }

    /// The diff the files of `source` take their line counts from.
    fn counted_diff(&self, source: &FileSource) -> Option<&LoadedDiff> {
        match source {
            FileSource::Uncommitted(..) => self.worktree_diff(),
            FileSource::Diff => self.diff.as_ref(),
        }
    }

    /// The lines the files under the folder `dir` in `source` add and remove, summed from the
    /// diff the list's files take theirs from, over the files it shows.
    fn folder_stats(&self, source: &FileSource, dir: &str, cx: &App) -> (u32, u32) {
        let Some(loaded) = self.counted_diff(source) else {
            return (0, 0);
        };
        let prefix = format!("{dir}/");
        let terms = self.filter_terms(cx);
        let leaving = loaded.key.leaving(self.store.read(cx));
        diff::total_stats(loaded.files.iter().filter(|f| {
            let path = f.file.path.as_str();
            path.starts_with(&prefix) && keeps_path(&terms, path) && !leaving(path)
        }))
    }

    /// What fills the diff's pane while the file list's keys are on the tree's folder `dir`: a
    /// card as the diff's files have, its header the folder's as theirs is the file's, then a
    /// row for each of its files, as the file list orders them, a click on which opens it.
    pub(super) fn render_folder_summary(
        &self,
        dir: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(diff) = &self.diff else {
            return div().into_any_element();
        };
        let files = self.folder_files(dir, cx);
        let listed = || (files.iter()).filter_map(|ix| diff.files.get(*ix));
        let (added, removed) = diff::total_stats(listed());
        let columns = diff::stats_columns(listed().map(FileDiff::stats));
        if self.summarized.borrow().as_deref() != Some(dir) {
            *self.summarized.borrow_mut() = Some(dir.to_owned());
            self.folder_rows.scroll_to(gpui::ListOffset::default());
        }
        let (name, parent) = diff::split_path(dir);
        let header = h_flex()
            .flex_none()
            .h(dp(diff::FILE_HEADER_HEIGHT))
            .pl(dp(10.))
            .pr(dp(12.))
            .gap(dp(8.))
            .border_b_1()
            .border_color(theme.border_2)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(dp(6.))
                    .child(folder_icon())
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme.text_1)
                            .child(name.to_owned()),
                    )
                    .child(
                        (div().min_w_0().overflow_hidden().whitespace_nowrap())
                            .text_ellipsis_middle()
                            .text_color(theme.text_3)
                            .child(parent.to_owned()),
                    ),
            )
            .child(faint(plural(files.len(), "file"), theme).flex_none())
            .child(diff::stats(added, removed, false, theme))
            .children(diff::change_scale(added, removed, theme));
        let height = files.len() as f32 * ROW_HEIGHT + 8.;
        let prefix = format!("{dir}/");
        let files = std::rc::Rc::new(files);
        let rows = side_list(
            files.len(),
            &self.folder_rows,
            "folder-summary-scrollbar",
            cx,
            |_, _| None,
            move |this, at, theme, _, cx| {
                let ix = *files.get(at)?;
                let file = this.diff.as_ref()?.files.get(ix)?;
                let path = &file.file.path;
                let (name, sub) = diff::split_path(path.strip_prefix(&prefix).unwrap_or(path));
                let (added, removed) = file.stats();
                let row = list_row(("folder-file", ix), false, theme)
                    .mx(dp(4.))
                    .pl(dp(8.))
                    .pr(dp(4.))
                    .gap(dp(8.))
                    .text_size(dp(13.))
                    .aria_label(path.clone())
                    .child(file_icon(path, cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(dp(8.))
                            .child(div().flex_none().child(name.to_owned()))
                            .child(
                                (div().min_w_0().overflow_hidden().whitespace_nowrap())
                                    .text_ellipsis_middle()
                                    .text_color(theme.text_3)
                                    .child(sub.to_owned()),
                            ),
                    )
                    .child(diff::column_stats(added, removed, columns, false, theme))
                    .child(
                        div()
                            .w(dp(MARK_WIDTH))
                            .flex_none()
                            .flex()
                            .justify_center()
                            .text_size(dp(11.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(diff::change_color(file.file.kind, theme))
                            .child(file.file.kind.letter()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.scroll_to_file(ix, true, cx)
                    }));
                Some(row.into_any_element())
            },
        );
        // As tall as its rows, till the pane runs out.
        let card = v_flex()
            .w_full()
            .max_h_full()
            .min_h_0()
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_2)
            .bg(theme.bg_1)
            .overflow_hidden()
            .child(header)
            .child(
                v_flex()
                    .h(dp(height))
                    .min_h_0()
                    .flex_shrink(1.)
                    .py(dp(4.))
                    .child(rows),
            );
        v_flex()
            .id("folder-summary")
            .flex_1()
            .min_w_0()
            .h_full()
            .p(dp(12.))
            .bg(theme.bg_1)
            .child(card)
            .into_any_element()
    }

    /// A tree's folder row, with the byte ranges of its name the filter `found`, its end lined
    /// up with the list's other rows at `columns`.
    #[expect(clippy::too_many_arguments)]
    fn render_dir_row(
        &self,
        row: &TreeRow,
        (holds_cursor, solid): (bool, bool),
        source: &FileSource,
        found: &[Range<usize>],
        columns: (usize, usize),
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
        let (added, removed) = self.folder_stats(source, &path, cx);
        let always_counts = self.store.read(cx).prefs.folder_line_counts;
        let chip = Chip::Folder(name.clone().into());
        let fold = PageFold::Dir(path.clone());
        let open = !collapsed && !self.page_folds.closing(&fold);
        let chevron = fold::chevron("fold", open, theme.text_3).into_any_element();
        let row = file_row(
            ElementId::Name(format!("dir-{path}").into()),
            holds_cursor,
            solid,
            theme,
        )
        .group(ROW_GROUP)
        .child(steps(depth, Some(chevron), theme))
        .child(folder_icon())
        // Cut in the middle: a chain of folders, `ios/App/Resources/de.lproj`, ends in the one
        // that tells it from its siblings. Folded, how many files it holds follows the name, so
        // its end lines up with the files'.
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(dp(8.))
                .child(
                    (div().min_w_0().overflow_hidden().whitespace_nowrap())
                        .text_ellipsis_middle()
                        .child(matched(name, found.iter().cloned())),
                )
                .when(!open, |el| {
                    el.child(faint(files.to_string(), theme).flex_none())
                }),
        )
        // Only under the pointer unless picked in Settings: always there, they crowd the names.
        .when(added + removed > 0, |el| {
            el.child(
                diff::column_stats(added, removed, columns, false, theme)
                    .when(!always_counts, |el| {
                        el.opacity(0.).group_hover(ROW_GROUP, |s| s.opacity(1.))
                    }),
            )
        })
        .child(div().flex_none().w(dp(MARK_WIDTH)))
        .when_some(self.changes_from(source, Vec::new(), cx), |el, nothing| {
            let nothing = Dragged::from(nothing);
            el.on_drag(nothing, self.folder_preview(source, &path, chip, cx))
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener({
                let (source, path) = (source.clone(), path.clone());
                move |this, event: &MouseDownEvent, _, cx| {
                    let payload = this.folder_payload(&source, &path, cx);
                    this.right_click_folder(payload.as_ref().map(|(_, changes)| changes), cx);
                    let at = At::Folder(path.clone());
                    this.changes_menu(payload, at, None, true, event.position, cx)
                }
            }),
        )
        .on_click(
            cx.listener(move |this, _: &ClickEvent, _, cx| this.fold_dir(path.clone(), None, cx)),
        );
        Some(row.into_any_element())
    }

    /// The checkbox for all the files the composer can take, while it's open and there are
    /// any: a click checks them all, or once they all are, unchecks them.
    fn check_all_box(&self, theme: &Theme, cx: &Context<Self>) -> Option<Stateful<Div>> {
        if !self.composing {
            return None;
        }
        let all = self.checks.all_check(&self.checkable_paths(cx))?;
        let check = all != Toggled::True;
        let (tip, key) = match check {
            true => ("Check all files", shortcut(&CheckAll, cx)),
            false => ("Uncheck all files", shortcut(&UncheckAll, cx)),
        };
        Some(
            // An id for each, as a tooltip stays as it showed, over a click that changes it.
            checkbox(("check-all", usize::from(check)), all, theme)
                .aria_label(tip)
                .tooltip(Tooltip::keyed(tip, key))
                // The keys stay where they are, as in the navigator placing the commit.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.set_all_checked(check, cx)),
                ),
        )
    }

    /// A file list's header: `check_all`, how many `paths` it has, the lines `diffs` add and
    /// remove, and ways to filter it or show it as a tree. While the filter is open: the
    /// filter, how many of `paths` it keeps, and a way to close it.
    fn list_header<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        diffs: impl Iterator<Item = &'a FileDiff>,
        check_all: Option<Stateful<Div>>,
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
                        .min_w_0()
                        .text_size(dp(13.))
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
        header_row()
            .children(check_all)
            .child(files_heading(
                paths.count(),
                diff::total_stats(diffs),
                theme,
            ))
            .child(div().flex_1())
            .child(
                icon_button("filter-files", Icon::Search)
                    .small()
                    .tooltip_for("Filter files", &FilterFiles)
                    .on_click(cx.listener(|this, _, window, cx| this.open_file_filter(window, cx))),
            )
            .child(
                icon_button("file-view", Icon::Kebab)
                    .small()
                    .tooltip("View options")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.open_menu(event.position(), |this, cx| this.view_items(cx), cx)
                    })),
            )
    }

    /// The file list's ⋯ menu, as but.dev's: the files as a list or as a tree, by name, with
    /// docs first, or the last written first. By the last written, files with no write time,
    /// as a commit's, go after those with one, by name.
    fn view_items(&self, cx: &App) -> Vec<MenuItem> {
        let prefs = &self.store.read(cx).prefs;
        let (view, sort) = (prefs.file_view, prefs.file_sort);
        let show = |label: &str, as_view: FileView| {
            MenuItem::new(label.to_owned(), move |this, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.file_view = as_view, cx)
                })
            })
            .ticked(view == as_view)
        };
        let by = |label: &str, by: FileSort| {
            MenuItem::new(label.to_owned(), move |this, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.file_sort = by, cx)
                })
            })
            .ticked(sort == by)
        };
        // Files with no changes, to edit, as Go to anything finds them too.
        let open = (self.openable_checkout()).map(|_| {
            MenuItem::keyed(
                "Open a file…",
                &crate::palette::GoTo,
                cx,
                |this, window, cx| this.open_file_picker(String::new(), window, cx),
            )
        });
        (open.into_iter())
            .chain([
                show("As list", FileView::List).under("View"),
                show("As tree", FileView::Tree),
                by("Name", FileSort::Name).separated().under("Sort by"),
                by("Docs first", FileSort::DocsFirst),
                by("Last modified", FileSort::Modified),
            ])
            .collect()
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
        // The letters the filter found, underlined where the name and its folder show them.
        let name_found = found_in(&extras.found, file.path.len() - name.len()..file.path.len());
        let dir_found = found_in(&extras.found, 0..dir.len());
        let path = file.path.clone();
        let diff_ix = extras.diff_ix;
        let file_diff = diff_ix.and_then(|ix| self.diff.as_ref()?.files.get(ix));
        let stats = file_diff.map(FileDiff::stats);
        // Edited in the diff, which the row marks quietly.
        let editing = file_diff.is_some_and(|f| self.edits_file(f));
        // Opened to edit with nothing changed yet: the pencil in the change letter's place, and
        // no counts.
        let opened = editing && self.opened_unchanged(file) && stats == Some((0, 0));
        let unsaved = file_diff.is_some_and(|f| self.unsaved(f));
        let stats = stats.filter(|_| !opened);
        let current = diff_ix.is_some_and(|ix| Some(ix) == self.current_file());
        let picked = extras.picked;
        let solid = (current || picked) && solid;
        // Not while the keys are on a folder, whose summary hides the diff's cursor.
        let on_folder = self.cursor_folder(cx).is_some();
        let (current, solid) = (current && !on_folder, solid && (picked || !on_folder));
        // On the solid ground, as but-ui-react's FileList inverts a selected row's contents.
        let muted = match solid {
            true => theme.muted_on_solid(),
            false => theme.text_2,
        };
        // In a tree a file sits under its directory, which names its path.
        let dir = if extras.depth.is_some() { "" } else { dir };
        // While composing, the checkbox takes the icon's place once it's checked, or while
        // the pointer is on the row.
        let lead = match extras.checkbox {
            None => file_icon(&file.path, cx).into_any_element(),
            Some(check) => {
                let on = check != Toggled::False;
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
                    .size(dp(14.))
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
        // Its menu is a right-click away, as a commit's is.
        let mark = div().map(|el| match (opened, file.conflicted) {
            (true, _) => el.child(
                div()
                    .id(("opened", ix))
                    .child(small_icon(Icon::Edit).text_color(muted))
                    .tooltip(Tooltip::text("Opened to edit, nothing changed yet")),
            ),
            (false, true) => el.child(small_icon(Icon::Warning).text_color(theme.warn)),
            (false, false) => el
                .text_size(dp(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(diff::change_color(file.kind, theme))
                .child(file.kind.letter()),
        });
        let end = div()
            .flex_none()
            .w(dp(MARK_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(mark);
        // Copied while the list has the keys, not the files picked with it.
        let blink = (current && solid)
            .then(|| self.blink(Copied::Cursor))
            .flatten();
        let mut row = file_row(("file", ix), current || picked, solid, theme)
            .when_some(blink, |el, n| {
                let shape = div().absolute().inset_0().rounded(dp(4.));
                el.relative()
                    .child(blink_once(("blink", n), shape, theme.pop))
            })
            .map(|el| crate::ui::joined(el, extras.joined, solid, theme))
            .aria_label(file.path.clone())
            .aria_description(file.kind.word())
            .group(ROW_GROUP)
            .children(extras.depth.and_then(|depth| {
                let room = extras.folders.then(|| div().into_any_element());
                (depth > 0 || room.is_some()).then(|| steps(depth, room, theme))
            }))
            .child(lead)
            // The name whole where it fits, then its folder dimmed and cut in the middle, as the
            // folder's end tells two files of one name apart.
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(dp(8.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .max_w_full()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis_middle()
                            .child(matched(name.to_owned(), name_found)),
                    )
                    // Typed in and not saved yet, as the diff's header marks it.
                    .when(unsaved, |el| {
                        el.child(
                            div()
                                .id(("unsaved", ix))
                                .flex_none()
                                .child(diff::unsaved_dot(muted))
                                .tooltip(Tooltip::keyed(
                                    "Not saved",
                                    shortcut(&super::SaveEdits, cx),
                                )),
                        )
                    })
                    .when(!dir.is_empty(), |el| {
                        el.child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_middle()
                                .text_color(muted)
                                .child(matched(dir.to_owned(), dir_found)),
                        )
                    }),
            )
            // Being edited in the diff: a quiet pencil.
            .when(editing && !opened, |el| {
                el.child(
                    div()
                        .id(("editing", ix))
                        .flex_none()
                        .child(small_icon(Icon::Edit).text_color(muted))
                        .tooltip(Tooltip::text("Being edited")),
                )
            })
            .when_some(stats, |el, (added, removed)| {
                el.child(diff::column_stats(
                    added,
                    removed,
                    extras.columns,
                    solid,
                    theme,
                ))
            })
            .child(end)
            .when_some(diff_ix, |el, diff_ix| {
                // ⌘ (ctrl off macOS) or ⇧ picks files. A plain press on a picked file keeps
                // the picks, for dragging them, until it's let go as a click.
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let (toggle, shift) = (event.modifiers.secondary(), event.modifiers.shift);
                        if toggle || shift {
                            return this.pick_file(diff_ix, shift, cx);
                        }
                        if !picked {
                            this.clear_file_picks(cx);
                        }
                        this.scroll_to_file(diff_ix, true, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    let held = event.modifiers();
                    if picked && !held.secondary() && !held.shift {
                        this.clear_file_picks(cx);
                    }
                }))
            });
        // A file partly checked drags as its checked hunks and lines. Not at all while
        // they're unknown, rather than as all of it.
        let dragged = match &extras.menu {
            Some((project, changes)) if !picked && extras.checkbox == Some(Toggled::Mixed) => {
                let loaded = self.loaded_file(&file.path);
                (self.selected_change(file, loaded)).map(|change| {
                    let changes = Changes {
                        files: vec![change].into(),
                        ..changes.clone()
                    };
                    (project.clone(), changes)
                })
            }
            menu => menu.clone(),
        };
        if let Some(dragged) = dragged {
            let label: SharedString = dragged.1.describe().into();
            row = row.on_drag(
                Dragged::from(dragged),
                self.drag_preview(label, false, cx.entity().downgrade()),
            );
        }
        let (menu, path) = (extras.menu, file.path.clone());
        row.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                // A file not picked is the menu's alone, selected as a click selects it, so
                // the menu doesn't look like the selected file's. Lines picked anywhere would
                // be what its commands act on.
                this.clear_line_picks(cx);
                if !picked {
                    this.clear_file_picks(cx);
                    if let Some(diff_ix) = diff_ix {
                        this.scroll_to_file(diff_ix, true, cx);
                    }
                }
                let at = At::File(path.clone(), None);
                this.changes_menu(menu.clone(), at, None, true, event.position, cx)
            }),
        )
        .into_any_element()
    }

    /// What right-clicking files, a folder, lines or a hunk offers: what can be done to the
    /// changes in `payload`, when they're the workspace's to change, or going to the last
    /// commit of `blamed`, then opening, copying or revealing the path `at`. A branch outside the
    /// workspace, or a commit of the target, has only the last. `in_list` for a row of the file
    /// list, whose file it copies whole.
    pub(super) fn changes_menu(
        &mut self,
        payload: Option<(ProjectId, Changes)>,
        at: At,
        blamed: Option<Blamed>,
        in_list: bool,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(
            position,
            move |this, cx| this.changes_items(payload, at, blamed, in_list, cx),
            cx,
        );
    }

    /// What ⌘K starts with and ⇧F10 opens while the file list or the diff has the keys: the
    /// menu of the file under the cursor, as right-clicking it opens. `None` elsewhere.
    pub(crate) fn focused_file_items(
        &mut self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> Option<Vec<MenuItem>> {
        if !(self.files_focus.is_focused(window) || self.focus.is_focused(window)) {
            return None;
        }
        let path = self.shown_cursor_path(cx)?;
        // A picked file's menu is all the picks', as on its row.
        let picked = (self.page.files()).is_some_and(|files| files.file_picks.contains(&path));
        let payload = match picked {
            true => self.owned_changes(self.picked_changes(), cx),
            false => self.diff_changes(self.current_file()?, None, cx),
        };
        let at = At::File(path, None);
        let in_list = self.files_focus.is_focused(window);
        Some(self.changes_items(payload, at, None, in_list, cx))
    }

    /// The path of the file under the cursor while the file list or the diff has the keys,
    /// and the full path to it on disk.
    pub(crate) fn focused_file_paths(
        &self,
        window: &gpui::Window,
        cx: &App,
    ) -> Option<(String, String)> {
        if !(self.files_focus.is_focused(window) || self.focus.is_focused(window)) {
            return None;
        }
        let path = match self.cursor_folder(cx) {
            Some(dir) => dir.to_owned(),
            None => self.shown_cursor_path(cx)?,
        };
        let key = &self.diff.as_ref()?.key;
        let root = self.checkout_root(&key.project, &key.files_checkout(), cx)?;
        // Git's paths use `/`; Windows' own `\`.
        let full = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
        Some((path, full.display().to_string()))
    }

    /// The folder `checkout`'s files are in: a linked worktree's own.
    fn checkout_root(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        cx: &App,
    ) -> Option<std::path::PathBuf> {
        let store = self.store.read(cx);
        match checkout.linked() {
            None => store.path(project),
            Some(name) => (store.workspace(project))
                .and_then(|ws| ws.find_worktree(name))
                .map(|w| w.path.clone()),
        }
    }

    fn changes_items(
        &mut self,
        payload: Option<(ProjectId, Changes)>,
        at: At,
        blamed: Option<Blamed>,
        in_list: bool,
        cx: &mut Context<Self>,
    ) -> Vec<MenuItem> {
        let (project, worktree) = match &payload {
            Some((project, changes)) => (project.clone(), changes.worktree.clone()),
            None => match &self.diff {
                Some(diff) => (diff.key.project.clone(), diff.key.files_checkout()),
                None => return Vec::new(),
            },
        };
        let mut items =
            payload.map_or_else(Vec::new, |(_, changes)| self.change_items(changes, cx));
        if let (Some(blamed), At::File(path, _)) = (blamed, &at) {
            items.extend(self.last_commit_item(path, blamed, cx));
        }
        items.extend(self.path_history_item(&project, &at, cx));
        let mut paths = self
            .path_items(&project, worktree, at, in_list, cx)
            .into_iter();
        // Apart from what changes them.
        if let Some(first) = (!items.is_empty()).then(|| paths.next()).flatten() {
            items.push(first.separated());
        }
        items.extend(paths);
        items
    }

    /// What the changes the keys are on are called: picked lines, a hunk, a file or files, as
    /// [`Self::cursor_changes`] takes them. What a menu calls its commands for them.
    pub(crate) fn changes_kind(&self, cx: &App) -> Option<&'static str> {
        let (_, Changes { files, .. }) = self.cursor_changes(cx)?;
        Some(change_kind(self.takes_lines(&files), &files))
    }

    /// Register what's done to the changes the keys are on, as their menu offers it: marking
    /// the conflicted ones of a checkout resolved, and taking a commit's out of it.
    pub(super) fn change_actions(&self, el: Stateful<Div>, cx: &Context<Self>) -> Stateful<Div> {
        let Some((_, Changes { from, files, .. })) = self.cursor_changes(cx) else {
            return el;
        };
        match from {
            None => el.when(files.iter().any(|f| f.file.conflicted), |el| {
                el.on_action(cx.listener(|this, _: &MarkResolved, _, cx| this.mark_resolved(cx)))
            }),
            // All of it would leave the message on an empty commit.
            Some(_) => el
                .on_action(cx.listener(|this, _: &UncommitChanges, window, cx| {
                    this.take_from_commit(Taken::Uncommit, window, cx)
                }))
                .when(!self.takes_all(&files), |el| {
                    el.on_action(cx.listener(|this, _: &SplitIntoNewCommit, window, cx| {
                        this.take_from_commit(Taken::Split, window, cx)
                    }))
                }),
        }
    }

    /// Mark the conflicted ones of the uncommitted changes the keys are on resolved.
    fn mark_resolved(&mut self, cx: &mut Context<Self>) {
        let Some((
            project,
            Changes {
                from: None,
                worktree,
                files,
            },
        )) = self.cursor_changes(cx)
        else {
            return;
        };
        let paths: Vec<bstr::BString> = (files.iter())
            .filter(|f| f.file.conflicted)
            .map(|f| f.file.change.path.clone())
            .collect();
        // Undo can't bring back a linked worktree, or a commit checked out for editing.
        let undoable = !worktree.is_linked()
            && (self.store.read(cx).workspace(&project)).is_some_and(|ws| ws.editing.is_none());
        let resolved = backend::MarkResolved {
            paths,
            checkout: worktree.clone(),
        };
        self.store.update(cx, |store, cx| {
            let overlay = Overlay {
                rereads: Rereads::Files,
                files_in: worktree,
                ..Overlay::default()
            };
            store.apply(
                &project,
                "Mark resolved",
                overlay,
                resolved,
                move |store, (), cx| store.snack_after(undoable, "Marked resolved", cx),
                cx,
            )
        })
    }

    /// Take the changes the keys are on out of their commit: into the uncommitted changes, or
    /// a new commit of their own.
    fn take_from_commit(&mut self, taken: Taken, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, changes)) = self.cursor_changes(cx) else {
            return;
        };
        let Some(commit) = changes.from else {
            return;
        };
        let files = changes.files.clone();
        let what = Moving::Files(changes);
        self.app
            .update_later(window, cx, move |app, window, cx| match taken {
                Taken::Uncommit => {
                    // A linked worktree's commit uncommits into that worktree.
                    let store = app.store.read(cx);
                    let uncommitted =
                        match store.worktree_for(&Selection::Commit(project.clone(), commit)) {
                            Some(worktree) => Selection::Uncommitted(
                                project.clone(),
                                Checkout::Linked(worktree.name.clone()),
                            ),
                            None => Selection::Uncommitted(project.clone(), Checkout::Main),
                        };
                    app.transfer(
                        &what,
                        &uncommitted,
                        crate::transfer::Place::Into,
                        window,
                        cx,
                    )
                }
                Taken::Split => app.split_off(project, commit, files, cx),
            })
    }

    /// What can be done to the changes in `payload` from a menu: squashing them, and moving,
    /// taking out or discarding them, as the keys do to the picked ones or the ones under the
    /// cursor, which a right-click makes them. Squashing what's uncommitted amends, so it says
    /// that here, as ⌘K leaves the command out for it.
    fn change_items(&self, changes: Changes, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let from = changes.from;
        let mut items: Vec<MenuItem> = (changes.movable().into_iter())
            .map(|_| match from {
                Some(_) => MenuItem::command(&crate::app::SquashSelection, cx),
                None => MenuItem::action("Amend", &crate::app::SquashSelection, cx),
            })
            .collect();
        match from {
            None => {
                items.insert(0, MenuItem::command(&MarkResolved, cx));
                items.push(MenuItem::discard_here(cx));
            }
            Some(_) => items.extend([
                MenuItem::command(&UncommitChanges, cx),
                MenuItem::command(&SplitIntoNewCommit, cx),
                MenuItem::discard_here(cx),
            ]),
        }
        items
    }

    /// Typing in the file at `path` where the diff shows it, as a double click on a line does,
    /// or once it's typed in, putting the caret away and saving.
    fn edit_items(&self, path: &str, cx: &App) -> Vec<MenuItem> {
        let Some((file, file_diff)) = (self.diff.as_ref()).and_then(|diff| {
            diff.files
                .iter()
                .enumerate()
                .find(|(_, f)| f.file.path == path)
        }) else {
            return Vec::new();
        };
        let edited = self.edits_file(file_diff);
        let mut items = Vec::new();
        // On the page with the keys, which the menu gives back once it closes.
        let on_page = |f: fn(&mut Detail, &mut Window, &mut Context<Detail>)| {
            move |app: &mut crate::app::AppView,
                  window: &mut Window,
                  cx: &mut Context<crate::app::AppView>| {
                if let Some(detail) = app.detail_with_keys(window, cx).cloned() {
                    detail.update(cx, |detail, cx| f(detail, window, cx));
                }
            }
        };
        if edited && self.has_caret() {
            items.push(MenuItem::keyed(
                "Stop editing",
                &super::FinishEdit,
                cx,
                on_page(Detail::leave_edit),
            ));
        } else if edited || self.editable(file, cx).is_ok() {
            let path = path.to_owned();
            items.push(MenuItem::keyed(
                "Edit file",
                &super::EditLines,
                cx,
                move |app, window, cx| {
                    if let Some(detail) = app.detail_with_keys(window, cx).cloned() {
                        let path = path.clone();
                        detail.update(cx, |detail, cx| detail.open_file(path, window, cx));
                    }
                },
            ));
        }
        if self.unsaved(file_diff) {
            items.push(MenuItem::keyed(
                "Save",
                &super::SaveEdits,
                cx,
                // Back to typing, where the keys were.
                on_page(|detail, window, cx| {
                    detail.save_now(cx);
                    if let Some(caret) = detail.caret_focus(cx) {
                        window.focus(&caret, cx);
                    }
                }),
            ));
        }
        items
    }

    /// Opening, copying or revealing the path `at` of `project`'s `checkout`, and copying the
    /// hunk or file's diff the cursor is on there: the file's whole diff `in_list`, as the file
    /// list has the keys. What isn't on disk, as a deleted file or one only a branch outside
    /// the workspace has, can only have its path or diff copied.
    fn path_items(
        &self,
        project: &ProjectId,
        checkout: Checkout,
        at: At,
        in_list: bool,
        cx: &App,
    ) -> Vec<MenuItem> {
        let Some(root) = self.checkout_root(project, &checkout, cx) else {
            return Vec::new();
        };
        let folder = matches!(at, At::Folder(_));
        let (relative, line) = match at {
            At::File(path, line) => (path, Some(line)),
            At::Folder(path) => (path, None),
        };
        // Git's paths use `/`; Windows' own `\`, which "Copy absolute path" and Explorer expect.
        let absolute = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let full = absolute.display().to_string();
        #[expect(
            clippy::disallowed_methods,
            reason = "one check as the menu opens, in a checkout the app is watching"
        )]
        let on_disk = absolute.exists();
        let mut items = Vec::new();
        if line.is_some() {
            items.extend(self.edit_items(&relative, cx));
        }
        // The file under the cursor, where a right-click put it.
        if line.is_some() && on_disk {
            items.push(MenuItem::command(&crate::open_in::OpenInEditor, cx));
        }
        // What ⌘C copies there: the hunk the cursor is on or the file's whole diff, else its
        // path.
        let copy = &crate::app::CopySelection;
        let cursor_here = (self.shown_cursor_path(cx)).is_some_and(|path| path == relative);
        let diff =
            (cursor_here.then(|| self.cursor_hunk(in_list, cx)).flatten()).map(|(_, hunk)| {
                match hunk {
                    Some(_) => "Copy hunk",
                    None => "Copy diff",
                }
            });
        if let Some(label) = diff {
            // As the keys were when it opened: the menu has them now.
            items.push(MenuItem::keyed(label, copy, cx, move |this, _, cx| {
                let patch = (this.detail).update(cx, |detail, cx| detail.copy_patch(in_list, cx));
                if let Some(patch) = patch {
                    this.copy_text(patch, cx)
                }
            }));
        }
        items.extend(match folder {
            false => {
                let copy_relative = MenuItem::command(&crate::app::CopyRelativePath, cx);
                [
                    match diff {
                        Some(_) => copy_relative,
                        None => copy_relative.keys(copy, cx),
                    },
                    MenuItem::command(&crate::app::CopyAbsolutePath, cx),
                ]
            }
            // The commands copy the file under the cursor, which is never on a folder.
            true => [
                MenuItem::new("Copy relative path", move |this, _, cx| {
                    this.copy_text(relative.clone(), cx)
                }),
                MenuItem::new("Copy absolute path", move |this, _, cx| {
                    this.copy_text(full.clone(), cx)
                }),
            ],
        });
        if on_disk {
            items.push(match folder {
                false => MenuItem::command(&crate::navigator::Reveal, cx),
                true => MenuItem::new(crate::ui::REVEAL, move |_, _, cx| cx.reveal_path(&absolute)),
            });
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{DiscardPart, discard_part, underlined};
    use crate::detail::Stop;
    use crate::detail::file_tree::{TreeRow, tree_rows};
    use crate::detail::path_match;

    #[test]
    fn discard_takes_the_hunk_the_cursor_is_on() {
        assert_eq!(
            discard_part(false, Stop::Hunk(1), true, false),
            Some(DiscardPart::Hunk)
        );
    }

    #[test]
    fn discard_takes_the_file_from_its_header() {
        assert_eq!(
            discard_part(false, Stop::File, true, false),
            Some(DiscardPart::File)
        );
    }

    #[test]
    fn discard_takes_nothing_from_the_box_above_a_hunk() {
        assert_eq!(
            discard_part(false, Stop::Gap(0), true, false),
            None,
            "it stands for lines that didn't change, so it's neither the hunk nor the file"
        );
    }

    #[test]
    fn discard_takes_nothing_from_a_header_scrolled_away() {
        assert_eq!(
            discard_part(false, Stop::File, false, false),
            None,
            "the file on screen isn't the one the cursor is on"
        );
    }

    #[test]
    fn discard_takes_nothing_from_a_hunk_scrolled_away() {
        assert_eq!(
            discard_part(false, Stop::Hunk(0), false, false),
            None,
            "what's on screen isn't what the cursor is on"
        );
    }

    #[test]
    fn discard_takes_nothing_from_a_conflicted_files_hunk() {
        assert_eq!(
            discard_part(false, Stop::Hunk(0), true, true),
            None,
            "a conflicted file goes whole or not at all"
        );
    }

    #[test]
    fn discard_takes_nothing_from_the_commit_message() {
        assert_eq!(discard_part(false, Stop::Message, true, false), None);
    }

    #[test]
    fn discard_takes_the_file_from_its_row_though_the_cursor_is_on_a_hunk() {
        assert_eq!(
            discard_part(true, Stop::Hunk(2), true, false),
            Some(DiscardPart::File),
            "the row is what's selected, and its hunk shows faint"
        );
    }

    #[test]
    fn a_folder_underlines_what_the_filter_found_in_its_name() {
        let paths = ["assets/keymaps/default.json", "src/keymap.rs"];
        let terms = vec!["keymap".to_owned()];
        let found = (paths.iter())
            .map(|path| path_match(&terms, path).map(|(_, found)| found))
            .collect::<Option<Vec<_>>>()
            .expect("the filter keeps both");
        let rows = tree_rows(&paths, &HashSet::new());
        let shown: Vec<(&str, Vec<&str>)> = (rows.iter().zip(underlined(&rows, &paths, found)))
            .map(|(row, found)| {
                let text = match row {
                    TreeRow::Dir { name, .. } => name.as_str(),
                    TreeRow::File { pos, .. } => paths.get(*pos).copied().unwrap_or_default(),
                };
                let found = found.into_iter().filter_map(|range| text.get(range));
                (text, found.collect())
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("assets/keymaps", vec!["keymap"]),
                ("assets/keymaps/default.json", vec!["keymap"]),
                ("src", vec![]),
                ("src/keymap.rs", vec!["keymap"]),
            ],
            "a folder underlines what its files' paths have in its name, a file all of its path"
        );
    }
}
