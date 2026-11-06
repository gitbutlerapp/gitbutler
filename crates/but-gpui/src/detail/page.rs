//! What each kind of page keeps of how it's being looked at, apart from the detail drawing it,
//! and the pages kept while nothing draws them.

use super::brief::state::BriefState;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use std::sync::Arc;

use gpui::{App, Entity, ListOffset, Pixels, Point, ScrollHandle};

use super::branches::Branches;
use super::graph::GraphState;
use super::history::{Group, History};
use super::lines::LinePicks;
use super::review_edit::EditLeft;
use super::slots::SlotPick;
use super::upstream::Upstream;
use super::{Cursor, KeyPane, LoadedDiff, Stop, follow_hunk, hunk_keys};
use crate::backend::{FileDiff, FullName, HunkKey};
use crate::state::{Picks, ProjectId, ProjectTab, RowKey, Section, Selection, Store, Tile};
use crate::ui::diff::DiffRow;

/// How a page's files are being looked at.
pub(super) struct FilesState {
    /// The file list beside the diff is hidden, for more room.
    pub(super) files_hidden: bool,
    /// The files between a list page's list and the diff are hidden, as they are at first: the
    /// page's own list already takes the room the file list has elsewhere.
    pub(super) beside_files_hidden: bool,
    /// Directories folded shut in tree-shaped file lists, by path.
    pub(super) collapsed_dirs: HashSet<String>,
    /// Files folded by hand.
    pub(super) folded: HashSet<String>,
    /// Where the diff was left, for when it's next drawn.
    pub(super) place: Option<Place>,
    /// Files picked in the file list, by path, to act on together.
    pub(super) file_picks: Picks<String>,
    /// Lines picked by their numbers, in one file, to act on together.
    pub(super) line_picks: Option<LinePicks>,
    /// SVG files shown as text rather than drawn, by path.
    pub(super) svg_text: HashSet<String>,
    /// What the file list was filtered by as the page was left, with the filter open.
    pub(super) filter: Option<String>,
    /// What the find bar was finding as the page was left, and which match it was on.
    pub(super) find: Option<(String, usize)>,
    /// What it shows has its message open past the title.
    pub(super) message_open: bool,
    /// What was written of the commit's message being edited as the page's detail went.
    pub(super) reworded: Option<String>,
    /// The pane the keys last left, which they go back to where several are on that side.
    pub(super) last_pane: Option<KeyPane>,
    /// The tree's folder the file list's keys stepped onto, with where the diff's cursor was
    /// then: it's the current one only while the cursor stays there.
    pub(super) cursor_dir: Option<(String, Cursor)>,
}

impl Default for FilesState {
    fn default() -> FilesState {
        FilesState {
            files_hidden: false,
            beside_files_hidden: true,
            collapsed_dirs: HashSet::new(),
            folded: HashSet::new(),
            place: None,
            file_picks: Picks::default(),
            line_picks: None,
            svg_text: HashSet::new(),
            filter: None,
            find: None,
            message_open: false,
            reworded: None,
            last_pane: None,
            cursor_dir: None,
        }
    }
}

/// A file of a diff by what it is, as other files and hunks coming and going move its index.
struct FilePlace {
    path: String,
    hunks: Vec<HunkKey>,
}

impl FilePlace {
    fn of(file: &FileDiff) -> FilePlace {
        FilePlace {
            path: file.file.path.clone(),
            hunks: hunk_keys(file),
        }
    }

    /// Where it is in `files`.
    fn find<'a>(&self, files: &'a [FileDiff]) -> Option<(usize, &'a FileDiff)> {
        (files.iter().enumerate()).find(|(_, file)| file.file.path == self.path)
    }
}

/// Where the cursor is in a diff, to find again in the files of another reading of it.
pub(super) struct CursorPlace {
    cursor: Cursor,
    file: Option<FilePlace>,
}

impl CursorPlace {
    pub(super) fn of(cursor: Cursor, files: &[FileDiff]) -> CursorPlace {
        CursorPlace {
            cursor,
            file: files.get(cursor.file).map(FilePlace::of),
        }
    }

    /// Where it sits in `files`, so a reload never leaves it pointing at a different file or a
    /// hunk that's gone.
    pub(super) fn find(&self, files: &[FileDiff]) -> Cursor {
        if self.cursor.at == Stop::Message {
            return self.cursor;
        }
        let Some(old) = &self.file else {
            return Cursor::default();
        };
        // The file went, say discarded: stay where it was, on what moved up into its
        // place, rather than jumping to the first file the next key would act on.
        let Some((file, new)) = old.find(files) else {
            return Cursor::on_file(self.cursor.file.min(files.len().saturating_sub(1)));
        };
        let near =
            (self.cursor.at.near()).and_then(|ix| follow_hunk(&old.hunks, ix, &hunk_keys(new)));
        let at = match (self.cursor.at, near) {
            (Stop::Gap(_), Some(hunk)) => Stop::Gap(hunk),
            (_, Some(hunk)) => Stop::Hunk(hunk),
            (_, None) => Stop::File,
        };
        Cursor { file, at }
    }
}

/// A row of a diff, to find again among the rows of another reading of it.
pub(super) struct RowPlace {
    row: DiffRow,
    file: Option<FilePlace>,
    /// How many gaps it's below the row asked for, as gaps all look alike and a gap is found by
    /// the row after it.
    gaps: usize,
}

impl RowPlace {
    pub(super) fn of(diff: &LoadedDiff, at: usize) -> Option<RowPlace> {
        let gaps = (diff.rows.iter().skip(at))
            .take_while(|row| **row == DiffRow::Gap)
            .count();
        let row = *diff.rows.get(at + gaps)?;
        let file = match row.file() {
            Some(file) => Some(FilePlace::of(diff.files.get(file)?)),
            None => None,
        };
        Some(RowPlace { row, file, gaps })
    }

    /// The row in `files`: by its file's path, as a reload can add or drop files before it, and
    /// its hunk as the cursor follows one, which hunks an agent adds or drops elsewhere in the
    /// file don't move.
    fn in_files(&self, files: &[FileDiff]) -> Option<DiffRow> {
        let Some(old) = &self.file else {
            return Some(self.row);
        };
        let (now, new) = old.find(files)?;
        let mut row = self.row.in_file(now);
        if let Some(hunk) = row.hunk_mut()
            && let Some(ix) = follow_hunk(&old.hunks, *hunk, &hunk_keys(new))
        {
            *hunk = ix;
        }
        Some(row)
    }

    /// Where it is in `rows` of `files`: the same row, and true, or its file's header when that
    /// row went away, and false.
    pub(super) fn find(&self, rows: &[DiffRow], files: &[FileDiff]) -> Option<(usize, bool)> {
        let row = self.in_files(files)?;
        let found =
            (rows.iter().position(|r| r.same_place(row))).and_then(|ix| ix.checked_sub(self.gaps));
        match found {
            Some(ix) => Some((ix, true)),
            None => (row.file())
                .and_then(|file| rows.iter().position(|r| *r == DiffRow::File(file)))
                .map(|ix| (ix, false)),
        }
    }
}

/// Where the view goes in `rows` of `files` to keep showing `row`, which was at `top`: the same
/// row, or its file's header when that row went away. Rows of one file `alone` that's not the
/// one at the top start at their own top.
pub(super) fn offset_in(
    top: ListOffset,
    row: Option<&RowPlace>,
    rows: &[DiffRow],
    files: &[FileDiff],
    alone: bool,
) -> ListOffset {
    // At the very top, what's added above shows: a file a merge left conflicted lists first.
    if top.item_ix == 0 && top.offset_in_item == Pixels::ZERO {
        return top;
    }
    match row.and_then(|row| row.find(rows, files)) {
        Some((item_ix, true)) => ListOffset { item_ix, ..top },
        Some((item_ix, false)) => ListOffset {
            item_ix,
            offset_in_item: Pixels::ZERO,
        },
        None if alone => ListOffset::default(),
        None => ListOffset {
            item_ix: top.item_ix,
            offset_in_item: Pixels::ZERO,
        },
    }
}

/// Where a diff was left: its cursor and scroll by what they were on, as its files can change
/// before it's drawn again.
pub(super) struct Place {
    pub(super) cursor: CursorPlace,
    top: ListOffset,
    row: Option<RowPlace>,
    /// The files shown whole, by path.
    pub(super) whole: Vec<String>,
    /// How far the lines of files scrolled sideways are, by path: the old side and the new.
    pub(super) sideways: Vec<(String, [Point<Pixels>; 2])>,
}

impl Place {
    pub(super) fn of(
        diff: &LoadedDiff,
        cursor: Cursor,
        top: ListOffset,
        whole: Vec<String>,
        sideways: Vec<(String, [Point<Pixels>; 2])>,
    ) -> Place {
        Place {
            cursor: CursorPlace::of(cursor, &diff.files),
            top,
            row: RowPlace::of(diff, top.item_ix),
            whole,
            sideways,
        }
    }

    /// Where the view goes to show what it was on, in `rows` of `files`.
    pub(super) fn offset_in(
        &self,
        rows: &[DiffRow],
        files: &[FileDiff],
        alone: bool,
    ) -> ListOffset {
        offset_in(self.top, self.row.as_ref(), rows, files, alone)
    }
}

/// One of a page's tabs, each showing in place of the others.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    strum::EnumIter,
    strum::IntoStaticStr,
    strum::EnumString,
)]
// As a `but://open` link names the tab (`tab=`), so renaming one renames its links: see
// `crate::links`.
#[strum(serialize_all = "kebab-case")]
pub(crate) enum PageTab {
    #[default]
    Changes,
    Review,
    Brief,
}

/// How a pull request or a brief is being looked at, on the pages that can show one.
#[derive(Default)]
pub(super) struct ReviewState {
    /// The tab picked. Without what it shows, a page shows its changes.
    pub(super) tab: PageTab,
    pub(super) brief: BriefState,
    /// Whether each pull request's checks are listed, by project and number, as the reader left
    /// them or as they opened by themselves on a failure. Others are folded. Rendering records
    /// the opening, so it's a cell.
    pub(super) checks_open: RefCell<HashMap<(ProjectId, u64), bool>>,
    /// The pull request threads shown open while resolved, or folded while not, by id.
    pub(super) threads_flipped: HashSet<String>,
    /// How far the tab was scrolled as the page was left, for when its rows are next drawn, and
    /// its side panel.
    pub(super) top: Cell<Option<ListOffset>>,
    pub(super) side: Point<Pixels>,
    /// The edit of the pull request's title and description the page was left with.
    pub(super) edit: Option<EditLeft>,
}

/// How a list page's list is being looked at.
pub(super) struct SideState {
    /// What its field filters it by, as the page was left.
    pub(super) filter: String,
    /// How far it was scrolled as the page was left, for when its rows are next drawn.
    pub(super) top: Cell<Option<ListOffset>>,
    /// The People card lists everyone, scrolling within it, rather than the busiest few.
    pub(super) people_open: bool,
}

impl Default for SideState {
    /// At the top of its list, which is shared with the pages of other projects.
    fn default() -> SideState {
        SideState {
            filter: String::new(),
            top: Cell::new(Some(ListOffset::default())),
            people_open: false,
        }
    }
}

/// How a checkout's overview is being looked at.
#[derive(Default)]
pub(super) struct CheckoutState {
    /// The hot file picked, whose commits are all Commits lists.
    pub(super) file: Option<Arc<str>>,
    /// The quarters of an hour picked off the activity, by when they begin, which the rest of
    /// the overview is narrowed to; none for all it shows.
    pub(super) span: SlotPick,
    /// Hot files' and the commits', each scrolling on its own.
    pub(super) hot_scroll: ScrollHandle,
    pub(super) commits_scroll: ScrollHandle,
}

/// The state of the page a detail shows, by what kind of page it is, so what a page doesn't have
/// it can't be asked for. A list page's `list` is what its detail read, held only while a pinned
/// tile's detail is gone: a detail keeps it by project, where other pages read it too.
pub(super) enum PageState {
    /// A worktree's uncommitted changes. The commit being written on them is the home tile's
    /// detail's, the one that composes, by checkout.
    Changes {
        files: FilesState,
    },
    /// A commit, a branch, a stack, or what a pull would bring in, by its diff. A branch and a
    /// commit only a remote has can show a pull request, and a branch a brief.
    Diff {
        files: FilesState,
        review: Option<ReviewState>,
    },
    /// The branches outside the workspace, and the diff and pull request of the one opened.
    Branches {
        files: FilesState,
        review: ReviewState,
        side: SideState,
        list: Option<Branches>,
    },
    /// The operation log, and the diff of the operation picked, with the kinds of operation it
    /// leaves out.
    History {
        files: FilesState,
        side: SideState,
        hidden: HashSet<Group>,
        list: Option<History>,
    },
    /// The project's page, and the diff and pull request of what's picked on it.
    Project {
        files: FilesState,
        review: ReviewState,
        side: SideState,
        list: Option<Upstream>,
    },
    Graph(GraphState),
    /// A page that sums up a checkout or a section, with no column beside it.
    Overview(CheckoutState),
    /// Nothing selected, or a terminal, which is drawn over the detail and keeps its own state.
    Blank,
}

/// Whether the page `selection` shows has tabs past its changes, a pull request's and a brief's,
/// as a `but://open` link can open it on.
pub(crate) fn has_tabs(selection: &Selection) -> bool {
    (PageState::of(Some(selection), ProjectTab::default()).review()).is_some()
}

/// Whether the page `selection` shows is its changes' diff, as a `but://open` link can open it at
/// one of their files.
pub(crate) fn shows_its_diff(selection: &Selection) -> bool {
    matches!(
        PageState::of(Some(selection), ProjectTab::default()),
        PageState::Changes { .. } | PageState::Diff { .. }
    )
}

impl PageState {
    /// The state of the page `selection` shows, as it opens.
    pub(super) fn of(selection: Option<&Selection>, tab: ProjectTab) -> PageState {
        let Some(selection) = selection else {
            return PageState::Blank;
        };
        match selection {
            Selection::Uncommitted(..) => PageState::Changes {
                files: FilesState::default(),
            },
            Selection::Branch(..) | Selection::Upstream(..) => PageState::Diff {
                files: FilesState::default(),
                review: Some(ReviewState::default()),
            },
            Selection::Commit(..)
            | Selection::Stack(..)
            | Selection::Mesh(..)
            | Selection::Section(_, Section::Upstream(_)) => PageState::Diff {
                files: FilesState::default(),
                review: None,
            },
            Selection::Project(_) => match tab {
                ProjectTab::Overview => PageState::Project {
                    files: FilesState::default(),
                    review: ReviewState::default(),
                    side: SideState::default(),
                    list: None,
                },
                ProjectTab::Branches => PageState::Branches {
                    files: FilesState::default(),
                    review: ReviewState::default(),
                    side: SideState::default(),
                    list: None,
                },
                ProjectTab::History => PageState::History {
                    files: FilesState::default(),
                    side: SideState::default(),
                    hidden: HashSet::new(),
                    list: None,
                },
                ProjectTab::Graph => PageState::Graph(GraphState::default()),
            },
            Selection::Checkout(..) | Selection::Section(_, Section::Inactive | Section::Mesh) => {
                PageState::Overview(CheckoutState::default())
            }
            Selection::Terminal(..) => PageState::Blank,
        }
    }

    /// The tab it is of a project's page, on one.
    pub(super) fn project_tab(&self) -> Option<ProjectTab> {
        match self {
            PageState::Project { .. } => Some(ProjectTab::Overview),
            PageState::Branches { .. } => Some(ProjectTab::Branches),
            PageState::History { .. } => Some(ProjectTab::History),
            PageState::Graph(_) => Some(ProjectTab::Graph),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Overview(_)
            | PageState::Blank => None,
        }
    }

    /// How its files are being looked at, on a page with files.
    pub(super) fn files(&self) -> Option<&FilesState> {
        match self {
            PageState::Changes { files }
            | PageState::Diff { files, .. }
            | PageState::Branches { files, .. }
            | PageState::History { files, .. }
            | PageState::Project { files, .. } => Some(files),
            PageState::Graph(_) | PageState::Overview(_) | PageState::Blank => None,
        }
    }

    /// How its pull request is being looked at, on a page that can show one.
    pub(super) fn review(&self) -> Option<&ReviewState> {
        match self {
            PageState::Diff { review, .. } => review.as_ref(),
            PageState::Branches { review, .. } | PageState::Project { review, .. } => Some(review),
            PageState::Changes { .. }
            | PageState::History { .. }
            | PageState::Graph(_)
            | PageState::Overview(_)
            | PageState::Blank => None,
        }
    }

    /// As [`PageState::review`], to change it.
    pub(super) fn review_mut(&mut self) -> Option<&mut ReviewState> {
        match self {
            PageState::Diff { review, .. } => review.as_mut(),
            PageState::Branches { review, .. } | PageState::Project { review, .. } => Some(review),
            PageState::Changes { .. }
            | PageState::History { .. }
            | PageState::Graph(_)
            | PageState::Overview(_)
            | PageState::Blank => None,
        }
    }

    /// How its brief is being looked at, on a page that can show one.
    pub(super) fn brief(&self) -> Option<&BriefState> {
        Some(&self.review()?.brief)
    }

    /// As [`PageState::brief`], to change it.
    pub(super) fn brief_mut(&mut self) -> Option<&mut BriefState> {
        Some(&mut self.review_mut()?.brief)
    }

    /// How its list is being looked at, on a list page.
    pub(super) fn side(&self) -> Option<&SideState> {
        match self {
            PageState::Branches { side, .. }
            | PageState::History { side, .. }
            | PageState::Project { side, .. } => Some(side),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Graph(_)
            | PageState::Overview(_)
            | PageState::Blank => None,
        }
    }

    /// As [`PageState::side`], to change it.
    pub(super) fn side_mut(&mut self) -> Option<&mut SideState> {
        match self {
            PageState::Branches { side, .. }
            | PageState::History { side, .. }
            | PageState::Project { side, .. } => Some(side),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Graph(_)
            | PageState::Overview(_)
            | PageState::Blank => None,
        }
    }

    /// How a checkout's overview is being looked at, on an overview.
    pub(super) fn overview(&self) -> Option<&CheckoutState> {
        match self {
            PageState::Overview(overview) => Some(overview),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Branches { .. }
            | PageState::History { .. }
            | PageState::Project { .. }
            | PageState::Graph(_)
            | PageState::Blank => None,
        }
    }

    /// As [`PageState::overview`], to change it.
    pub(super) fn overview_mut(&mut self) -> Option<&mut CheckoutState> {
        match self {
            PageState::Overview(overview) => Some(overview),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Branches { .. }
            | PageState::History { .. }
            | PageState::Project { .. }
            | PageState::Graph(_)
            | PageState::Blank => None,
        }
    }

    /// Whether kind of operation `group` is left out of the list, on the History page.
    pub(super) fn hides_operations(&self, group: &Group) -> bool {
        matches!(self, PageState::History { hidden, .. } if hidden.contains(group))
    }

    /// How many kinds of operation the list leaves out, on the History page.
    pub(super) fn operations_hidden(&self) -> usize {
        match self {
            PageState::History { hidden, .. } => hidden.len(),
            PageState::Changes { .. }
            | PageState::Diff { .. }
            | PageState::Branches { .. }
            | PageState::Project { .. }
            | PageState::Graph(_)
            | PageState::Overview(_)
            | PageState::Blank => 0,
        }
    }

    /// Leave kind of operation `group` out of the list or put it back, on the History page.
    pub(super) fn toggle_operations(&mut self, group: Group) {
        if let PageState::History { hidden, .. } = self
            && !hidden.remove(&group)
        {
            hidden.insert(group);
        }
    }

    /// Whether it shows its pull request in place of its changes.
    pub(super) fn review_tab(&self) -> bool {
        self.review()
            .is_some_and(|review| review.tab == PageTab::Review)
    }

    /// Whether it shows its brief in place of its changes.
    pub(super) fn brief_tab(&self) -> bool {
        self.review()
            .is_some_and(|review| review.tab == PageTab::Brief)
    }

    /// Whether the message of what it shows is open, on a page with files.
    pub(super) fn message_open(&self) -> bool {
        self.files().is_some_and(|files| files.message_open)
    }

    /// The lines picked, on a page with files and while some are.
    pub(super) fn line_picks(&self) -> Option<&LinePicks> {
        self.files()?.line_picks.as_ref()
    }

    /// Whether files are picked, on a page with files.
    pub(super) fn has_file_picks(&self) -> bool {
        self.files()
            .is_some_and(|files| !files.file_picks.is_empty())
    }

    /// As [`PageState::files`], to change it. On the page rather than the detail, so holding
    /// it leaves the detail's other fields free.
    pub(super) fn files_mut(&mut self) -> Option<&mut FilesState> {
        match self {
            PageState::Changes { files }
            | PageState::Diff { files, .. }
            | PageState::Branches { files, .. }
            | PageState::History { files, .. }
            | PageState::Project { files, .. } => Some(files),
            PageState::Graph(_) | PageState::Overview(_) | PageState::Blank => None,
        }
    }
}

/// Whose a page is: the row whose tiling it's in, its tile there and, of a project's page, the
/// tab, so the same thing shown in two places is looked at apart. The home tile's row is what
/// the navigator selects.
pub type PageKey = (RowKey, Tile, ProjectTab);

/// The key of the page a detail shows on `tab`: `pinned`'s tile, by the row of its tiling and
/// the row it's pinned to, or else the selection's home tile. Nothing shown has none.
pub(super) fn page_key(
    store: &Store,
    pinned: Option<&(RowKey, RowKey)>,
    shows: Option<&Selection>,
    tab: ProjectTab,
) -> Option<PageKey> {
    let shows = shows?;
    Some(match pinned {
        Some((tiling, row)) => (tiling.clone(), Tile::Pinned(row.clone()), tab),
        None => (store.row_key(shows), Tile::Home, tab),
    })
}

/// The tab a detail shows of the page it's coming to: as it was left, or for a tile pinned on
/// one, that one.
pub(super) fn tab_to_show(
    kept: &Entity<KeptPages>,
    store: &Store,
    pinned: Option<&(RowKey, RowKey)>,
    shows: Option<&Selection>,
    cx: &App,
) -> ProjectTab {
    let key = page_key(store, pinned, shows, ProjectTab::default());
    let left = key.and_then(|(row, tile, _)| kept.read(cx).tabs.get(&(row, tile)).copied());
    let pinned_on = pinned.and_then(|(_, row)| match row {
        RowKey::Tab(_, tab) => Some(*tab),
        RowKey::Commit(..) | RowKey::Row(_) | RowKey::Changes(_) => None,
    });
    left.or(pinned_on).unwrap_or_default()
}

/// The page of `key` showing `shows`: as it was left, or as it opens.
pub(super) fn open_page(
    kept: &Entity<KeptPages>,
    key: Option<&PageKey>,
    shows: Option<&Selection>,
    cx: &mut App,
) -> PageState {
    let tab = key.map_or(ProjectTab::default(), |key| key.2);
    (key.and_then(|key| kept.update(cx, |kept, _| kept.take(key)))).unwrap_or_else(|| {
        let mut page = PageState::of(shows, tab);
        // Changes beside a brief need the width for the diff, and have the brief to go by.
        if let (Some((_, Tile::Pinned(RowKey::Changes(_)), _)), Some(files)) =
            (key, page.files_mut())
        {
            files.files_hidden = true;
        }
        page
    })
}

/// The pages nothing draws now, as they were left, for when they're shown again. A page being
/// drawn is its detail's, and only here while it isn't.
#[derive(Default)]
pub struct KeptPages {
    pages: HashMap<PageKey, PageState>,
    /// The tab each project's page was left on, by its row and tile.
    tabs: HashMap<(RowKey, Tile), ProjectTab>,
}

impl KeptPages {
    pub(super) fn take(&mut self, key: &PageKey) -> Option<PageState> {
        self.pages.remove(key)
    }

    pub(super) fn keep(&mut self, key: PageKey, page: PageState) {
        if let Some(tab) = page.project_tab() {
            self.tabs.insert((key.0.clone(), key.1.clone()), tab);
        }
        self.pages.insert(key, page);
    }

    /// A branch renamed is the same row: its page and its stack's go on under the new name. A
    /// tile pinned to it stays under the name its tiling has it by.
    pub(super) fn follow_rename(
        &mut self,
        project: &ProjectId,
        from: Option<&FullName>,
        to: &FullName,
    ) {
        let rows: [fn(ProjectId, Option<FullName>) -> Selection; 2] =
            [Selection::Branch, Selection::Stack];
        for row in rows {
            let was = RowKey::Row(row(project.clone(), from.cloned()));
            if let Some(page) = self.pages.remove(&(was, Tile::Home, ProjectTab::default())) {
                let now = RowKey::Row(row(project.clone(), Some(to.clone())));
                self.pages
                    .insert((now, Tile::Home, ProjectTab::default()), page);
            }
        }
    }

    /// The pinned pages of `from`'s tiling are of `to`'s now, as the tiling went there.
    pub fn move_tiling(&mut self, from: &RowKey, to: &RowKey) {
        let moved: Vec<PageKey> = (self.pages.keys())
            .filter(|(row, tile, _)| row == from && *tile != Tile::Home)
            .cloned()
            .collect();
        for key in moved {
            if let Some(page) = self.pages.remove(&key) {
                self.pages.insert((to.clone(), key.1.clone(), key.2), page);
            }
            if let Some(tab) = self.tabs.remove(&(key.0, key.1.clone())) {
                self.tabs.insert((to.clone(), key.1), tab);
            }
        }
    }

    /// Forget the pages `keep` doesn't want, which went from the navigator or their tiling.
    pub fn retain(&mut self, keep: impl Fn(&PageKey) -> bool) {
        self.pages.retain(|key, _| keep(key));
        self.tabs
            .retain(|(row, tile), tab| keep(&(row.clone(), tile.clone(), *tab)));
    }
}

#[cfg(test)]
mod tests {
    use gpui::{ListOffset, Pixels};

    use super::{
        CursorPlace, FilePlace, Group, KeptPages, PageKey, PageState, RowPlace, offset_in,
    };
    use crate::backend::model::build::full;
    use crate::backend::{Checkout, DiffBody, FileChange, FileDiff, FullName, Hunk, HunkKey};
    use crate::detail::{Cursor, Stop};
    use crate::state::{ProjectTab, RowKey, Section, Selection, Tile};
    use crate::ui::diff::DiffRow;

    /// A file changing one line at each of `lines`, on the old side.
    fn file(path: &str, lines: &[u32]) -> FileDiff {
        let hunk = |old_start: &u32| Hunk {
            key: HunkKey(but_core::HunkHeader {
                old_start: *old_start,
                old_lines: 1,
                new_start: *old_start,
                new_lines: 1,
            }),
            header: String::new(),
            patch: "".into(),
            lines: Vec::new(),
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        };
        FileDiff {
            file: FileChange::bare(path),
            body: DiffBody::Hunks {
                hunks: lines.iter().map(hunk).collect(),
                added: 0,
                removed: 0,
                widest: [1, 1],
                conflicts: Vec::new(),
            },
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            commit: None,
        }
    }

    fn line_of(file: usize, hunk: usize) -> DiffRow {
        DiffRow::Line {
            file,
            hunk,
            line: 0,
        }
    }

    fn at(item_ix: usize, offset: f32) -> ListOffset {
        ListOffset {
            item_ix,
            offset_in_item: Pixels::from(offset),
        }
    }

    #[test]
    fn the_cursor_follows_its_file_past_files_added_before_it() {
        let was = [file("b.rs", &[10])];
        let place = CursorPlace::of(Cursor::on_file(0), &was);
        let now = [file("a.rs", &[1]), file("b.rs", &[10])];
        assert_eq!(place.find(&now), Cursor::on_file(1), "found by its path");
    }

    #[test]
    fn the_cursor_follows_its_hunk_past_hunks_added_before_it() {
        let was = [file("a.rs", &[30])];
        let place = CursorPlace::of(Cursor::on_hunk(0, 0), &was);
        let now = [file("a.rs", &[5, 30])];
        assert_eq!(
            place.find(&now),
            Cursor::on_hunk(0, 1),
            "a hunk keeps its lines of the old side"
        );
    }

    #[test]
    fn the_cursor_keeps_a_gap_above_its_hunk() {
        let was = [file("a.rs", &[30])];
        let cursor = Cursor {
            file: 0,
            at: Stop::Gap(0),
        };
        let now = [file("a.rs", &[5, 30])];
        assert_eq!(
            CursorPlace::of(cursor, &was).find(&now).at,
            Stop::Gap(1),
            "still the box above the same hunk"
        );
    }

    #[test]
    fn the_cursor_stays_where_a_file_went() {
        let was = [file("a.rs", &[1]), file("b.rs", &[1]), file("c.rs", &[1])];
        let place = CursorPlace::of(Cursor::on_hunk(1, 0), &was);
        let now = [file("a.rs", &[1]), file("c.rs", &[1])];
        assert_eq!(
            place.find(&now),
            Cursor::on_file(1),
            "on what moved up into its place, not the first file"
        );
    }

    #[test]
    fn the_cursor_stays_in_the_files_when_the_last_went() {
        let was = [file("a.rs", &[1]), file("b.rs", &[1])];
        let place = CursorPlace::of(Cursor::on_file(1), &was);
        let now = [file("a.rs", &[1])];
        assert_eq!(
            place.find(&now),
            Cursor::on_file(0),
            "there's no second file"
        );
    }

    #[test]
    fn the_cursor_stays_on_the_message() {
        let place = CursorPlace::of(Cursor::on_message(), &[]);
        assert_eq!(
            place.find(&[file("a.rs", &[1])]),
            Cursor::on_message(),
            "it's in no file"
        );
    }

    fn row_in(row: DiffRow, was: &FileDiff) -> RowPlace {
        RowPlace {
            row,
            file: Some(FilePlace::of(was)),
            gaps: 0,
        }
    }

    #[test]
    fn a_row_is_found_by_its_file_and_hunk() {
        let place = row_in(line_of(0, 0), &file("b.rs", &[30]));
        let now = [file("a.rs", &[1]), file("b.rs", &[5, 30])];
        let rows = [
            DiffRow::File(0),
            line_of(0, 0),
            DiffRow::File(1),
            line_of(1, 0),
            line_of(1, 1),
        ];
        assert_eq!(place.find(&rows, &now), Some((4, true)), "the same line");
    }

    #[test]
    fn a_row_gone_from_its_file_is_found_at_the_files_header() {
        let place = row_in(line_of(0, 0), &file("a.rs", &[30]));
        let now = [file("a.rs", &[30])];
        let rows = [DiffRow::Gap, DiffRow::File(0)];
        assert_eq!(
            place.find(&rows, &now),
            Some((1, false)),
            "its file is as near as it gets"
        );
    }

    #[test]
    fn a_row_of_a_file_gone_is_not_found() {
        let place = row_in(line_of(0, 0), &file("a.rs", &[30]));
        let now = [file("b.rs", &[1])];
        assert_eq!(
            place.find(&[DiffRow::File(0)], &now),
            None,
            "another file's rows say nothing of it"
        );
    }

    #[test]
    fn a_gap_is_found_by_the_row_after_it() {
        let place = RowPlace {
            gaps: 1,
            ..row_in(DiffRow::File(0), &file("a.rs", &[1]))
        };
        let rows = [DiffRow::Message, DiffRow::Gap, DiffRow::File(0)];
        assert_eq!(
            place.find(&rows, &[file("a.rs", &[1])]),
            Some((1, true)),
            "the gap above it"
        );
    }

    #[test]
    fn the_view_keeps_how_far_into_its_row_it_was() {
        let place = row_in(line_of(0, 0), &file("a.rs", &[30]));
        let (now, rows) = ([file("a.rs", &[30])], [DiffRow::File(0), line_of(0, 0)]);
        let offset = offset_in(at(9, 7.), Some(&place), &rows, &now, false);
        assert_eq!(
            (offset.item_ix, offset.offset_in_item),
            (1, Pixels::from(7.)),
            "the same row, as far down it"
        );
    }

    #[test]
    fn the_view_starts_a_files_header_at_its_top() {
        let place = row_in(line_of(0, 0), &file("a.rs", &[30]));
        let (now, rows) = ([file("a.rs", &[30])], [DiffRow::Gap, DiffRow::File(0)]);
        let offset = offset_in(at(9, 7.), Some(&place), &rows, &now, false);
        assert_eq!(
            (offset.item_ix, offset.offset_in_item),
            (1, Pixels::ZERO),
            "the offset was into a row that went"
        );
    }

    #[test]
    fn a_view_at_the_very_top_stays_there() {
        let place = row_in(line_of(0, 0), &file("a.rs", &[30]));
        let (now, rows) = ([file("a.rs", &[30])], [DiffRow::Gap, line_of(0, 0)]);
        let offset = offset_in(at(0, 0.), Some(&place), &rows, &now, false);
        assert_eq!(offset.item_ix, 0, "what's added above shows");
    }

    #[test]
    fn one_file_shown_alone_starts_at_its_top_when_the_row_is_gone() {
        let offset = offset_in(at(9, 7.), None, &[], &[], true);
        assert_eq!(
            (offset.item_ix, offset.offset_in_item),
            (0, Pixels::ZERO),
            "the rows are another file's"
        );
    }

    #[test]
    fn the_view_stays_as_far_down_when_the_row_is_gone() {
        let offset = offset_in(at(9, 7.), None, &[], &[], false);
        assert_eq!(
            (offset.item_ix, offset.offset_in_item),
            (9, Pixels::ZERO),
            "as near as it can be without its row"
        );
    }

    fn commit(key: &str) -> RowKey {
        RowKey::Commit("project".into(), key.into())
    }

    fn hidden_diff() -> PageState {
        let mut page = PageState::of(
            Some(&Selection::Section(
                "project".into(),
                Section::Upstream(full("refs/heads/feature")),
            )),
            ProjectTab::default(),
        );
        if let Some(files) = page.files_mut() {
            files.files_hidden = true;
        }
        page
    }

    fn is_hidden(page: Option<PageState>) -> bool {
        page.is_some_and(|page| page.files().is_some_and(|files| files.files_hidden))
    }

    #[test]
    fn the_files_beside_a_list_start_hidden_and_those_beside_a_diff_shown() {
        let page = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::History,
        );
        let files = page.files();
        assert!(
            files.is_some_and(|files| files.beside_files_hidden),
            "a list page's own list already takes their room"
        );
        assert!(
            files.is_some_and(|files| !files.files_hidden),
            "nothing hid them yet"
        );
    }

    #[test]
    fn uncommitted_changes_have_files() {
        let selection = Selection::Uncommitted("project".into(), Checkout::Main);
        let page = PageState::of(Some(&selection), ProjectTab::default());
        assert!(matches!(page, PageState::Changes { .. }), "its own kind");
        assert!(page.files().is_some(), "their diff is the page");
    }

    #[test]
    fn what_a_pull_brings_in_is_a_diff() {
        let section = Section::Upstream(full("refs/heads/feature"));
        let selection = Selection::Section("project".into(), section);
        assert!(
            matches!(
                PageState::of(Some(&selection), ProjectTab::default()),
                PageState::Diff { .. }
            ),
            "unlike the other sections, it shows files"
        );
    }

    #[test]
    fn a_branch_can_show_a_pull_request() {
        let selection = Selection::Branch("project".into(), None);
        assert!(
            PageState::of(Some(&selection), ProjectTab::default())
                .review()
                .is_some(),
            "a branch has a tab for one"
        );
    }

    #[test]
    fn a_commit_has_no_pull_request_of_its_own() {
        let selection = Selection::Stack("project".into(), None);
        assert!(
            PageState::of(Some(&selection), ProjectTab::default())
                .review()
                .is_none(),
            "its branches have them"
        );
    }

    #[test]
    fn a_page_of_a_pull_request_opens_on_its_changes() {
        let page = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::Branches,
        );
        assert!(!page.review_tab(), "the tab is asked for");
    }

    #[test]
    fn a_list_page_has_a_list_and_a_diff_page_has_none() {
        let page = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::History,
        );
        assert!(page.side().is_some(), "the operations are listed");
        let selection = Selection::Branch("project".into(), None);
        assert!(
            PageState::of(Some(&selection), ProjectTab::default())
                .side()
                .is_none(),
            "the navigator lists it"
        );
    }

    #[test]
    fn only_the_history_leaves_operations_out() {
        let group = Group::Commits;
        let mut history = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::History,
        );
        history.toggle_operations(group);
        assert!(history.hides_operations(&group), "it's left out now");
        history.toggle_operations(group);
        assert_eq!(history.operations_hidden(), 0, "and put back");
        let mut graph = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::Graph,
        );
        graph.toggle_operations(group);
        assert_eq!(graph.operations_hidden(), 0, "it lists no operations");
    }

    #[test]
    fn the_graph_has_no_files() {
        let page = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::Graph,
        );
        assert!(matches!(page, PageState::Graph(_)), "its own kind");
        assert!(page.files().is_none(), "it has no diff");
    }

    #[test]
    fn a_checkouts_overview_has_no_files() {
        let selection = Selection::Checkout("project".into(), Checkout::Main);
        let page = PageState::of(Some(&selection), ProjectTab::default());
        assert!(
            matches!(page, PageState::Overview(_)),
            "it sums up a checkout"
        );
        assert!(page.files().is_none(), "it has no diff");
    }

    #[test]
    fn a_section_of_worktrees_is_an_overview() {
        let selection = Selection::Section("project".into(), Section::Inactive);
        assert!(
            matches!(
                PageState::of(Some(&selection), ProjectTab::default()),
                PageState::Overview(_)
            ),
            "the navigator lists what it sums up"
        );
    }

    #[test]
    fn nothing_selected_is_blank() {
        assert!(
            matches!(PageState::of(None, ProjectTab::default()), PageState::Blank),
            "there's no page"
        );
    }

    fn project_page(tab: ProjectTab) -> PageState {
        PageState::of(Some(&Selection::Project("project".into())), tab)
    }

    fn project_row() -> RowKey {
        RowKey::Row(Selection::Project("project".into()))
    }

    #[test]
    fn a_projects_page_opens_on_the_tab_asked_for() {
        assert_eq!(
            project_page(ProjectTab::Branches).project_tab(),
            Some(ProjectTab::Branches),
            "which tab shows is the page's own"
        );
        assert_eq!(
            project_page(ProjectTab::Graph).project_tab(),
            Some(ProjectTab::Graph),
            "the Graph is a tab like the others"
        );
    }

    #[test]
    fn only_a_projects_page_has_tabs() {
        let branch = Selection::Branch("project".into(), None);
        assert_eq!(
            PageState::of(Some(&branch), ProjectTab::Graph).project_tab(),
            None,
            "a branch's page has tabs of its own, kept in its review"
        );
    }

    #[test]
    fn each_tab_of_a_project_is_kept_apart() {
        let mut kept = KeptPages::default();
        let (row, home) = (project_row(), Tile::Home);
        let branches = (row.clone(), home.clone(), ProjectTab::Branches);
        kept.keep(branches.clone(), project_page(ProjectTab::Branches));
        assert!(
            kept.take(&(row, home, ProjectTab::History)).is_none(),
            "another tab has a page of its own"
        );
        assert!(
            kept.take(&branches).is_some(),
            "the tab left keeps its page"
        );
    }

    #[test]
    fn a_projects_page_keeps_the_tab_it_was_left_on() {
        let mut kept = KeptPages::default();
        let key = (project_row(), Tile::Home, ProjectTab::History);
        kept.keep(key, project_page(ProjectTab::History));
        assert_eq!(
            kept.tabs.get(&(project_row(), Tile::Home)),
            Some(&ProjectTab::History),
            "coming back shows it, as a branch's page shows the tab it was left on"
        );
    }

    #[test]
    fn a_page_without_tabs_keeps_no_tab() {
        let mut kept = KeptPages::default();
        kept.keep(
            (commit("a"), Tile::Home, ProjectTab::default()),
            hidden_diff(),
        );
        assert!(kept.tabs.is_empty(), "only a project's page has them");
    }

    #[test]
    fn a_pinned_tab_goes_with_its_tiling() {
        let mut kept = KeptPages::default();
        let (from, to) = (commit("a"), commit("b"));
        let pinned = Tile::Pinned(RowKey::Tab("project".into(), ProjectTab::Branches));
        let key = (from.clone(), pinned.clone(), ProjectTab::Graph);
        kept.keep(key, project_page(ProjectTab::Graph));
        kept.move_tiling(&from, &to);
        assert_eq!(
            kept.tabs.get(&(to, pinned)),
            Some(&ProjectTab::Graph),
            "it stays on the tab it was switched to"
        );
    }

    #[test]
    fn a_page_comes_back_as_it_was_left() {
        let mut kept = KeptPages::default();
        let key: PageKey = (commit("a"), Tile::Home, ProjectTab::default());
        kept.keep(key.clone(), hidden_diff());
        assert!(is_hidden(kept.take(&key)), "as it was left");
        assert!(kept.take(&key).is_none(), "its detail has it now");
    }

    #[test]
    fn one_row_is_kept_apart_in_each_tile_showing_it() {
        let mut kept = KeptPages::default();
        let beside_a: PageKey = (
            commit("a"),
            Tile::Pinned(commit("x")),
            ProjectTab::default(),
        );
        let beside_b: PageKey = (
            commit("b"),
            Tile::Pinned(commit("x")),
            ProjectTab::default(),
        );
        let at_home: PageKey = (commit("x"), Tile::Home, ProjectTab::default());
        kept.keep(beside_a.clone(), hidden_diff());
        assert!(kept.take(&beside_b).is_none(), "another tiling's tile");
        assert!(kept.take(&at_home).is_none(), "its own home tile");
        assert!(is_hidden(kept.take(&beside_a)), "the tile that hid them");
    }

    #[test]
    fn a_tilings_pinned_pages_go_with_it() {
        let mut kept = KeptPages::default();
        let (from, to) = (commit("a"), commit("b"));
        let pinned = Tile::Pinned(commit("x"));
        kept.keep(
            (from.clone(), pinned.clone(), ProjectTab::default()),
            hidden_diff(),
        );
        kept.move_tiling(&from, &to);
        assert!(
            kept.take(&(from, pinned.clone(), ProjectTab::default()))
                .is_none(),
            "its row has no tiling now"
        );
        assert!(
            is_hidden(kept.take(&(to, pinned, ProjectTab::default()))),
            "as it was left"
        );
    }

    #[test]
    fn a_rows_own_page_stays_as_its_tiling_goes() {
        let mut kept = KeptPages::default();
        let (from, to) = (commit("a"), commit("b"));
        kept.keep(
            (from.clone(), Tile::Home, ProjectTab::default()),
            hidden_diff(),
        );
        kept.move_tiling(&from, &to);
        assert!(
            is_hidden(kept.take(&(from, Tile::Home, ProjectTab::default()))),
            "it's still that row's"
        );
    }

    fn branch(name: &str) -> FullName {
        FullName::try_from(format!("refs/heads/{name}")).expect("a valid name")
    }

    fn branch_row(name: &str) -> RowKey {
        RowKey::Row(Selection::Branch("project".into(), Some(branch(name))))
    }

    fn stack_row(name: &str) -> RowKey {
        RowKey::Row(Selection::Stack("project".into(), Some(branch(name))))
    }

    #[test]
    fn a_branchs_page_follows_its_rename() {
        let mut kept = KeptPages::default();
        kept.keep(
            (branch_row("old"), Tile::Home, ProjectTab::default()),
            hidden_diff(),
        );
        kept.follow_rename(&"project".into(), Some(&branch("old")), &branch("new"));
        assert!(
            is_hidden(kept.take(&(branch_row("new"), Tile::Home, ProjectTab::default()))),
            "it's the same row by another name"
        );
        assert!(
            kept.take(&(branch_row("old"), Tile::Home, ProjectTab::default()))
                .is_none(),
            "nothing's left under the old name"
        );
    }

    #[test]
    fn a_stacks_page_follows_its_bottom_branchs_rename() {
        let mut kept = KeptPages::default();
        kept.keep(
            (stack_row("old"), Tile::Home, ProjectTab::default()),
            hidden_diff(),
        );
        kept.follow_rename(&"project".into(), Some(&branch("old")), &branch("new"));
        assert!(
            is_hidden(kept.take(&(stack_row("new"), Tile::Home, ProjectTab::default()))),
            "a stack goes by its bottom branch"
        );
    }

    #[test]
    fn another_projects_branch_of_that_name_stays() {
        let mut kept = KeptPages::default();
        kept.keep(
            (branch_row("old"), Tile::Home, ProjectTab::default()),
            hidden_diff(),
        );
        kept.follow_rename(&"other".into(), Some(&branch("old")), &branch("new"));
        assert!(
            is_hidden(kept.take(&(branch_row("old"), Tile::Home, ProjectTab::default()))),
            "it wasn't renamed"
        );
    }

    #[test]
    fn a_new_list_starts_at_its_top() {
        let page = PageState::of(
            Some(&Selection::Project("project".into())),
            ProjectTab::History,
        );
        let top = page.side().and_then(|side| side.top.take());
        assert!(
            top.is_some_and(|top| top.item_ix == 0),
            "the list is shared with other projects' pages, which leave it scrolled"
        );
    }

    #[test]
    fn pages_gone_are_forgotten() {
        let mut kept = KeptPages::default();
        let (gone, stays): (PageKey, PageKey) = (
            (commit("a"), Tile::Home, ProjectTab::default()),
            (commit("b"), Tile::Home, ProjectTab::default()),
        );
        kept.keep(gone.clone(), hidden_diff());
        kept.keep(stays.clone(), hidden_diff());
        kept.retain(|key| *key != gone);
        assert!(kept.take(&gone).is_none(), "its row went");
        assert!(kept.take(&stays).is_some(), "its row's still there");
    }
}
