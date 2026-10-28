//! The file lists beside the diff: uncommitted files, or the loaded diff's, flat or as a
//! tree, with what right-clicking them offers.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, MouseButton, MouseDownEvent, Pixels,
    Point, ScrollStrategy, SharedString, Stateful, Toggled, div, img, prelude::*,
};

use super::blame::Blamed;
use super::file_tree::{self, TreeRow};
use super::panes::side_list;
use super::{Detail, FilterFiles, keeps_path, merged, ranked};
use crate::app::AppView;
use crate::backend::{self, Checkout, DiffSource, FileChange, FileDiff, HunkKey, Selected};
use crate::overlays::MenuItem;
use crate::state::{Cut, FileSort, OpKind, Overlay, ProjectId, Rereads, Selection};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::file_icons::file_icon;
use crate::ui::illustration::Illustration;
use crate::ui::{
    Icon, ROW_HEIGHT, checkbox, count_badge, dp, empty_state, faint, fixed, h_flex, icon_button,
    matched, muted, popover, quiet, relative_time, side_row, small_icon, v_flex,
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
    menu: Option<Cut>,
    /// How deep the file sits in a tree; it then shows only its name.
    depth: Option<usize>,
    /// Picked with other files, which its menu and a drag take along.
    picked: bool,
    /// The rows above and below are picked with it: see [`crate::ui::joined`].
    joined: (bool, bool),
    /// The byte ranges of its path the filter found, to underline.
    found: Vec<Range<usize>>,
}

/// How far each level of a file tree steps in.
const STEP: f32 = 12.;

/// The group a file row's hover reaches its checkbox through.
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

/// A tree row's indent, as ui-react's FileListItem draws it: a step per level, each with a
/// guide that joins the rows above and below into one line down its directory, and a
/// directory's `chevron` in a step of its own. A file's icon so sits under its directory's.
fn steps(depth: usize, chevron: Option<gpui::Svg>, theme: &Theme) -> Div {
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

/// A folder row, for what shows on it under the pointer.
const DIR_GROUP: &str = "dir-row";

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
        .child(diff::stats(added, removed, theme))
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
        let (rows, _) = self.file_list("uncommitted-files", source, cx);
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
            theme,
            cx,
        );
        // A clean checkout is good news, as but.dev greets it, here as its overview is beside.
        let clean = !self.filtering && !files.iter().any(|file| shown(&file.path));
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
            .child(v_flex().flex_1().min_h_0().child(rows))
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
                .px(dp(16.))
                .py(dp(10.))
                .into_any_element();
        };
        let (rows, _) = self.file_list("diff-files", FileSource::Diff, cx);
        // Counted as listed: without the files a pending operation is taking out.
        let leaving = diff.key.leaving(self.store.read(cx));
        let paths = (diff.files.iter())
            .map(|f| f.file.path.as_str())
            .filter(|path| !leaving(path));
        let diffs = diff.files.iter().filter(|f| !leaving(&f.file.path));
        let header = self.list_header(paths, diffs, theme, cx);
        v_flex()
            .flex_1()
            .min_h_0()
            .child(header)
            .child(rows)
            .into_any_element()
    }

    /// The files beside the diff, in the popover the files button opens on hover while
    /// they aren't beside it: the uncommitted ones of a checkout's page, else the diff's.
    /// A file picked in it scrolls the diff to it and leaves it open, to pick the next.
    pub(super) fn render_files_popover(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let source = (self.selected_checkout(cx))
            .map_or(FileSource::Diff, |(project, worktree)| {
                FileSource::Uncommitted(project, worktree)
            });
        let (rows, count) = self.file_list("popover-files", source, cx);
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
    fn file_list(&self, id: &'static str, source: FileSource, cx: &Context<Self>) -> (Div, usize) {
        let store = self.store.read(cx);
        let tree = store.prefs.file_tree;
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
                // In the order picked, as the diff beside it has them.
                kept.sort_by(|(_, a), (_, b)| file_tree::file_order(store.prefs.file_sort, a, b));
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
            file_tree::tree_rows(&paths, &self.collapsed_dirs)
        } else {
            (0..paths.len())
                .map(|pos| TreeRow::File { pos, depth: 0 })
                .collect()
        };
        let found = underlined(&rows, &paths, found);
        let scroll = match &source {
            FileSource::Uncommitted(..) => &self.uncommitted_files,
            FileSource::Diff => &self.diff_files,
        };
        // The list follows the cursor to another file, as j and ↓ move it, or to the folded
        // directory the file is in.
        let cursor = self
            .cursor_path()
            .filter(|path| paths.contains(&path.as_str()));
        let cursor_row = cursor.as_deref().and_then(|cursor| {
            rows.iter().position(|row| match row {
                TreeRow::File { pos, .. } => paths.get(*pos) == Some(&cursor),
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
        // Which rows are picked, while some are, so picked files next to each other join up.
        let picks: HashSet<&str> = self.file_picks.iter().map(String::as_str).collect();
        // A picked file's menu is all the picks'.
        let picked = (!picks.is_empty())
            .then(|| self.owned_cut(self.picked_changes(), cx))
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
        let count = rows.len();
        let list = side_list(
            id,
            count,
            scroll,
            "files-scrollbar",
            cx,
            move |this, ix, theme, window, cx| match rows.get(ix)? {
                dir @ TreeRow::Dir { .. } => {
                    let found = found.get(ix)?;
                    this.render_dir_row(dir, cursor_row == Some(ix), &source, found, theme, cx)
                }
                TreeRow::File { pos, depth } => {
                    let lit = |ix: usize| lit.get(ix).copied().unwrap_or(false);
                    let row = FileRow {
                        depth: tree.then_some(*depth),
                        joined: match lit(ix) {
                            true => (ix > 0 && lit(ix - 1), lit(ix + 1)),
                            false => (false, false),
                        },
                        picked: lit(ix),
                        menu: picked.clone().filter(|_| lit(ix)),
                        found: found.get(ix)?.clone(),
                        ..Default::default()
                    };
                    // The current file is solid while the list holds the focus, as the
                    // navigator's selection is while it does.
                    let solid = this.files_focus.is_focused(window);
                    this.render_source_file(&source, *index.get(*pos)?, row, solid, theme, cx)
                }
            },
        );
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
                let file = workspace.uncommitted_in(worktree)?.get(pos)?;
                let whole = Selected::whole(file);
                let extras = FileRow {
                    diff_ix: (self.worktree_diff())
                        .and_then(|diff| diff.files.iter().position(|f| f.file.path == file.path)),
                    checkbox: self.composing.then(|| self.checks.file_check(&file.path)),
                    menu: row.menu.or_else(|| {
                        Some(Cut::Files {
                            project: project.clone(),
                            from: None,
                            worktree: worktree.clone(),
                            files: vec![whole].into(),
                        })
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
                    menu: row.menu.or_else(|| self.diff_cut(pos, None, cx)),
                    ..row
                };
                Some(self.render_file_row(pos, &file.file, extras, solid, theme, cx))
            }
        }
    }

    /// `files` of the loaded diff as they'd be cut, to move or take out of where they are:
    /// a commit listed in the navigator, or a checkout's uncommitted changes. A branch's diff
    /// spans commits, and a commit of the target's history, on the project page, isn't the
    /// workspace's to change: their files aren't cut.
    pub(super) fn owned_cut(&self, files: Vec<Selected>, cx: &App) -> Option<Cut> {
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
        Some(Cut::Files {
            project: key.project.clone(),
            from,
            worktree,
            files: files.into(),
        })
    }

    /// What ⌘X in the diff cuts: the picked lines, or the picked files, or else the hunk under
    /// the cursor, or its file.
    pub fn cursor_cut(&self, cx: &App) -> Option<Cut> {
        // Right after selecting something else, the diff can still be the last one, another
        // project's.
        let project = self.selected_project(cx);
        if self.diff.as_ref().map(|d| &d.key.project) != project.as_ref() {
            return None;
        }
        if self.line_picks.is_some() {
            return self.line_picks_cut(cx);
        }
        if !self.file_picks.is_empty() {
            return self.owned_cut(self.picked_changes(), cx);
        }
        // Not the cursor's file under a checkout's overview, where no row says which it is.
        if !self.shown_files().contains(&self.cursor.file) || self.showing_now(cx) {
            return None;
        }
        let file = self.diff.as_ref()?.files.get(self.cursor.file)?;
        let hunk = (self.cursor.hunk).and_then(|hunk| Some(file.hunks().get(hunk)?.key));
        self.diff_cut(self.cursor.file, hunk.map(|key| vec![key]), cx)
    }

    /// The loaded diff's file at `file`, or `hunks` of it, to drag or cut.
    pub(super) fn diff_cut(
        &self,
        file: usize,
        hunks: Option<Vec<HunkKey>>,
        cx: &App,
    ) -> Option<Cut> {
        let part = self.diff.as_ref()?.files.get(file)?.part(hunks);
        self.owned_cut(vec![part], cx)
    }

    /// The loaded diff's picked files, whole, in its order: not those it doesn't show.
    pub(super) fn picked_changes(&self) -> Vec<Selected> {
        let picks: HashSet<&str> = self.file_picks.iter().map(String::as_str).collect();
        (self.shown_changes().into_iter())
            .filter(|s| picks.contains(s.file.path.as_str()))
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
        let next = match run {
            true => self.file_picks.run(&order, &cursor, &path),
            false => Some(self.file_picks.toggle(&cursor, &path)),
        };
        self.picked_file(next, cx);
    }

    /// Step `delta` files from the cursor, picking the run as ⇧-click does.
    pub(super) fn pick_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self
            .file_step(delta, cx)
            .and_then(|file| self.diff_path(file));
        let (Some(cursor), Some(next)) = (self.cursor_path(), next) else {
            return;
        };
        let next = self.file_picks.run(&self.listed_paths(cx), &cursor, &next);
        self.picked_file(next, cx);
    }

    pub(super) fn pick_all_files(&mut self, cx: &mut Context<Self>) {
        self.file_picks.all(&self.listed_paths(cx));
        self.picked_file(None, cx);
    }

    /// After a pick, bring the cursor to `next`. Picks are of one list at a time, so commits
    /// picked in the navigator go.
    fn picked_file(&mut self, next: Option<String>, cx: &mut Context<Self>) {
        if !self.file_picks.is_empty() && !self.store.read(cx).picked.is_empty() {
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
            self.scroll_to_file(file, cx);
        }
        cx.notify();
    }

    /// Let the picked files go. Whether any were.
    pub fn clear_file_picks(&mut self, cx: &mut Context<Self>) -> bool {
        let had = !self.file_picks.is_empty();
        if had {
            self.file_picks.clear();
            cx.notify();
        }
        had
    }

    /// How many files are picked.
    pub fn picked_files(&self) -> usize {
        self.file_picks.len()
    }

    pub(super) fn diff_path(&self, file: usize) -> Option<String> {
        Some(self.diff.as_ref()?.files.get(file)?.file.path.clone())
    }

    /// The paths of the files the list shows, in its order.
    fn listed_paths(&self, cx: &App) -> Vec<String> {
        (self.listed_files(cx).into_iter())
            .filter_map(|file| self.diff_path(file))
            .collect()
    }

    /// The files under the folder `dir` in `source`, to act on together.
    fn folder_payload(&self, source: &FileSource, dir: &str, cx: &App) -> Option<Cut> {
        let prefix = format!("{dir}/");
        // Only what the folder shows: not files the filter or a pending operation hides.
        let terms = self.filter_terms(cx);
        let under = |path: &str| path.starts_with(&prefix) && keeps_path(&terms, path);
        let store = self.store.read(cx);
        let cut = match source {
            FileSource::Uncommitted(project, worktree) => Cut::Files {
                project: project.clone(),
                from: None,
                worktree: worktree.clone(),
                files: (store.workspace(project)?.uncommitted_in(worktree)?.iter())
                    .filter(|f| under(&f.path) && !store.hides_file(project, worktree, &f.path))
                    .map(Selected::whole)
                    .collect(),
            },
            FileSource::Diff => {
                let diff = self.diff.as_ref()?;
                let leaving = diff.key.leaving(store);
                let files = (diff.files.iter())
                    .map(|f| &f.file)
                    .filter(|f| under(&f.path) && !leaving(&f.path))
                    .map(Selected::whole)
                    .collect();
                self.owned_cut(files, cx)?
            }
        };
        matches!(&cut, Cut::Files { files, .. } if !files.is_empty()).then_some(cut)
    }

    /// A tree's folder row, with the byte ranges of its name the filter `found`.
    fn render_dir_row(
        &self,
        row: &TreeRow,
        holds_cursor: bool,
        source: &FileSource,
        found: &[Range<usize>],
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
        // The lines its files add and remove, shown on hover: always there, they'd crowd the
        // names. Summed from the diff's files, as each file's own counts are.
        let under = format!("{path}/");
        let (added, removed) = diff::total_stats(
            (self.diff.iter())
                .flat_map(|diff| diff.files.iter())
                .filter(|file| file.file.path.starts_with(&under)),
        );
        let row = file_row(
            ElementId::Name(format!("dir-{path}").into()),
            holds_cursor,
            false,
            theme,
        )
        .group(DIR_GROUP)
        .child(steps(
            depth,
            Some(
                small_icon(if collapsed {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                })
                .text_color(theme.text_3),
            ),
            theme,
        ))
        // In colour and at its own 17 by 14 points, as ui-react's FolderIcon draws it, in the
        // box a file's icon has, which it runs a little past.
        .child(
            div()
                .flex_none()
                .size(dp(14.))
                .child(img(Icon::FolderFill.path()).w(dp(17.)).h(dp(14.))),
        )
        // Cut in the middle: a chain of folders, `ios/App/Resources/de.lproj`, ends in the one
        // that tells it from its siblings.
        .child(
            (div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap())
            .text_ellipsis_middle()
            .child(matched(name, found.iter().cloned())),
        )
        .when(added + removed > 0, |el| {
            el.child(
                diff::stats(added, removed, theme)
                    .flex_none()
                    .opacity(0.)
                    .group_hover(DIR_GROUP, |style| style.opacity(1.)),
            )
        })
        .when(collapsed, |el| el.child(faint(files.to_string(), theme)))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener({
                let (source, path) = (source.clone(), path.clone());
                move |this, event: &MouseDownEvent, _, cx| {
                    let payload = this.folder_payload(&source, &path, cx);
                    let at = At::Folder(path.clone());
                    this.changes_menu(payload, at, None, event.position, cx)
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

    /// A file list's header: how many `paths` it has, the lines `diffs` add and remove, and
    /// ways to filter it or show it as a tree. While the filter is open: the filter, how many
    /// of `paths` it keeps, and a way to close it.
    fn list_header<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        diffs: impl Iterator<Item = &'a FileDiff>,
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
                    .on_click(cx.listener(|this, _, window, cx| this.open_filter(window, cx))),
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

    /// The file list's ⋯ menu, as but.dev's: the files as a list or as a tree, by name or
    /// the last written first. Files with no write time, as a commit's, go by name either way.
    fn view_items(&self, cx: &App) -> Vec<MenuItem> {
        let prefs = &self.store.read(cx).prefs;
        let (tree, sort) = (prefs.file_tree, prefs.file_sort);
        let show = |label: &str, as_tree: bool| {
            MenuItem::new(label.to_owned(), move |this, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.file_tree = as_tree, cx)
                })
            })
            .ticked(tree == as_tree)
        };
        let by = |label: &str, by: FileSort| {
            MenuItem::new(label.to_owned(), move |this, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.file_sort = by, cx)
                })
            })
            .ticked(sort == by)
        };
        vec![
            show("As list", false).under("View"),
            show("As tree", true),
            by("Name", FileSort::Name).separated().under("Sort by"),
            by("Last modified", FileSort::Modified),
        ]
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
        let stats = (diff_ix.and_then(|ix| self.diff.as_ref()?.files.get(ix))).map(FileDiff::stats);
        // None while a checkout's overview shows: no file is picked then.
        let current = diff_ix == Some(self.cursor.file) && !self.showing_now(cx);
        let picked = extras.picked;
        let solid = (current || picked) && solid;
        // On the solid ground, as ui-react's FileList inverts a selected row's contents.
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
        let mark = div().map(|el| match file.conflicted {
            true => el.child(small_icon(Icon::Warning).text_color(theme.warn)),
            false => el
                .text_size(dp(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(diff::change_color(file.kind, theme))
                .child(file.kind.letter()),
        });
        let end = div()
            .flex_none()
            .w(dp(22.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(mark);
        let mut row = file_row(("file", ix), current || picked, solid, theme)
            .map(|el| crate::ui::joined(el, extras.joined, solid, theme))
            .aria_label(file.path.clone())
            .aria_description(file.kind.word())
            .group(ROW_GROUP)
            .children(
                extras
                    .depth
                    .filter(|&d| d > 0)
                    .map(|depth| steps(depth, None, theme)),
            )
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
            .when_some(stats, |el, (added, removed)| {
                el.child(diff::stats(added, removed, theme))
            })
            // When it last changed, faint, for watching what an agent touches.
            .children(
                (!file.conflicted)
                    .then_some(file.modified)
                    .flatten()
                    .map(|ms| quiet(faint(relative_time(ms / 1000), theme), solid, theme)),
            )
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
                        this.scroll_to_file(diff_ix, cx)
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
            Some(Cut::Files {
                project,
                from,
                worktree,
                ..
            }) if !picked && extras.checkbox == Some(Toggled::Mixed) => {
                let loaded = self.loaded_file(&file.path);
                (self.selected_change(file, loaded)).map(|change| Cut::Files {
                    project: project.clone(),
                    from: *from,
                    worktree: worktree.clone(),
                    files: vec![change].into(),
                })
            }
            menu => menu.clone(),
        };
        if let Some(dragged) = dragged {
            let label: SharedString = dragged.describe(self.store.read(cx)).into();
            row = row.on_drag(
                dragged,
                self.drag_preview(label, false, cx.entity().downgrade()),
            );
        }
        let (menu, path) = (extras.menu, file.path.clone());
        row.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                // A file not picked is the menu's alone, selected as a click selects it, so
                // the menu doesn't look like the selected file's.
                if !picked {
                    this.clear_file_picks(cx);
                    if let Some(diff_ix) = diff_ix {
                        this.scroll_to_file(diff_ix, cx);
                    }
                }
                let at = At::File(path.clone(), None);
                this.changes_menu(menu.clone(), at, None, event.position, cx)
            }),
        )
        .into_any_element()
    }

    /// What right-clicking files, a folder, lines or a hunk offers: what can be done to the
    /// changes in `payload`, when they're the workspace's to change, or going to the last
    /// commit of `blamed`, then opening, copying or revealing the path `at`. A branch outside the
    /// workspace, or a commit of the target, has only the last.
    pub(super) fn changes_menu(
        &mut self,
        payload: Option<Cut>,
        at: At,
        blamed: Option<Blamed>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(
            position,
            move |this, cx| this.changes_items(payload, at, blamed, position, cx),
            cx,
        );
    }

    /// What M offers while the file list or the diff has the keys: the menu of the file under
    /// the cursor, as right-clicking it opens. `None` elsewhere.
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
        let payload = match self.file_picks.contains(&path) {
            true => self.owned_cut(self.picked_changes(), cx),
            false => self.diff_cut(self.cursor.file, None, cx),
        };
        let at = At::File(path, None);
        Some(self.changes_items(payload, at, None, window.mouse_position(), cx))
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
        let path = self.shown_cursor_path(cx)?;
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
        payload: Option<Cut>,
        at: At,
        blamed: Option<Blamed>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> Vec<MenuItem> {
        let (project, worktree) = match &payload {
            Some(Cut::Files {
                project, worktree, ..
            }) => (project.clone(), worktree.clone()),
            _ => match &self.diff {
                Some(diff) => (diff.key.project.clone(), diff.key.files_checkout()),
                None => return Vec::new(),
            },
        };
        let mut items = payload.map_or_else(Vec::new, |payload| self.change_items(payload, cx));
        if let (Some(blamed), At::File(path, _)) = (blamed, &at) {
            items.extend(self.last_commit_item(path, blamed, cx));
        }
        let mut paths = self
            .path_items(&project, worktree, at, position, cx)
            .into_iter();
        // Apart from what changes them.
        if let Some(first) = (!items.is_empty()).then(|| paths.next()).flatten() {
            items.push(first.separated());
        }
        items.extend(paths);
        items
    }

    /// What can be done to the changes in `payload` from a menu: cutting them, and moving,
    /// taking out or discarding them.
    fn change_items(&self, payload: Cut, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let Cut::Files {
            project,
            from,
            worktree,
            files,
        } = payload.clone()
        else {
            return Vec::new();
        };
        let hunk = files.iter().any(|f| f.hunks.is_some());
        let what = match (self.takes_lines(&files), hunk, files.len()) {
            (true, ..) => "lines",
            (false, true, _) => "hunk",
            (false, false, 1) => "file",
            (false, false, _) => "files",
        };
        let cut = payload.clone();
        let mut items = vec![MenuItem::keyed(
            "Cut",
            &crate::app::CutSelection,
            cx,
            move |this: &mut AppView, _, cx| this.cut(cut.clone(), cx),
        )];
        match from {
            None => {
                let conflicted: Vec<bstr::BString> = (files.iter())
                    .filter(|f| f.file.conflicted)
                    .map(|f| f.file.change.path.clone())
                    .collect();
                if !conflicted.is_empty() {
                    let (project, worktree) = (project.clone(), worktree.clone());
                    // Undo can't bring back a linked worktree, or a commit checked out for
                    // editing.
                    let undoable = !worktree.is_linked()
                        && (self.store.read(cx).workspace(&project))
                            .is_some_and(|ws| ws.editing.is_none());
                    items.insert(
                        0,
                        MenuItem::new("Mark resolved", move |this, _, cx| {
                            let (files, worktree) = (conflicted.clone(), worktree.clone());
                            this.store.update(cx, |store, cx| {
                                store.run(
                                    &project,
                                    "Mark resolved",
                                    Overlay {
                                        kind: OpKind::MarkResolved,
                                        rereads: Rereads::Files,
                                        files_in: worktree.clone(),
                                        ..Overlay::default()
                                    },
                                    move |path, _| {
                                        backend::mark_resolved(path, worktree.linked(), files)
                                    },
                                    move |store, (), cx| {
                                        store.snack_after(undoable, "Marked resolved", cx)
                                    },
                                    cx,
                                )
                            })
                        }),
                    );
                }
                let discard = crate::edits::discard_label(
                    match what {
                        "lines" | "hunk" => what,
                        _ => "changes",
                    },
                    worktree.is_linked(),
                );
                items.push(
                    MenuItem::keyed(discard, &super::DiscardFile, cx, move |this, window, cx| {
                        let (project, worktree) = (project.clone(), worktree.clone());
                        this.discard(project, worktree, files.to_vec(), window, cx)
                    })
                    .danger(),
                );
            }
            Some(commit) => {
                // A linked worktree's commit uncommits into that worktree.
                let uncommitted = match (self.store.read(cx))
                    .worktree_for(&Selection::Commit(project.clone(), commit))
                {
                    Some(worktree) => Selection::Worktree(project.clone(), worktree.name.clone()),
                    None => Selection::Uncommitted(project.clone()),
                };
                let (split_project, split_files) = (project.clone(), files.clone());
                let (drop_project, drop_files) = (project, files.clone());
                items.push(MenuItem::new(
                    format!("Uncommit {what}"),
                    move |this, window, cx| {
                        let into = crate::transfer::Place::Into;
                        this.transfer(&payload, &uncommitted, into, window, cx);
                    },
                ));
                // All of it would leave the message on an empty commit.
                if !self.takes_all(&files) {
                    items.push(MenuItem::new(
                        "Split into a new commit",
                        move |this, _, cx| {
                            this.split_off(split_project.clone(), commit, split_files.clone(), cx)
                        },
                    ));
                }
                items.push(
                    MenuItem::new(
                        format!("Discard {what} from commit"),
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
                );
            }
        }
        items
    }

    /// Opening, copying or revealing the path `at` of `project`'s `checkout`. What isn't on disk,
    /// as a deleted file or one only a branch outside the workspace has, can only have its path
    /// copied.
    fn path_items(
        &self,
        project: &ProjectId,
        checkout: Checkout,
        at: At,
        position: Point<Pixels>,
        cx: &App,
    ) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let Some(root) = self.checkout_root(project, &checkout, cx) else {
            return Vec::new();
        };
        let (relative, line) = match at {
            At::File(path, line) => (path, Some(line)),
            At::Folder(path) => (path, None),
        };
        // Git's paths use `/`; Windows' own `\`, which "Copy full path" and Explorer expect.
        let absolute = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let full = absolute.display().to_string();
        #[expect(
            clippy::disallowed_methods,
            reason = "one check as the menu opens, in a checkout the app is watching"
        )]
        let on_disk = absolute.exists();
        let mut items = Vec::new();
        if let (Some(line), true) = (line, on_disk) {
            let (open_project, open_file) = (project.clone(), relative.clone());
            let label = crate::open_in::editor_label(store);
            items.push(MenuItem {
                key: crate::ui::shortcut(&crate::open_in::OpenInEditor, cx),
                ..MenuItem::new(label, move |this, _, cx| {
                    let file = Some(open_file.clone());
                    let checkout = checkout.clone();
                    this.open_in_editor(&open_project, checkout, file, line, position, cx)
                })
            });
        }
        items.extend([
            MenuItem::keyed(
                "Copy path",
                &crate::app::CopySelection,
                cx,
                move |this, _, cx| this.copy_text(relative.clone(), cx),
            ),
            MenuItem::new("Copy full path", move |this, _, cx| {
                this.copy_text(full.clone(), cx)
            }),
        ]);
        if on_disk {
            items.push(MenuItem::new(crate::ui::REVEAL, move |_, _, cx| {
                cx.reveal_path(&absolute)
            }));
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::underlined;
    use crate::detail::file_tree::{TreeRow, tree_rows};
    use crate::detail::path_match;

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
