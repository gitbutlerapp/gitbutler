//! The navigator's tree of rows as it's built, before any is drawn: every card's
//! projects, worktrees, stacks, branches and commits, and under the filter, what it finds.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, SharedString};

use super::moving::{landing_at, left_behind};
use super::{BranchKey, ClickOpened, MERGE_GUIDE, Navigator, STEP, UPSTREAM_GUIDE};
use crate::backend::{
    Branch, Checkout, Commit, CommitId, Destination, FullName, InactiveAfter, Lane, Workspace,
    Worktree,
};
use crate::state::{NewAt, NewCommit, ProjectEntry, ProjectId, Section, Selection, Store};
use crate::terminal::TerminalId;
use crate::transfer::{self, Dragged, Stop};
use crate::ui::fold::{Folds, MOVING_ROWS};

/// How wide each loading row's name is, as 🍑 But UI's loading card draws them: names of the
/// lengths a card's rows often have.
pub(super) const LOADING_WIDTHS: [f32; 4] = [156., 76., 199., 116.];

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

/// How tall a watched project's heading is, atop its card: a row's, as but-ui-react's RepoItem.
pub(super) const HEADING_HEIGHT: f32 = 28.;

/// The room inside a card around its rows, as 🍑 But UI's sidebar cards have it.
pub(super) const CARD_PADDING: f32 = 8.;

/// How tall the line under a project's heading is with the room around it: the heading's
/// padding below, the 1px line, and the padding above the card's rows.
pub(super) const HEADER_RULE: f32 = 2. * CARD_PADDING + 1.;

/// How far a row's ground sits in from its card's sides: the card's padding.
pub(super) const ROW_MARGIN: f32 = CARD_PADDING;

/// How much a card's edge adds above its first row and below its last: its padding and its
/// 1px border.
pub(super) const CARD_EDGE: f32 = CARD_PADDING + 1.;

/// Whether row `ix` of `rows` is its card's first, and whether it's its last, which hold the
/// card's edge.
pub(super) fn card_ends(rows: &[Row], ix: usize) -> (bool, bool) {
    let edge = |row: Option<&Row>| row.is_none_or(|row| matches!(row, Row::Gap));
    let above = ix.checked_sub(1).and_then(|ix| rows.get(ix));
    (edge(above), edge(rows.get(ix + 1)))
}

/// How far below the top of its card the middle of a project's heading is.
pub(super) const HEADING_MIDDLE: f32 = CARD_EDGE + HEADING_HEIGHT / 2.;

/// How recently a project was last active for "Active in the last 7 days" to keep it.
const ACTIVE_WITHIN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Rows in display order. Only [`Row::Item`] can be selected.
#[derive(PartialEq)]
pub(super) enum Row {
    /// The space between two cards.
    Gap,
    /// Whatever the [`Selection`] names: a project, a worktree, a branch, a commit,
    /// one of a project's pages, a section's heading, or one of these on another machine.
    Item(Selection),
    Note(SharedString, bool),
    /// A row still loading, the `n`th of a card's, in the card's rows' place until they land.
    Loading(usize),
    /// A branch's commits folded away, and how many.
    More(ProjectId, Option<FullName>, usize),
    /// The end of a branch's commits unfolded, which folds them again.
    Fewer(ProjectId, Option<FullName>),
    /// A commit a mutation is making or moving, by the title it shows by, whether it's been
    /// long enough to show it's still on its way, and the commit it stands in for, when it moves
    /// one. One the mutation selects once it's made is an item, [`Selection::Pending`].
    Pending(SharedString, bool, Option<RowKey>),
    /// Where a commit of the project being dragged lands in a new branch of its own, or a
    /// stacked branch in a stack of its own.
    NewBranch(ProjectId),
    /// A copy of what's being moved, a commit or a branch's row, where landing would put it.
    Ghost(Selection),
    /// The new branch commits being moved would land in at that stop, over their copies.
    GhostBranch(ProjectId, Stop),
    /// The caption a branch alone in its lane would get as a new branch lands beside it,
    /// making the two a stack, over both.
    GhostStack(ProjectId),
    /// The commit uncommitted changes being committed would make where the commit's at.
    NewCommit(ProjectId),
    /// Where one stack ends and the next starts, a hairline across, so two branches alone in
    /// their lanes don't read as one stack. A branch dropped low on one stack's last branch
    /// joins that stack rather than the next.
    Between(ProjectId),
    WorktreeDivider,
    /// The line under a watched project's heading, across its card, which sets the heading apart
    /// from the rows under it, as 🍑 But UI's and but-ui-react's cards have their header; and
    /// whether the main worktree's rows follow it with no heading of their own.
    HeaderRule(bool),
    /// In a linked worktree's section, a branch its lane rests on, by the row it selects, with
    /// the checkout it's in on the first of each lane.
    RestsOn(Selection, Option<Checkout>),
    /// The drawer with no projects hidden in it: a card saying so, as but-ui-react's Drawer
    /// draws it empty.
    Empty,
}

/// What a row is across rebuilds, for a reorder to slide it from where it was.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum RowKey {
    Item(Selection),
    /// A commit by its key, which outlives the rewrites that move it.
    Commit(ProjectId, String),
    /// A terminal by its id alone, which outlives moving it to another project.
    Terminal(TerminalId),
    RestsOn(Selection),
    /// A branch's commits folded away or unfolded, whichever it shows.
    Fold(ProjectId, Option<FullName>),
    NewBranch(ProjectId),
    /// The row of what's being moved, a commit or a branch, by the key it has otherwise, while
    /// it is: its copy, which takes that key, slides out from it to where it'd land, from place
    /// to place, and back over it.
    Left(Box<RowKey>),
    GhostBranch(ProjectId),
    GhostStack(ProjectId),
    /// The commit being made, which slides from stop to stop as the commit steps.
    NewCommit(ProjectId),
    /// No row's: the top edge of the outline of what a squash goes into, which slides from row
    /// to row as the squash steps.
    SquashTop,
    /// No row's: that outline's bottom edge, which slides apart from its top as the outline
    /// grows into the rows where the squash started, and back as it leaves.
    SquashBottom,
    /// A row of no other kind, by the one before it, which it moves with.
    After(Box<RowKey>),
    /// The line under a project's heading, by the heading's key: a key of its own, so the row
    /// that takes its place as the card folds, the gap after the card, isn't counted as it and
    /// slid from where it was.
    HeaderRule(Box<RowKey>),
}

pub(super) fn row_keys(rows: &[Row], store: &Store) -> Vec<Option<RowKey>> {
    let commits: HashMap<CommitId, &str> = (store.projects.iter())
        .filter_map(|entry| entry.workspace.as_ref())
        .flat_map(|workspace| workspace.commits())
        .map(|commit| (commit.id, commit.key.as_str()))
        .collect();
    let item_key = |item: &Selection| item_key(item, &commits);
    let mut keys: Vec<Option<RowKey>> = Vec::with_capacity(rows.len());
    // The key of the last row that isn't a copy of what's moving, which the rows of no other
    // kind after it go by, as they make room for the copies rather than show up in place.
    let mut before = None;
    for row in rows {
        let key = match row {
            Row::Item(item) if left_behind(store, item) => {
                Some(RowKey::Left(Box::new(item_key(item))))
            }
            Row::Item(item) | Row::Ghost(item) => Some(item_key(item)),
            Row::RestsOn(item, _) => Some(RowKey::RestsOn(item.clone())),
            Row::More(project, branch, _) | Row::Fewer(project, branch) => {
                Some(RowKey::Fold(project.clone(), branch.clone()))
            }
            Row::NewBranch(project) => Some(RowKey::NewBranch(project.clone())),
            Row::GhostBranch(project, _) => Some(RowKey::GhostBranch(project.clone())),
            Row::GhostStack(project) => Some(RowKey::GhostStack(project.clone())),
            Row::NewCommit(project) => Some(RowKey::NewCommit(project.clone())),
            Row::Pending(_, _, Some(moved)) => Some(moved.clone()),
            Row::HeaderRule(_) => {
                (before.clone()).map(|before| RowKey::HeaderRule(Box::new(before)))
            }
            _ => (before.clone()).map(|before| RowKey::After(Box::new(before))),
        };
        if !matches!(
            row,
            Row::Ghost(_)
                | Row::GhostBranch(..)
                | Row::GhostStack(_)
                | Row::NewCommit(_)
                | Row::HeaderRule(_)
        ) {
            before = key.clone();
        }
        keys.push(key);
    }
    keys
}

/// An item's row, and a copy of it, by the item, with the keys of the `commits` there are.
fn item_key(item: &Selection, commits: &HashMap<CommitId, &str>) -> RowKey {
    match item {
        Selection::Commit(project, id) => match commits.get(id) {
            Some(key) => RowKey::Commit(project.clone(), key.to_string()),
            None => RowKey::Item(item.clone()),
        },
        Selection::Terminal(_, id) => RowKey::Terminal(*id),
        _ => RowKey::Item(item.clone()),
    }
}

/// How tall rows `span` of `rows` are together, as [`Row::height`] counts them with rows `row`
/// tall.
pub(super) fn span_height(rows: &[Row], span: Range<usize>, row: f32) -> f32 {
    rows.get(span)
        .unwrap_or_default()
        .iter()
        .map(|r| r.height(row))
        .sum()
}

impl Row {
    /// How tall the row is drawn, near enough for a fold moving it to count by. The gap between
    /// two cards counts their edges too, rather than the rows either side of it, so a row that
    /// comes to start or end a card, as a heading does as its card folds, keeps its height, and
    /// rows sliding as cards trade places start where they're drawn. `row` is how tall an item's
    /// row is, as the density has it.
    pub(super) fn height(&self, row: f32) -> f32 {
        match self {
            Row::Gap => super::CARD_GAP + 2. * CARD_EDGE,
            Row::Between(_) => BETWEEN,
            Row::Empty => super::DRAWER_EMPTY,
            // Worktrees follow one another with nothing between, as 🍑 But UI's sidebar has
            // them; the divider stays, for what drops and moves between them.
            Row::WorktreeDivider => 0.,
            // The heading's padding below, the line, and the card's padding above its rows.
            Row::HeaderRule(_) => HEADER_RULE,
            Row::Item(Selection::Project(_)) => HEADING_HEIGHT,
            Row::Item(_)
            | Row::More(..)
            | Row::Fewer(..)
            | Row::RestsOn(..)
            | Row::Note(..)
            | Row::Loading(_)
            | Row::Pending(..)
            | Row::NewBranch(_)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::GhostStack(_)
            | Row::NewCommit(_) => row,
        }
    }
}

/// What a fold in the cards holds: the rows under an item's row, a branch's commits past the
/// first few, the drawer's projects, or the rows of a project unfolded in the drawer.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Fold {
    Under(Selection),
    More(BranchKey),
    Drawer,
    InDrawer(ProjectId),
}

/// Where a row is in the fold it moves with: how far down the fold's rows, by their heights,
/// how tall it is, and how tall they are together, and the row above them, which says where
/// on screen the fold starts.
#[derive(Clone)]
pub(super) struct Moving {
    pub(super) fold: Fold,
    pub(super) top: f32,
    pub(super) height: f32,
    pub(super) extent: f32,
    pub(super) above: Option<usize>,
}

/// The rows as they're built, each with how far in it sits: a worktree's section and a
/// stack's branches step in from what holds them. Each also has the guides it draws, as
/// but.dev's sidebar does: a line down from under the icon of each row it's under, and while
/// a fold holding it moves, its place in that.
#[derive(Default)]
pub(super) struct Rows {
    pub(super) rows: Vec<Row>,
    pub(super) insets: Vec<f32>,
    pub(super) guides: Vec<Vec<f32>>,
    /// Whether each row is the last under its innermost guide, which turns into it there, as
    /// but-ui-react's tree guides end, rather than running on.
    pub(super) ends: Vec<bool>,
    pub(super) moving: Vec<Option<Moving>>,
    /// How many guides were open as each row was pushed: a row is under the last one of them.
    depths: Vec<usize>,
    /// Where the rows under each guide in `open` start.
    starts: Vec<usize>,
    /// Each fold the rows were built through: whether it showed what it holds, and the row
    /// just above that, which says where it is.
    pub(super) folds: Vec<(Fold, bool, Option<usize>)>,
    /// What the rows pushed next sit in by.
    inset: f32,
    /// Where the guides of the rows pushed next run.
    open: Vec<f32>,
    /// What the rows sat in by before each guide in `open` began, which closing it restores.
    outer: Vec<f32>,
    /// The first item the filter found, which Enter in it selects.
    pub(super) first_hit: Option<Selection>,
    pub(super) drawer: Drawer,
    /// How tall an item's row is, as the density has it, which a fold that moves counts by.
    pub(super) row: f32,
}

/// The projects not watched, in the drawer under the cards: their rows while it's open, how
/// many it lists, and how many there are, filters aside. With no cards it's always open.
#[derive(Default)]
pub(super) struct Drawer {
    pub(super) rows: Vec<Row>,
    pub(super) listed: usize,
    pub(super) total: usize,
    /// How far in each row sits, the guides it draws, and whether it ends its innermost one,
    /// as [`Rows`] has them: the rows of a project unfolded in the drawer step in as a card's.
    pub(super) insets: Vec<f32>,
    pub(super) guides: Vec<Vec<f32>>,
    pub(super) ends: Vec<bool>,
    /// Whether each row is a project folded in it, in a group of its own in a card of the
    /// projects folded next to it, rather than a row of one unfolded, a card of its own.
    pub(super) grouped: Vec<bool>,
    /// Where each row is in the fold of a project unfolding or folding here, as [`Rows`] has it.
    pub(super) moving: Vec<Option<Moving>>,
    /// For a gap that parts a project's card from the folded projects only while it's unfolded,
    /// that project's fold, which opens the gap as it opens.
    pub(super) parting: Vec<Option<Fold>>,
    /// The folds of the projects listed, as [`Rows::folds`] has them.
    pub(super) folds: Vec<(Fold, bool, Option<usize>)>,
}

/// Where an item row's content starts in its card: the row's margin and its padding, 8, as
/// [`RowStyle::row`](super::RowStyle::row) and [`section_heading`](super::section_heading)
/// set them.
pub(super) const ROW_LEFT: f32 = ROW_MARGIN + 8.;

/// How far in from [`ROW_LEFT`] a commit's 12-point glyph sits: a step in from its branch and
/// 2 on, where but-ui-react puts the glyph of a row that doesn't fold, under its branch's
/// glyph, and a point into that 14-point box.
pub(super) const COMMIT_INDENT: f32 = STEP + 2. + 1.;

/// Where the middle of a row's chevron sits in a row set in by nothing: the row's left edge
/// and half the 12-point step, where the guide down to the rows under it hangs from.
pub(super) const GUIDE_AT: f32 = ROW_LEFT + 6.;

/// How tall the gap between two stacks is, its hairline across the middle.
pub(super) const BETWEEN: f32 = 9.;

/// The caption over `project`'s lane of `branches`, as drawn: over two or more, by the bottom one.
pub(super) fn caption(project: &ProjectId, branches: &[Branch]) -> Option<Selection> {
    let [_, .., bottom] = branches else {
        return None;
    };
    Some(Selection::Stack(project.clone(), bottom.ref_name.clone()))
}

impl Rows {
    /// No rows yet, an item's row `row` tall.
    pub(super) fn new(row: f32) -> Self {
        Rows {
            row,
            ..Rows::default()
        }
    }

    pub(super) fn push(&mut self, row: Row) {
        self.rows.push(row);
        self.insets.push(self.inset);
        self.guides.push(self.open.clone());
        self.ends.push(false);
        self.depths.push(self.open.len());
        self.moving.push(None);
    }

    /// Push, with `rows`, what `fold` holds while it shows that: `open`, or still closing.
    /// While it moves, those rows are drawn as far as it's open, as one curtain, the first
    /// [`MOVING_ROWS`] of them; the rest join or go once it's there. A fold inside one that
    /// moves is drawn with it.
    pub(super) fn fold(
        &mut self,
        fold: Fold,
        open: bool,
        folds: &Folds<Fold>,
        rows: impl FnOnce(&mut Self),
    ) {
        let above = self.rows.len().checked_sub(1);
        self.folds.push((fold.clone(), open, above));
        if !folds.shows(&fold, open) {
            return;
        }
        let from = self.rows.len();
        rows(self);
        if folds.openness(&fold).is_none() {
            return;
        }
        self.truncate(self.rows.len().min(from + MOVING_ROWS));
        let row = self.row;
        let heights: Vec<f32> = (self.rows.iter().skip(from))
            .map(|r| r.height(row))
            .collect();
        let extent = heights.iter().sum();
        let mut top = 0.;
        for (moving, height) in self.moving.iter_mut().skip(from).zip(heights) {
            *moving = Some(Moving {
                fold: fold.clone(),
                top,
                height,
                extent,
                above,
            });
            top += height;
        }
    }

    /// Set the rows pushed until [`Self::close`] under the row just pushed, a guide running
    /// down beside them from under its icon.
    pub(super) fn open(&mut self) {
        self.open_at(GUIDE_AT);
    }

    /// As [`Self::open`], for a row whose icon's middle is `at` instead.
    fn open_at(&mut self, at: f32) {
        let under = self.insets.last().copied().unwrap_or_default();
        self.open.push(at + under);
        self.starts.push(self.rows.len());
        self.outer.push(self.inset);
    }

    /// As [`Self::open`], with the rows stepped in `by` further.
    pub(super) fn open_in(&mut self, by: f32) {
        self.open();
        self.inset += by;
    }

    /// End the guide [`Self::open`] began: it turns into the last row right under it, and the
    /// rows under that one go on without it, as but-ui-react's tree guides end.
    pub(super) fn close(&mut self) {
        let level = self.open.len();
        let start = self
            .starts
            .pop()
            .unwrap_or(self.rows.len())
            .min(self.rows.len());
        if let Some(at) = self.open.pop() {
            let last = (start..self.rows.len())
                .rev()
                .find(|&ix| self.depths.get(ix) == Some(&level));
            if let Some(last) = last {
                if let Some(end) = self.ends.get_mut(last) {
                    *end = true;
                }
                for guides in self.guides.iter_mut().skip(last + 1) {
                    guides.retain(|g| *g != at);
                }
            }
        }
        self.inset = self.outer.pop().unwrap_or_default();
    }

    /// Rule what follows off from the rows above with a line, unless one already does.
    pub(super) fn rule_off(&mut self) {
        if !matches!(self.rows.last(), Some(Row::WorktreeDivider)) {
            self.push(Row::WorktreeDivider);
        }
    }

    /// Take back a line ruling off the last rows, which end a card whose edge does that, so the
    /// last row sits as far in from the bottom edge as from the sides.
    pub(super) fn unrule_end(&mut self) {
        if matches!(self.rows.last(), Some(Row::WorktreeDivider)) {
            self.truncate(self.rows.len() - 1);
        }
    }

    /// Settle the [`Row::HeaderRule`] at `at`, under `project`'s heading: gone when no row
    /// follows it, and saying whether the main worktree's rows follow it with no heading.
    pub(super) fn settle_rule(&mut self, at: usize, project: &ProjectId) {
        if self.rows.len() == at + 1 {
            self.truncate(at);
            return;
        }
        let main = matches!(
            self.rows.get(at + 1),
            Some(Row::Item(next)) if next.project() == project && !matches!(next, Selection::Checkout(..))
        );
        if let Some(rule) = self.rows.get_mut(at) {
            *rule = Row::HeaderRule(main);
        }
    }

    /// Push `item`, the filter's first hit if it's `hit` and none came before it.
    pub(super) fn push_found(&mut self, item: Selection, hit: bool) {
        if hit && self.first_hit.is_none() {
            self.first_hit = Some(item.clone());
        }
        self.push(Row::Item(item));
    }

    /// Take back the rows pushed since there were `len`.
    pub(super) fn truncate(&mut self, len: usize) {
        self.rows.truncate(len);
        self.insets.truncate(len);
        self.guides.truncate(len);
        self.ends.truncate(len);
        self.depths.truncate(len);
        self.moving.truncate(len);
    }
}

impl Extend<Row> for Rows {
    fn extend<I: IntoIterator<Item = Row>>(&mut self, rows: I) {
        rows.into_iter().for_each(|row| self.push(row));
    }
}

impl Navigator {
    /// Rows of every card, with a gap between two cards, and of the drawer. Watched projects
    /// get a card each; the rest wait in the drawer.
    pub(super) fn build_rows(&self, cx: &App) -> Rows {
        let terms = self.filter_terms(cx);
        if !terms.is_empty() {
            return self.build_filtered_rows(&terms, cx);
        }
        if let Some(scope) = self.rows_scope(cx) {
            return self.scope_rows(&scope, cx);
        }
        let store = self.store.read(cx);
        let mut rows = Rows::new(self.row);
        for entry in self.watched(store) {
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            let item = Selection::Project(entry.id.clone());
            rows.push(Row::Item(item.clone()));
            let open = !store.project_folded(&entry.id);
            // The heading stands apart, ruled off from its rows, which start at the card's
            // edge rather than a step under it, as 🍑 But UI's and but-ui-react's cards have
            // their header.
            rows.fold(Fold::Under(item), open, &self.folds, |rows| {
                let rule = rows.rows.len();
                rows.push(Row::HeaderRule(false));
                self.card_rows(store, entry, rows);
                rows.settle_rule(rule, &entry.id);
            });
        }
        let mut listing = self.drawer_listing(store);
        rows.drawer.listed = listing.listed;
        rows.drawer.total = listing.total;
        // Its own projects' folds move wherever it's scrolled, as it's always all in view.
        let in_drawer = std::mem::take(&mut listing.folds);
        (rows.folds).extend(
            in_drawer
                .into_iter()
                .map(|(fold, open, _)| (fold, open, None)),
        );
        // With no cards it's always open.
        if !rows.rows.is_empty() {
            let open = !store.prefs.projects_folded;
            rows.folds.push((Fold::Drawer, open, None));
            if !self.folds.shows(&Fold::Drawer, open) {
                return rows;
            }
        }
        rows.drawer = listing;
        rows
    }

    /// The projects not watched, as the settings list them, folded or not.
    pub(super) fn drawer_listing(&self, store: &Store) -> Drawer {
        let others: Vec<&ProjectEntry> = (store.projects.iter())
            .filter(|entry| !entry.expanded)
            .collect();
        let prefs = &store.prefs;
        let selected = |entry: &ProjectEntry| {
            store.selection.as_ref() == Some(&Selection::Project(entry.id.clone()))
        };
        let mut listed: Vec<(&ProjectEntry, Option<i64>)> = (others.iter())
            .map(|entry| (*entry, last_active(entry)))
            .filter(|(entry, at)| {
                !prefs.active_projects_only || at.is_some_and(active_lately) || selected(entry)
            })
            .collect();
        // Stable, so projects active at the same moment stay by name.
        if prefs.projects_by_activity {
            listed.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        }
        // Each project unfolded in it a card of its own where it is in the list, its heading
        // over its rows as a watched project's card, splitting the projects folded around it
        // into cards of their own, as 🍑 But UI's drawer has them.
        // One unfolding or folding is a card of its own until it's folded, its rows opening or
        // closing under its heading, as a watched card's do, and the gaps parting it from the
        // folded projects around it opening and closing with them.
        let mut cards = Rows::new(self.row);
        let mut grouped = Vec::new();
        let mut parting = Vec::new();
        // Whether the project before was a card of its own, and its fold if that's moving.
        let mut was: Option<(bool, Option<Fold>)> = None;
        for (entry, _) in &listed {
            let fold = Fold::InDrawer(entry.id.clone());
            let open = self.drawer_open.contains(&entry.id);
            let shown = self.folds.shows(&fold, open);
            let moving = (shown && self.folds.openness(&fold).is_some()).then(|| fold.clone());
            if let Some((was_shown, was_moving)) = &was
                && (*was_shown || shown)
            {
                // Next to a card that stays open the gap stays; else it opens with the fold.
                let parts = match (was_shown, shown) {
                    (true, true) if was_moving.is_none() || moving.is_none() => None,
                    _ => was_moving.clone().or_else(|| moving.clone()),
                };
                cards.push(Row::Gap);
                parting.resize(cards.rows.len() - 1, None);
                parting.push(parts);
            }
            was = Some((shown, moving));
            cards.push(Row::Item(Selection::Project(entry.id.clone())));
            cards.fold(fold, open, &self.folds, |cards| {
                let rule = cards.rows.len();
                cards.push(Row::HeaderRule(false));
                self.card_rows(store, entry, cards);
                cards.settle_rule(rule, &entry.id);
            });
            grouped.resize(cards.rows.len(), !shown);
        }
        parting.resize(cards.rows.len(), None);
        if others.is_empty() {
            cards.push(Row::Empty);
            grouped.push(false);
        }
        if listed.is_empty() && !others.is_empty() {
            cards.push(Row::Note("None active in the last 7 days".into(), false));
            grouped.push(true);
        }
        Drawer {
            rows: cards.rows,
            listed: listed.len(),
            total: others.len(),
            insets: cards.insets,
            guides: cards.guides,
            ends: cards.ends,
            grouped,
            moving: cards.moving,
            parting,
            folds: cards.folds,
        }
    }

    /// The rows of watched project `entry`'s card under its own: its worktrees with their
    /// branches, then its checkouts elsewhere.
    pub(super) fn card_rows(&self, store: &Store, entry: &ProjectEntry, rows: &mut Rows) {
        let id = &entry.id;
        let Some(workspace) = &entry.workspace else {
            match &entry.error {
                Some(err) => rows.push(Row::Note(err.clone(), true)),
                // Rows of bones, as many as a card often holds, so it keeps its shape while the
                // workspace loads, as 🍑 But UI's loading card.
                None => rows.extend((0..LOADING_WIDTHS.len()).map(Row::Loading)),
            }
            return;
        };
        if let Some(err) = &entry.error {
            rows.push(Row::Note(err.clone(), true));
        }
        // Nothing but the edit goes on while a commit is checked out, its files listed
        // where the uncommitted ones are.
        if workspace.editing.is_some() {
            rows.push(Row::Item(Selection::Uncommitted(
                id.clone(),
                Checkout::Main,
            )));
            return;
        }
        // Alone, the main worktree's rows go right under the project's, which stands in for
        // its heading. Beside other checkouts its heading folds its branches away, which step
        // in under it, and each linked worktree is a section too, ruled off from the next.
        let selection = store.selection.as_ref().filter(|s| s.project() == id);
        if !store.main_headed(id) {
            self.main_checkout_rows(store, id, workspace, rows);
        } else {
            let main_selected =
                selection.is_some_and(|s| in_checkout(store, s, workspace, &Checkout::Main));
            let heading = Selection::Checkout(id.clone(), Checkout::Main);
            let main_open = self.shows_under(&heading, main_selected);
            rows.push(Row::Item(heading.clone()));
            rows.open_in(STEP);
            rows.fold(Fold::Under(heading), main_open, &self.folds, |rows| {
                self.main_checkout_rows(store, id, workspace, rows)
            });
            rows.close();
        }
        // Under the workspace it adds to, not past the worktrees.
        if lands_apart(store.dragging.as_ref(), id) {
            rows.push(Row::NewBranch(id.clone()));
        }
        let worktrees = worktrees_listed(store, id, workspace);
        if !worktrees.is_empty() {
            rows.push(Row::WorktreeDivider);
            let on_branches = |wt: &Worktree| {
                let checkout = Checkout::Linked(wt.name.clone());
                selection.is_some_and(|s| in_checkout(store, s, workspace, &checkout))
            };
            // The inactive ones wait after the rest, folded behind a row that says how many
            // unless opened or the selection's in one, as but-ui-react's MoreItem pages a list.
            let (inactive, active): (Vec<_>, Vec<_>) =
                (worktrees.into_iter()).partition(|wt| wt.is_inactive(store.prefs.inactive_after));
            for wt in active {
                self.worktree_rows(store, id, wt, on_branches(wt), rows);
            }
            if !inactive.is_empty() {
                let more = Selection::Section(id.clone(), Section::Inactive);
                let open = inactive_open(store, id, &inactive);
                rows.fold(Fold::Under(more.clone()), open, &self.folds, |rows| {
                    for wt in inactive {
                        self.worktree_rows(store, id, wt, on_branches(wt), rows);
                    }
                });
                rows.push(Row::Item(more));
            }
        }
        // Its checkouts on the user's other machines, as worktrees: none while signed out.
        self.mesh_rows(store, id, rows);
        rows.unrule_end();
    }

    /// What the main worktree's heading folds away: its uncommitted changes, its terminals and
    /// its stacks.
    pub(super) fn main_checkout_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        workspace: &Workspace,
        rows: &mut Rows,
    ) {
        changes_row(store, workspace, id, &Checkout::Main, rows);
        terminal_rows(store, id, &Checkout::Main, rows);
        // A stack being unapplied leaves at once, as one whose branches are all being
        // deleted does.
        let stacks = workspace.lanes_in(&Checkout::Main).filter(|stack| {
            !(stack.branches.iter())
                .any(|b| (b.ref_name.as_ref()).is_some_and(|r| store.hides_branch(id, r)))
                && !store.after_deletes(id, &stack.branches).is_empty()
        });
        // Whether a branch is checked out on its own or several share the workspace,
        // they're listed alike, a mark between each stack and the next; with none, as on
        // the default branch, there's nothing to list, and the next commit starts one.
        // What's being moved to a new stack shows where it goes, before a stack or last.
        // The new stack is parted from the others as they are from each other.
        let mut first = true;
        let mut part = |rows: &mut Rows| {
            if !std::mem::take(&mut first) {
                rows.push(Row::Between(id.clone()));
            }
        };
        for stack in stacks {
            let before = |at: &Stop| match at {
                Stop::NewStack {
                    before: Some(before),
                } => (stack.branches.iter()).any(|b| b.ref_name.as_ref() == Some(before)),
                _ => false,
            };
            if let Some(moving) = landing_at(store, id, before) {
                part(rows);
                rows.push_ghosts(moving, true);
            }
            part(rows);
            self.lane_rows(store, id, &stack.branches, rows);
        }
        if let Some(moving) = landing_at(store, id, |at| *at == Stop::NewStack { before: None }) {
            part(rows);
            rows.push_ghosts(moving, true);
        }
    }

    /// The rows while the filter has words in it: every project, worktree, branch and commit
    /// that matches, under what holds it, folded or not. One matches when each word is in
    /// its name or in the name of what holds it, and one at least is in its own, so "shop
    /// cart" finds shop's cart branch, not all of shop. A branch matches by its name or its
    /// pull request's title, a commit by its title, a project not expanded by its name.
    fn build_filtered_rows(&self, terms: &[String], cx: &App) -> Rows {
        let store = self.store.read(cx);
        let hit = |held, own| matches(terms, held, own);
        let mut rows = Rows::new(self.row);
        for entry in self.watched(store) {
            let id = &entry.id;
            let start = rows.rows.len();
            if start > 0 {
                rows.push(Row::Gap);
            }
            let own = found(terms, &[&entry.project.name]);
            rows.push_found(Selection::Project(id.clone()), hit(0, own));
            let rule = rows.rows.len();
            rows.push(Row::HeaderRule(false));
            let project_end = rows.rows.len();
            let workspace = (entry.workspace.as_ref()).filter(|ws| ws.editing.is_none());
            if let Some(workspace) = workspace {
                // The main worktree's branches under its heading, or alone, right under the
                // project's.
                let alone = !store.main_headed(id);
                let main = rows.rows.len();
                if !alone {
                    rows.push(Row::Item(Selection::Checkout(id.clone(), Checkout::Main)));
                    rows.open_in(STEP);
                }
                let heading_end = rows.rows.len();
                for stack in workspace.lanes_in(&Checkout::Main) {
                    // A mark between two stacks, taken back when nothing in the next matches.
                    let start = rows.rows.len();
                    if start > heading_end {
                        rows.push(Row::Between(id.clone()));
                    }
                    let lane = rows.rows.len();
                    self.filtered_lane(store, id, &stack.branches, terms, own, &mut rows);
                    if rows.rows.len() == lane {
                        rows.truncate(start);
                    }
                }
                if !alone {
                    rows.close();
                }
                if rows.rows.len() == heading_end {
                    rows.truncate(main);
                }
                for wt in worktrees_listed(store, id, workspace) {
                    let wt_start = rows.rows.len();
                    let wt_own = found(terms, &[wt.name.to_string().as_str()]);
                    let item = Selection::Checkout(id.clone(), Checkout::Linked(wt.name.clone()));
                    rows.push_found(item, hit(own, wt_own));
                    rows.open_in(STEP);
                    let lane = workspace.worktree_lane(wt.name.as_ref());
                    let branches = lane.map_or(&[][..], |lane| &lane.branches);
                    self.filtered_lane(store, id, branches, terms, own | wt_own, &mut rows);
                    rows.close();
                    if rows.rows.len() == wt_start + 1 && !hit(own, wt_own) {
                        rows.truncate(wt_start);
                    }
                }
            }
            self.filtered_mesh_rows(store, id, terms, own, &mut rows);
            if rows.rows.len() == project_end && !hit(0, own) {
                rows.truncate(start);
            } else {
                rows.settle_rule(rule, id);
            }
        }
        let others = (store.projects.iter()).filter(|entry| !entry.expanded);
        let total = others.clone().count();
        let listed: Vec<Row> = others
            .filter(|entry| hit(0, found(terms, &[&entry.project.name])))
            .map(|entry| Row::Item(Selection::Project(entry.id.clone())))
            .collect();
        if rows.first_hit.is_none()
            && let Some(Row::Item(first)) = listed.first()
        {
            rows.first_hit = Some(first.clone());
        }
        rows.drawer = Drawer {
            listed: listed.len(),
            total,
            rows: listed,
            ..Drawer::default()
        };
        if rows.rows.is_empty() && rows.drawer.rows.is_empty() {
            rows.push(Row::Note("Nothing matches".into(), false));
        }
        rows
    }

    /// [`Self::lane_rows`] under the filter, with `held` the words found in what holds the
    /// lane: the branches that match or have commits that do, with those commits, under
    /// their stack's caption when there are two or more.
    fn filtered_lane(
        &self,
        store: &Store,
        id: &ProjectId,
        branches: &[Branch],
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        // A stack being unapplied leaves at once.
        if (branches.iter())
            .any(|b| (b.ref_name.as_ref()).is_some_and(|r| store.hides_branch(id, r)))
        {
            return;
        }
        let branches = &*store.after_deletes(id, branches);
        let start = rows.rows.len();
        let caption = caption(id, branches);
        if let Some(caption) = &caption {
            rows.push(Row::Item(caption.clone()));
            rows.open_in(STEP);
        }
        let caption_end = rows.rows.len();
        for branch in branches {
            let own = found(
                terms,
                &[store.name_of(id, branch), store.label_of(id, branch)],
            );
            let commits: Vec<_> = (branch.commits.iter())
                .filter(|commit| {
                    !store.hides_commit(id, &commit.key)
                        && matches(terms, held | own, found(terms, &[&commit.title]))
                })
                .collect();
            let branch_hit = matches(terms, held, own);
            if !branch_hit && commits.is_empty() {
                continue;
            }
            rows.push_found(
                Selection::Branch(id.clone(), branch.ref_name.clone()),
                branch_hit,
            );
            rows.open();
            for commit in commits {
                rows.push_found(Selection::Commit(id.clone(), commit.id), true);
            }
            rows.close();
        }
        if caption.is_some() {
            rows.close();
        }
        // A caption with nothing found under it goes too.
        if rows.rows.len() == caption_end {
            rows.truncate(start);
        }
    }

    /// A stack's or a linked worktree's branches: over two or more a caption, which folds
    /// them away unless the selection's in them, and they step in under it, as ui-react's
    /// StackCaption does; then each branch with its commits, up to the fold, unless it's closed.
    fn lane_rows(&self, store: &Store, id: &ProjectId, branches: &[Branch], rows: &mut Rows) {
        let branches = &*store.after_deletes(id, branches);
        let Some(caption) = caption(id, branches) else {
            // A new branch landing beside a branch alone makes the two a stack, so they show
            // under the caption they'd have, stepped in as a stack's branches are.
            let stacks = |branch: &Branch| {
                landing_at(
                    store,
                    id,
                    |at| matches!(at, Stop::NewBranch { beside, .. } if branch.ref_name.as_ref() == Some(beside)),
                )
            };
            if !branches.iter().any(|branch| stacks(branch).is_some()) {
                return self.stacked_rows(store, id, branches, rows);
            }
            rows.push(Row::GhostStack(id.clone()));
            rows.open_in(STEP);
            self.stacked_rows(store, id, branches, rows);
            return rows.close();
        };
        let selected = (store.selection.as_ref())
            .is_some_and(|s| s.project() == id && branches.iter().any(|b| s.on(b)));
        let open = self.shows_under(&caption, selected);
        rows.push(Row::Item(caption.clone()));
        rows.fold(Fold::Under(caption), open, &self.folds, |rows| {
            rows.open_in(STEP);
            self.stacked_rows(store, id, branches, rows);
            rows.close();
        });
    }

    /// The rows of a lane's `branches`, and copies of commits being moved where they'd land
    /// in a new branch above one of them, or below the last.
    fn stacked_rows(&self, store: &Store, id: &ProjectId, branches: &[Branch], rows: &mut Rows) {
        let new_branch = |branch: &Branch, side: bool| {
            landing_at(store, id, |at| {
                matches!(at, Stop::NewBranch { beside, below }
                    if *below == side && branch.ref_name.as_ref() == Some(beside))
            })
        };
        for branch in branches {
            if let Some(moving) = new_branch(branch, false) {
                rows.push_ghosts(moving, true);
            }
            self.branch_rows(store, id, branch, rows);
        }
        if let Some(moving) = branches.last().and_then(|bottom| new_branch(bottom, true)) {
            rows.push_ghosts(moving, true);
        }
    }

    /// A branch's row and, unless it's closed, its remote's commits and its own, up to the
    /// fold unless that's open.
    fn branch_rows(&self, store: &Store, id: &ProjectId, branch: &Branch, rows: &mut Rows) {
        let key = (id.clone(), branch.ref_name.clone());
        // Past the fold, but never past the selected commit, as after moving it down a step,
        // nor the one a selected commit on its way goes next to.
        let selected_at = match &store.selection {
            Some(Selection::Commit(p, c)) if p == id => {
                branch.commits.iter().position(|commit| commit.id == *c)
            }
            Some(Selection::Pending(p, _, NewAt::Above(key) | NewAt::Below(key))) if p == id => {
                branch.commits.iter().position(|commit| commit.key == *key)
            }
            _ => None,
        };
        let unfolded = self.unfolded(&key);
        let fold_at = FOLD_AFTER.max(selected_at.map_or(0, |at| at + 1));
        let shown = match unfolded {
            true => branch.commits.len(),
            false => fold_at,
        };
        let item = Selection::Branch(key.0.clone(), key.1.clone());
        rows.push(Row::Item(item.clone()));
        let at_top = |at: &Stop| matches!(at, Stop::In(Destination::Branch(top)) if branch.ref_name.as_ref() == Some(top));
        let inside = |rows: &mut Rows| {
            rows.open();
            // An unnamed branch has no remote of its own.
            if let (Some(workspace), Some(ref_name)) = (store.workspace(id), &branch.ref_name) {
                self.upstream_rows(store, workspace, id, ref_name, rows);
            }
            let pending = |at: &dyn Fn(&NewAt) -> bool| {
                (store.new_commit(id, at).into_iter()).map(|new| match new {
                    NewCommit::Made(pending) => Row::Item(pending),
                    NewCommit::Shown(title, slow, moved) => Row::Pending(
                        title,
                        slow,
                        moved.map(|key| RowKey::Commit(id.clone(), key)),
                    ),
                })
            };
            rows.extend(pending(
                &|at| matches!(at, NewAt::Top(ref_name) if branch.ref_name.as_ref() == Some(ref_name)),
            ));
            if let Some(moving) = landing_at(store, id, at_top) {
                rows.push_ghosts(moving, false);
            }
            // Commits a pending operation takes away go, unless selected and going nowhere
            // else, as a moved one shows twice otherwise; ones it makes show where they go.
            let commits: Vec<_> = (branch.commits.iter().enumerate())
                .filter(|&(at, commit)| {
                    !store.hides_commit(id, &commit.key)
                        || (selected_at == Some(at) && !store.moves_commit(id, &commit.key))
                })
                .map(|(_, commit)| commit)
                .collect();
            let count = commits.len();
            let commit_rows = |commits: &[&Commit], rows: &mut Rows| {
                for commit in commits {
                    rows.extend(pending(
                        &|at| matches!(at, NewAt::Above(key) if *key == commit.key),
                    ));
                    rows.push(Row::Item(Selection::Commit(id.clone(), commit.id)));
                    let below = |at: &Stop| matches!(at, Stop::In(Destination::BelowCommit(c)) if *c == commit.id);
                    if let Some(moving) = landing_at(store, id, below) {
                        rows.push_ghosts(moving, false);
                    }
                    rows.extend(pending(
                        &|at| matches!(at, NewAt::Below(key) if *key == commit.key),
                    ));
                }
            };
            let (first, rest) = commits.split_at(fold_at.min(count));
            commit_rows(first, rows);
            if !rest.is_empty() {
                rows.fold(Fold::More(key.clone()), unfolded, &self.folds, |rows| {
                    commit_rows(rest, rows)
                });
            }
            // Show fewer goes by the fold, not the selection, so selecting the last row keeps it.
            if count > shown {
                rows.push(Row::More(
                    id.clone(),
                    branch.ref_name.clone(),
                    count - shown,
                ));
            } else if unfolded && count > FOLD_AFTER {
                rows.push(Row::Fewer(id.clone(), branch.ref_name.clone()));
            }
            rows.close();
        };
        // A branch with no commits has nothing to fold.
        if branch.commits.is_empty() {
            return inside(rows);
        }
        let open = (store.selection.as_ref()).is_some_and(|s| s.project() == id && s.under(branch))
            || self.is_open(&ClickOpened::Here(key.clone()));
        let fold = Fold::Under(item);
        let shown = self.folds.shows(&fold, open);
        rows.fold(fold, open, &self.folds, inside);
        // Folded with a move at its top, stepped to or folded while there, as a click on its
        // chevron does, its commits stay out of sight, but not those landing there.
        if !shown && let Some(moving) = landing_at(store, id, at_top) {
            rows.open();
            rows.push_ghosts(moving, false);
            rows.close();
        }
    }

    /// A linked worktree's heading and, unless it's folded without the selection in them, its
    /// uncommitted changes and its branches; then a line ruling it off from what follows.
    fn worktree_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        worktree: &Worktree,
        selected: bool,
        rows: &mut Rows,
    ) {
        let name = worktree.name.clone();
        let heading = Selection::Checkout(id.clone(), Checkout::Linked(name.clone()));
        rows.push(Row::Item(heading.clone()));
        let open = self.shows_under(&heading, selected);
        rows.fold(Fold::Under(heading), open, &self.folds, |rows| {
            rows.open_in(STEP);
            self.linked_checkout_rows(store, id, &name, rows);
            rows.close();
        });
        rows.push(Row::WorktreeDivider);
    }

    /// What linked worktree `name`'s heading folds away: its uncommitted changes, its terminals,
    /// its branches and what they rest on.
    pub(super) fn linked_checkout_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        name: &bstr::BString,
        rows: &mut Rows,
    ) {
        let workspace = store.workspace(id);
        if let Some(workspace) = workspace {
            changes_row(store, workspace, id, &Checkout::Linked(name.clone()), rows);
        }
        terminal_rows(store, id, &Checkout::Linked(name.clone()), rows);
        let lane = workspace.and_then(|ws| ws.worktree_lane(name.as_ref()));
        self.lane_rows(store, id, lane.map_or(&[][..], |lane| &lane.branches), rows);
        // A worktree with no branch of its own says what it's on in its heading's meta instead.
        let lane = lane.filter(|lane| !lane.branches.is_empty());
        if let Some((workspace, lane)) = workspace.zip(lane) {
            rows.extend(
                rests_on(id, workspace, lane, listed(store, id))
                    .map(|(item, checkout)| Row::RestsOn(item, checkout)),
            );
        }
    }

    /// The heading over the commits only `ref_name`'s remote has, and when it's open each of
    /// them, newest first, as `but status` lists them. The target's new ones are listed on
    /// the project's page, not here.
    fn upstream_rows(
        &self,
        store: &Store,
        workspace: &Workspace,
        id: &ProjectId,
        ref_name: &FullName,
        rows: &mut Rows,
    ) {
        let commits = workspace.upstream(ref_name);
        if commits.is_empty() {
            return;
        }
        let owner = ref_name.clone();
        let section = Selection::Section(id.clone(), Section::Upstream(owner.clone()));
        rows.push(Row::Item(section.clone()));
        let open = self.upstream_open(store, id, ref_name);
        rows.fold(Fold::Under(section), open, &self.folds, |rows| {
            // Guides from the heading's glyph and an open merge's, as from a branch's.
            rows.open_at(UPSTREAM_GUIDE);
            let selected = match &store.selection {
                Some(Selection::Upstream(p, r, c)) if p == id && *r == owner => Some(*c),
                _ => None,
            };
            for commit in commits {
                let item = Selection::Upstream(id.clone(), owner.clone(), commit.id);
                rows.push(Row::Item(item.clone()));
                if !commit.merge {
                    continue;
                }
                // Its commits load as it opens.
                let merged = (store.merged.get(&commit.id))
                    .and_then(|m| m.as_ref())
                    .map(|(merged, _)| merged);
                let holds =
                    selected.is_some_and(|c| merged.is_some_and(|m| m.iter().any(|m| m.id == c)));
                let open = merged.is_some() && (self.open_merges.contains(&commit.id) || holds);
                rows.fold(Fold::Under(item), open, &self.folds, |rows| {
                    if let Some(merged) = merged {
                        rows.open_at(MERGE_GUIDE);
                        rows.extend(merged.iter().map(|merged| {
                            Row::Item(Selection::Upstream(id.clone(), owner.clone(), merged.id))
                        }));
                        rows.close();
                    }
                });
            }
            rows.close();
        });
    }
}

/// What linked worktree `name` is on when it has no branch of its own: the commit its HEAD is
/// at, by the branch it's in, which its heading's meta names.
pub(super) fn head_of(
    store: &Store,
    id: &ProjectId,
    workspace: &Workspace,
    name: &bstr::BStr,
) -> Option<Selection> {
    let lane = workspace.worktree_lane(name)?;
    if !lane.branches.is_empty() {
        return None;
    }
    rests_on(id, workspace, lane, listed(store, id))
        .next()
        .map(|(item, _)| item)
}

/// Whether `branch` is listed, neither hidden nor being deleted.
fn listed<'a>(store: &'a Store, id: &'a ProjectId) -> impl Fn(&Branch) -> bool + Copy + 'a {
    move |branch: &Branch| {
        !(branch.ref_name.as_ref())
            .is_some_and(|r| store.hides_branch(id, r) || store.deletes_branch(id, r))
    }
}

fn rests_on<'a>(
    id: &'a ProjectId,
    workspace: &'a Workspace,
    lane: &'a Lane,
    listed: impl Fn(&Branch) -> bool + Copy + 'a,
) -> impl Iterator<Item = (Selection, Option<Checkout>)> + 'a {
    workspace.beneath(lane).flat_map(move |link| {
        let mut checkout = Some(link.lane.checkout.clone());
        (link.branches().iter().enumerate())
            .filter(move |(_, branch)| listed(branch))
            .filter_map(move |(at, branch)| {
                let item = match (link.from.filter(|_| at == 0), &branch.ref_name) {
                    (Some(from), _) => Selection::Commit(id.clone(), from),
                    (None, Some(_)) => Selection::Branch(id.clone(), branch.ref_name.clone()),
                    (None, None) => Selection::Commit(id.clone(), branch.commits.first()?.id),
                };
                Some((item, checkout.take()))
            })
    })
}

/// When `entry` was last active: what Git last recorded in it, or what its loaded workspace
/// shows since, as files an agent is writing.
pub(super) fn last_active(entry: &ProjectEntry) -> Option<i64> {
    let loaded = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
    entry.project.last_active.max(loaded)
}

fn active_lately(at: i64) -> bool {
    crate::ui::now() - at <= ACTIVE_WITHIN.as_secs() as i64
}

/// Whether the selection's in one of project `id`'s `inactive` worktrees.
pub(super) fn in_inactive(store: &Store, id: &ProjectId, inactive: &[&Worktree]) -> bool {
    (store.selection.as_ref())
        .filter(|s| s.project() == id)
        .is_some_and(|s| {
            let workspace = store.workspace(id);
            (inactive.iter()).any(|wt| {
                let checkout = Checkout::Linked(wt.name.clone());
                matches!(s, Selection::Checkout(_, Checkout::Linked(name)) if *name == wt.name)
                    || workspace.is_some_and(|ws| in_checkout(store, s, ws, &checkout))
            })
        })
}

/// Whether project `id`'s `inactive` worktrees are listed above the row that folds them: once
/// it's opened, and while the selection's in one.
pub(super) fn inactive_open(store: &Store, id: &ProjectId, inactive: &[&Worktree]) -> bool {
    in_inactive(store, id, inactive) || store.prefs.shown_inactive.contains(id.as_ref())
}

/// Project `id`'s linked worktrees in `workspace`, in the order kept for them, but one being
/// moved, dragged by its heading or with the keys, where it'd land, among those listed with it
/// as they were when the move started; one added meanwhile goes after them.
fn worktrees_shown<'a>(
    store: &Store,
    id: &ProjectId,
    workspace: &'a Workspace,
) -> Vec<&'a Worktree> {
    let mut worktrees = store.worktrees_in_order(id, workspace);
    let shown = (store.moving.as_ref())
        .filter(|moving| moving.project == *id)
        .and_then(|moving| moving.worktree_order());
    let Some(shown) = shown else {
        return worktrees;
    };
    // Those listed with it trade the places they had among the rest.
    let listed = |wt: &&Worktree| shown.contains(&wt.name);
    let mut moved: Vec<&Worktree> = worktrees.iter().copied().filter(listed).collect();
    moved.sort_by_key(|wt| shown.iter().position(|name| *name == wt.name));
    let mut moved = moved.into_iter();
    for place in worktrees.iter_mut().filter(|wt| listed(wt)) {
        if let Some(wt) = moved.next() {
            *place = wt;
        }
    }
    worktrees
}

/// Project `id`'s linked worktrees the navigator lists: all but those an operation under way
/// takes away, in the order [`worktrees_shown`] gives but the active ones first, as the
/// inactive ones wait folded after them. The rows, the tabs across the top and
/// Go to anything all list them so.
pub(crate) fn worktrees_listed<'a>(
    store: &Store,
    id: &ProjectId,
    workspace: &'a Workspace,
) -> Vec<&'a Worktree> {
    let listed = (worktrees_shown(store, id, workspace).into_iter())
        .filter(|wt| !store.hides_worktree(id, wt.name.as_ref()))
        .collect();
    active_first(listed, store.prefs.inactive_after)
}

/// `worktrees` in their order, but the active ones before the inactive ones.
fn active_first(worktrees: Vec<&Worktree>, after: InactiveAfter) -> Vec<&Worktree> {
    let (active, inactive): (Vec<_>, Vec<_>) =
        (worktrees.into_iter()).partition(|wt| !wt.is_inactive(after));
    active.into_iter().chain(inactive).collect()
}

/// The linked worktrees of `project` listed with `name`, as they're listed now: the active
/// ones, or the inactive ones folded after them, which it can't leave.
pub(super) fn listed_with(
    store: &Store,
    project: &ProjectId,
    name: &bstr::BStr,
) -> Option<Arc<[bstr::BString]>> {
    let workspace = store.workspace(project)?;
    let after = store.prefs.inactive_after;
    let inactive = workspace.find_worktree(name)?.is_inactive(after);
    let listed = (worktrees_listed(store, project, workspace).into_iter())
        .filter(|wt| wt.is_inactive(after) == inactive)
        .map(|wt| wt.name.clone());
    Some(listed.collect())
}

/// The checkout `row` heads, `next` being the row after it: a worktree's heading's, or the main
/// one's for a project's row while it stands in for that heading, its card's rows following
/// without one.
pub(super) fn heads(row: &Row, next: Option<&Row>) -> Option<Checkout> {
    match (row, next) {
        (Row::Item(Selection::Checkout(_, checkout)), _) => Some(checkout.clone()),
        (Row::Item(Selection::Project(p)), Some(Row::Item(next)))
            if next.project() == p && !matches!(next, Selection::Checkout(..)) =>
        {
            Some(Checkout::Main)
        }
        (Row::Item(Selection::Project(_)), Some(Row::HeaderRule(true))) => Some(Checkout::Main),
        _ => None,
    }
}

/// Whether `selection` is in `checkout`, which folding its heading hides: its uncommitted
/// changes, a branch, commit or stack in its lane, or a terminal open on it.
pub(super) fn in_checkout(
    store: &Store,
    selection: &Selection,
    workspace: &Workspace,
    checkout: &Checkout,
) -> bool {
    *selection == Selection::Uncommitted(selection.project().clone(), checkout.clone())
        || within(selection, workspace.branches_in(checkout))
        || store.terminals.checkout_of(selection) == Some(checkout)
}

/// Whether `project`'s `checkout`'s uncommitted changes are listed, first under its heading:
/// only while there are some, as with none there's nothing to show or commit.
pub(crate) fn lists_changes(
    store: &Store,
    workspace: &Workspace,
    project: &ProjectId,
    checkout: &Checkout,
) -> bool {
    (workspace.uncommitted_in(checkout))
        .is_some_and(|files| !store.uncommitted_shown(project, checkout, files).is_empty())
}

/// Whether anything is listed under `project`'s `checkout`'s heading while it's open: its
/// changes, a terminal, a branch or what its branches rest on. Without any, the heading
/// doesn't fold, like a branch without commits.
pub(super) fn checkout_holds(
    store: &Store,
    workspace: &Workspace,
    project: &ProjectId,
    checkout: &Checkout,
) -> bool {
    lists_changes(store, workspace, project, checkout)
        || store.terminals.on(project, checkout).next().is_some()
        || (workspace.lanes_in(checkout)).any(|lane| !lane.branches.is_empty())
}

/// `project`'s `checkout`'s uncommitted changes' row, while [`lists_changes`] says it's listed.
fn changes_row(
    store: &Store,
    workspace: &Workspace,
    project: &ProjectId,
    checkout: &Checkout,
    rows: &mut Rows,
) {
    if lists_changes(store, workspace, project, checkout) {
        rows.push(Row::Item(Selection::Uncommitted(
            project.clone(),
            checkout.clone(),
        )));
    }
}

/// The terminals open on `project`'s `checkout`, first under its heading.
fn terminal_rows(store: &Store, project: &ProjectId, checkout: &Checkout, rows: &mut Rows) {
    let open = store.terminals.on(project, checkout).map(|(id, _)| id);
    // One being moved goes where it'd land, from its checkout to another too, of this project
    // or another's, the others making room; one opened since goes after them.
    let moved = (store.moving.as_ref()).and_then(|moving| moving.terminal_order());
    let terminals: Vec<TerminalId> = match &moved {
        Some(lists) => {
            let listed = |id: &TerminalId| lists.iter().any(|(_, list)| list.contains(id));
            let here =
                (lists.iter()).find(|(on, _)| on.project == *project && on.checkout == *checkout);
            let kept = here.into_iter().flat_map(|(_, list)| list.iter().copied());
            let kept = kept.filter(|id| store.terminals.get(*id).is_some());
            kept.chain(open.filter(|id| !listed(id))).collect()
        }
        None => open.collect(),
    };
    rows.extend(
        terminals
            .into_iter()
            .map(|id| Row::Item(Selection::Terminal(project.clone(), id))),
    );
}

/// Whether `selection` is one of `branches`, a commit on one, or a stack of them.
pub(super) fn within<'a>(
    selection: &Selection,
    mut branches: impl Iterator<Item = &'a Branch>,
) -> bool {
    branches.any(|branch| {
        selection.on(branch)
            || matches!(selection, Selection::Stack(_, bottom) if *bottom == branch.ref_name)
    })
}

/// Whether what's `dragging` lands in the zone under `project`'s stacks: commits, in a new
/// branch of their own. Outside the workspace, landing there sets it up, as `but move` does. A
/// branch goes between the stacks instead, where it would be one of its own.
fn lands_apart(dragging: Option<&Dragged>, project: &ProjectId) -> bool {
    let Some(dragged) = dragging.filter(|dragged| dragged.project == *project) else {
        return false;
    };
    match &dragged.what {
        transfer::Moving::Commits(_) => true,
        transfer::Moving::Branch(_)
        | transfer::Moving::Project(_)
        | transfer::Moving::Worktree { .. }
        | transfer::Moving::Terminal { .. }
        | transfer::Moving::Files(_)
        | transfer::Moving::Copies(_) => false,
    }
}

/// Which of `terms`, lowercase, any of `texts` holds, as bits by the term's place.
pub(super) fn found(terms: &[String], texts: &[&str]) -> u64 {
    (terms.iter().enumerate())
        .filter(|(_, term)| texts.iter().any(|text| holds(text, term)))
        .fold(0, |bits, (ix, _)| bits | 1 << ix)
}

/// Whether a row whose own text has the terms in `own`, under rows with those in `held`,
/// matches: every term found, and one at least in its own text.
pub(super) fn matches(terms: &[String], held: u64, own: u64) -> bool {
    own != 0 && held | own == u64::MAX >> (64 - terms.len())
}

/// Whether `text` holds `term` (lowercase), in any case; without allocating for ASCII text,
/// as most names and titles are, since the filter looks through every commit on each key.
fn holds(text: &str, term: &str) -> bool {
    match text.is_ascii() {
        true => (text.as_bytes().windows(term.len()))
            .any(|window| window.eq_ignore_ascii_case(term.as_bytes())),
        false => text.to_lowercase().contains(term),
    }
}

#[cfg(test)]
mod active_first_tests {
    use super::active_first;
    use crate::backend::{InactiveAfter, Worktree};

    /// Worktree `name`, last moved at `updated_at`, with nothing uncommitted.
    fn worktree(name: &str, updated_at: Option<i64>) -> Worktree {
        Worktree {
            name: name.into(),
            path: name.into(),
            branch: None,
            uncommitted: Vec::new(),
            git_operation: None,
            tree: None,
            updated_at,
        }
    }

    #[test]
    fn inactive_worktrees_go_after_the_active_ones() {
        let (old, new) = (worktree("old", Some(0)), worktree("new", None));
        let (older, newer) = (worktree("older", Some(0)), worktree("newer", None));
        let listed = active_first(
            Vec::from([&old, &new, &older, &newer]),
            InactiveAfter::TwoDays,
        );
        let names: Vec<String> = listed.iter().map(|wt| wt.name.to_string()).collect();
        assert_eq!(
            names,
            ["new", "newer", "old", "older"],
            "each keeps its place among its own, as the rows list the inactive folded after the rest"
        );
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{found, holds, matches};

    #[test]
    fn a_row_matches_with_what_holds_it() {
        let terms = ["shop".to_owned(), "cart".to_owned()];
        let project = found(&terms, &["shop"]);
        assert!(
            !matches(&terms, 0, project),
            "the project alone lacks \"cart\""
        );
        let branch = found(&terms, &["feature/Cart"]);
        assert!(
            matches(&terms, project, branch),
            "the branch has \"cart\", in any case, and its project \"shop\""
        );
        let commit = found(&terms, &["Keep items in order"]);
        assert!(
            !matches(&terms, project | branch, commit),
            "a commit needs a word of its own, not only its branch's"
        );
        assert!(
            holds("Über die Karte", "über"),
            "non-ASCII text folds case too"
        );
    }
}

#[cfg(test)]
mod within_tests {
    use super::within;
    use crate::backend::model::build::full;
    use crate::backend::{Branch, CommitId, PushState};
    use crate::state::{Section, Selection};

    #[test]
    fn a_commit_only_the_remote_has_is_within_its_branch() {
        let branch = Branch {
            name: "feature".into(),
            ref_name: Some(full("refs/heads/feature")),
            commits: Vec::new(),
            incoming: Vec::new(),
            remote: Some("origin/feature".into()),
            push: PushState::NothingToPush,
            review: None,
        };
        let upstream = |ref_name: &str| {
            let id = CommitId(gix::ObjectId::from([1; 20]));
            Selection::Upstream("p".into(), full(ref_name), id)
        };
        assert!(
            within(&upstream("refs/heads/feature"), [&branch].into_iter()),
            "folding the branch's worktree or stack must keep its selected remote commit"
        );
        assert!(
            !within(&upstream("refs/heads/other"), [&branch].into_iter()),
            "another branch's remote commit is elsewhere"
        );
        let heading = Selection::Section("p".into(), Section::Upstream(full("refs/heads/feature")));
        assert!(
            within(&heading, [&branch].into_iter()),
            "and so must the heading over its remote's commits"
        );
    }
}

#[cfg(test)]
mod lands_apart_tests {
    use super::lands_apart;
    use crate::backend::model::build::{full, id};
    use crate::state::ProjectId;
    use crate::transfer::{Dragged, Moving};

    #[test]
    fn commits_land_in_the_zone_and_a_branch_between_the_stacks() {
        let project = ProjectId::from("p");
        let commit = Dragged {
            project: project.clone(),
            what: Moving::Commits([id(3)].into()),
        };
        assert!(
            lands_apart(Some(&commit), &project),
            "commits go in a branch of their own there"
        );
        assert!(
            !lands_apart(Some(&commit), &ProjectId::from("other")),
            "they land only in their own project"
        );
        let branch = Dragged {
            project: project.clone(),
            what: Moving::Branch(full("refs/heads/ui")),
        };
        assert!(
            !lands_apart(Some(&branch), &project),
            "a branch is a stack of its own between the others, where it lands"
        );
    }
}

#[cfg(test)]
mod row_key_tests {
    use std::collections::HashMap;

    use super::item_key;
    use crate::state::Selection;
    use crate::terminal::TerminalId;
    use crate::ui::fold::reordered;

    #[test]
    fn a_terminal_moving_to_another_project_slides_there() {
        let terminal = |project: &str, id| {
            let key = item_key(
                &Selection::Terminal(project.into(), TerminalId(id)),
                &HashMap::new(),
            );
            (Some(key), 28.)
        };
        let old = [terminal("alpha", 1), terminal("beta", 2)];
        let new = [terminal("beta", 2), terminal("beta", 1)];
        let (moved, _) = terminal("beta", 1);
        let slides = reordered(&old, &new);
        assert_eq!(
            moved.and_then(|key| slides.get(&key).copied()),
            Some(-28.),
            "it starts from where it was listed under the other project"
        );
    }
}

#[cfg(test)]
mod rests_on_tests {
    use super::rests_on;
    use crate::backend::Branch;
    use crate::backend::model::build::{branch, commit_name, workspace, worktree};
    use crate::state::Selection;

    #[test]
    fn a_worktree_lists_each_branch_beneath_it_once() {
        let unnamed = Branch {
            name: "Unnamed branch".into(),
            ..branch("", &[9])
        };
        let ws = workspace(
            vec![
                vec![
                    branch("top", &[1]),
                    branch("middle", &[2, 3]),
                    branch("bottom", &[4]),
                ],
                vec![unnamed, branch("base", &[10])],
            ],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[5])]),
                worktree("wt-stacked", Some(5), vec![branch("wt-stacked", &[6])]),
                worktree("on-unnamed", Some(9), vec![]),
                worktree("outside", None, vec![branch("outside", &[7])]),
            ],
        );
        let id = "p".into();
        let rows: String = (ws.lanes.iter().filter(|lane| lane.checkout.is_linked()))
            .map(|lane| {
                let rows = (rests_on(&id, &ws, lane, |b| b.name != "base"))
                    .map(|(row, checkout)| {
                        let row = match row {
                            Selection::Branch(_, ref_name) => {
                                format!(
                                    "Branch({})",
                                    ref_name
                                        .as_ref()
                                        .map(ToString::to_string)
                                        .unwrap_or_default()
                                )
                            }
                            Selection::Commit(_, at) => {
                                format!("Commit({})", commit_name(&ws, at))
                            }
                            other => format!("{other:?}"),
                        };
                        let checkout = checkout.map_or(String::new(), |checkout| {
                            format!(" in {}", checkout.linked().unwrap_or("Main".into()))
                        });
                        format!("{row}{checkout}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: [{rows}]\n", lane.checkout.linked().unwrap_or_default())
            })
            .collect();
        snapbox::assert_data_eq!(
            rows,
            snapbox::str![[r#"
wt: [Commit(middle~1) in Main, Branch(refs/heads/bottom)]
wt-stacked: [Branch(refs/heads/wt) in wt, Commit(middle~1) in Main, Branch(refs/heads/bottom)]
on-unnamed: [Commit(Unnamed branch) in Main]
outside: []

"#]]
        );
    }
}
