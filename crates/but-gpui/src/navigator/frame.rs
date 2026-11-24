//! What a frame does with the navigator's rows before they're drawn, in its list or in the
//! card under a tab: going on to the frame, building the rows, following their folds, and
//! bringing the list up to date.

use std::collections::HashMap;
use std::ops::Range;

use gpui::{App, ListOffset, ListState, Pixels, Window};

use super::cursor::OffRow;
use super::rows::{Drawer, Fold, Moving, Row, RowKey, Rows, row_keys, span_height};
use super::{ABOVE_LAST_LISTED, LIST_TOP, Navigator, holder, moving, shows_where_it_lands};
use crate::backend::Checkout;
use crate::clock;
use crate::state::{ProjectId, Selection};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::transfer::{Op, Stop};
use crate::ui::fold;

/// One build of the rows and what's worked out from them. The card under a tab has one of its
/// own while it's drawn or the keys act on it ([`Navigator::in_scope`]), so whatever is added
/// here, the card has as the list does.
#[derive(Default)]
pub(super) struct Built {
    /// The rows as last drawn, which the list draws as they scroll into view.
    pub(super) rows: Vec<Row>,
    pub(super) insets: Vec<f32>,
    pub(super) guides: Vec<Vec<f32>>,
    /// Whether each row's innermost guide ends at it: see [`Rows::ends`].
    pub(super) ends: Vec<bool>,
    /// Each row's place in the fold it moves with, while one does.
    pub(super) moving: Vec<Option<Moving>>,
    /// Each row's key, to tell which rows a reorder moved.
    pub(super) row_keys: Vec<Option<RowKey>>,
    /// The projects not watched as last drawn.
    pub(super) drawer: Drawer,
    /// The rows of the card being moved, dragged by its heading or with the keys, as last
    /// drawn, which show it held.
    pub(super) held_card: Option<Range<usize>>,
    /// The rows of the card the selection's in, while the navigator has the keys, which edge
    /// it as the tile with them is: see [`Navigator::find_focused_card`].
    pub(super) focused_card: std::cell::RefCell<Option<Range<usize>>>,
    /// The rows of the drawer's card the selection's in, edged as [`Self::focused_card`] is.
    pub(super) focused_drawer_card: std::cell::RefCell<Option<Range<usize>>>,
    /// The rows of the linked worktree being moved, dragged by its heading or with the keys, as
    /// last drawn, which go over the rows they pass.
    pub(super) held_worktree: Option<Range<usize>>,
}

/// What draws the rows this frame, which decides what else the frame does with them: see
/// [`Navigator::prepare`].
#[derive(Clone, Copy)]
pub(super) enum RowsIn {
    /// The navigator's own list, beside the detail or out over it.
    List,
    /// The card under a tab across the top, where the navigator itself isn't drawn.
    Card,
}

/// Tell `list` its rows from `at` changed from `old` to `new`: the ones between what both
/// start and end with, which it measures again. The rows of `old` they were, if any changed.
pub(super) fn splice(
    list: &ListState,
    old: &[Row],
    new: &[Row],
    at: usize,
) -> Option<Range<usize>> {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (
        old.get(head..).unwrap_or_default(),
        new.get(head..).unwrap_or_default(),
    );
    let tail = (old.iter().rev().zip(new.iter().rev()))
        .take_while(|(a, b)| a == b)
        .count();
    let changed = old.len() > tail || new.len() > tail;
    let spliced = at + head..at + head + old.len() - tail;
    changed.then(|| {
        list.splice(spliced.clone(), new.len() - tail);
        spliced
    })
}

pub(super) fn at_top(list: &ListState) -> bool {
    let top = list.logical_scroll_top();
    top.item_ix == 0 && top.offset_in_item == Pixels::ZERO
}

/// Whether `new` stands where `old` did, to keep it under the pointer: the same row, or the
/// other side of a fold, as "Show fewer" becomes "N more".
fn same_place(old: &Row, new: &Row) -> bool {
    match (old, new) {
        (Row::Fewer(project, ref_name), Row::More(p, r, _)) => project == p && ref_name == r,
        _ => old == new,
    }
}

/// Whether `old` and `new` list the same cards, and the same linked worktrees in them, in
/// another order.
fn headings_reordered(old: &[Row], new: &[Row]) -> bool {
    fn headings(rows: &[Row]) -> Vec<&Selection> {
        (rows.iter())
            .filter_map(|row| match row {
                Row::Item(
                    heading @ (Selection::Project(_) | Selection::Checkout(_, Checkout::Linked(_))),
                ) => Some(heading),
                _ => None,
            })
            .collect()
    }
    let (old, new) = (headings(old), headings(new));
    old != new && old.len() == new.len() && old.iter().all(|heading| new.contains(heading))
}

/// Where the list's `top` in `old` goes in `new`, their rows the same up to `from`: as far
/// down past `from`, as the rows count their heights ([`span_height`]).
fn as_deep(
    old: &[Row],
    new: &[Row],
    from: usize,
    top: ListOffset,
    theme: &Theme,
    row: f32,
) -> ListOffset {
    let depth =
        span_height(old, from..top.item_ix, row) + f32::from(top.offset_in_item) / theme.scale;
    let mut above = 0.;
    for (ix, item) in new.iter().enumerate().skip(from) {
        let height = item.height(row);
        if above + height > depth {
            return ListOffset {
                item_ix: ix,
                offset_in_item: theme.scaled(depth - above),
            };
        }
        above += height;
    }
    ListOffset {
        item_ix: new.len(),
        offset_in_item: Pixels::ZERO,
    }
}

/// Whether going from `old` to `new` only lists or unlists checkouts' uncommitted changes, or
/// the zone for a new branch, which a drag brings before it's a move.
fn room_came_or_went(old: &[Row], new: &[Row]) -> bool {
    fn others(rows: &[Row]) -> impl Iterator<Item = &Row> {
        (rows.iter()).filter(|row| {
            !matches!(
                row,
                Row::Item(Selection::Uncommitted(..)) | Row::NewBranch(_)
            )
        })
    }
    old.len() != new.len() && others(old).eq(others(new))
}

/// The terminal `new` lists under another project than `old` does, if one is.
fn crossed_cards(old: &[Row], new: &[Row]) -> Option<TerminalId> {
    let terminals = |rows: &'_ [Row]| -> HashMap<TerminalId, ProjectId> {
        (rows.iter())
            .filter_map(|row| match row {
                Row::Item(Selection::Terminal(project, id)) => Some((*id, project.clone())),
                _ => None,
            })
            .collect()
    };
    let was = terminals(old);
    (terminals(new).into_iter())
        .find(|(id, project)| was.get(id).is_some_and(|from| from != project))
        .map(|(id, _)| id)
}

/// Record how each fold one build of the rows went through is now, as `seen` says, and give
/// those that opened or shut since `record` last had them. One seen for the first time is
/// recorded as it is. Across the top, the rows of each card drawn and of the selection's tab
/// are built each frame, each with folds of its own, so the folds a build didn't go through
/// keep what another recorded.
fn folds_changed<'a>(
    record: &mut HashMap<Fold, bool>,
    seen: &'a [(Fold, bool, Option<usize>)],
) -> Vec<&'a (Fold, bool, Option<usize>)> {
    (seen.iter())
        .filter(|(fold, open, _)| match record.get_mut(fold) {
            Some(was) if was == open => false,
            Some(was) => {
                *was = *open;
                true
            }
            None => {
                record.insert(fold.clone(), *open);
                false
            }
        })
        .collect()
}

impl Navigator {
    /// Go on to the frame being drawn and bring the rows up to date for it, whatever draws
    /// them, saying whether they go on folding or sliding after it. One frame's work for both,
    /// so the card under a tab does what the list does but what only a list can.
    pub(super) fn prepare(&mut self, rows_in: RowsIn, window: &Window, cx: &App) -> bool {
        self.row = Theme::get(cx).sizes.control;
        self.next_frame();
        // As the list drew it, before the rows change, which a reorder doesn't do by much.
        let last_listed = match rows_in {
            RowsIn::List => {
                // The card says where its first row is as it draws it, and isn't drawn.
                self.card_top.set(None);
                let inset = Theme::get(cx).scaled(LIST_TOP);
                (self.list.logical_scroll_top().item_ix..self.built.rows.len())
                    .take_while(|ix| fold::listed(&self.list, inset, *ix))
                    .last()
            }
            RowsIn::Card => None,
        };
        self.catch_up_rows(window, cx);
        self.built.held_card = self.held_card(self.store.read(cx));
        self.built.held_worktree = self.held_worktree(self.store.read(cx));
        self.end_crossing();
        match rows_in {
            RowsIn::List => {
                self.slide_edges = self.reorder.any().then(|| {
                    let first = self.list.logical_scroll_top().item_ix;
                    let below =
                        last_listed.map_or(first, |last| last.saturating_sub(ABOVE_LAST_LISTED));
                    (first, below.max(first))
                });
                self.fit_folds(cx);
            }
            // A card holds all its rows in view, so they slide and fold all the way.
            RowsIn::Card => {}
        }
        self.folds.any() || self.reorder.any()
    }

    /// Which of the card's rows to scroll to, once the selection changed or where it'd be
    /// moved did, as the list scrolls to it as it brings its rows up to date.
    pub(super) fn take_card_reveal(&mut self, cx: &App) -> Option<usize> {
        self.reveal_source = false;
        std::mem::take(&mut self.reveal_selection)
            .then(|| self.card_reveal(cx))
            .flatten()
    }

    /// Which row a reveal goes to in the card: where what's moving lands, else the selection.
    fn card_reveal(&self, cx: &App) -> Option<usize> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        (self
            .built
            .rows
            .iter()
            .position(|row| shows_where_it_lands(store, row)))
        .or_else(|| {
            (self.built.rows.iter()).position(|row| matches!(row, Row::Item(it) if it == selection))
        })
    }

    /// Go on to the frame being drawn, here or in the card under a tab: what folds and slides,
    /// and what stands in for rows that went, which fades as they close over it.
    fn next_frame(&mut self) {
        self.folds.frame();
        self.reorder.frame();
        let now = clock::now();
        self.stand_ins.retain_mut(|stand_in| stand_in.fade(now));
    }

    /// With the rows at rest, a terminal crossing to another card has got there.
    fn end_crossing(&mut self) {
        if !self.reorder.any() {
            self.crossing = None;
        }
    }

    /// Build the rows and bring the list up to date, as [`Self::prepare`] does for a frame but
    /// without going on to one, for what reads them while nothing draws them.
    pub(crate) fn catch_up_rows(&mut self, window: &Window, cx: &App) {
        let Rows {
            rows,
            insets,
            guides,
            ends,
            moving,
            drawer,
            first_hit,
            ..
        } = self.rows_now(cx);
        self.built.moving = moving;
        self.sync_list(rows, drawer, window, cx);
        self.built.insets = insets;
        self.built.guides = guides;
        self.built.ends = ends;
        self.first_hit = first_hit;
    }

    /// The rows as they're built now, any fold that opened or shut since they were last built
    /// set moving first, which changes what they hold.
    pub(super) fn rows_now(&mut self, cx: &App) -> Rows {
        let built = self.build_rows(cx);
        match self.follow_folds(&built.folds, cx) {
            true => self.build_rows(cx),
            false => built,
        }
    }

    /// Set moving the folds `seen` building the rows that opened or shut since they were last
    /// built ([`folds_changed`]), saying whether any did. Only one whose rows start on screen
    /// moves: elsewhere it would move only what's in view. Under the filter, which lists what it
    /// finds whatever's folded, none do.
    fn follow_folds(&mut self, seen: &[(Fold, bool, Option<usize>)], cx: &App) -> bool {
        if self.filtering(cx) {
            self.folds_seen.clear();
            self.folds.stop_all();
            return false;
        }
        // Where the rows above them were drawn, which is where they'll be. A card's rows are
        // all in view, and aren't in the list here, which isn't drawn while they're a card's.
        let in_card = self.rows_scope(cx).is_some();
        let list = &self.list;
        let view = list.viewport_bounds();
        let on_screen = |above: Option<usize>| {
            in_card
                || above.is_none_or(|ix| {
                    (list.bounds_for_item(ix)).is_some_and(|row| {
                        row.bottom() > view.top() && row.bottom() < view.bottom()
                    })
                })
        };
        let mut started = false;
        for (fold, open, above) in folds_changed(&mut self.folds_seen, seen) {
            if on_screen(*above) {
                self.folds.start(fold.clone(), *open, cx);
                started = true;
            }
        }
        started
    }

    /// Have each moving fold move only as far as the list shows it, from the row above it as
    /// last drawn: the rows past the list's bottom go or come at once, so what's seen moves
    /// all the way. Here, as a row can't ask the list while it draws.
    fn fit_folds(&mut self, cx: &App) {
        if !self.folds.any() {
            return;
        }
        let (list, scale) = (&self.list, Theme::get(cx).scale);
        let view = list.viewport_bounds();
        let room = |above: Option<usize>| {
            (above.and_then(|ix| list.bounds_for_item(ix)))
                .map(|row| f32::from(view.bottom() - row.bottom()).max(0.) / scale)
        };
        let mut rooms = HashMap::new();
        for m in self.built.moving.iter_mut().flatten() {
            let room = *rooms.entry(m.above).or_insert_with(|| room(m.above));
            if let Some(room) = room {
                m.extent = m.extent.min(room);
            }
        }
    }

    /// Bring the lists up to date with `rows` and `drawer`, and show the selection if it just
    /// changed. Rows that didn't change keep their place on screen. When a load changed the
    /// cards' while the pointer is over them, so does the row under it: rows added above it,
    /// like an agent's new commit, push the rows above it up rather than it down.
    fn sync_list(&mut self, rows: Vec<Row>, drawer: Drawer, window: &Window, cx: &App) {
        // Rows not drawn yet count as one row tall, which a project's row is.
        let row = Theme::get(cx).scaled(self.row);
        // At the very top a list stays there, showing what's added, as the diff does: rows
        // added above the first one count as added above the top, which would leave it
        // scrolled to their end, as the drawer's rows are when it opens.
        let drawer_at_top = at_top(&self.drawer_list);
        if splice(&self.drawer_list, &self.built.drawer.rows, &drawer.rows, 0).is_some() {
            self.drawer_list.clone().with_uniform_item_height(row);
            if drawer_at_top {
                self.drawer_list.scroll_to(ListOffset::default());
            }
        }
        self.built.drawer = drawer;
        // Scrolled, the row under the pointer stays where it is.
        let at_top = at_top(&self.list);
        let hovered = (std::mem::take(&mut self.hold_still) && !at_top)
            .then(|| self.hovered_row(window))
            .flatten();
        // The hovered row's index before and after, and how far below the top it is.
        let anchor = hovered.and_then(|(old_ix, offset)| {
            let old = self.built.rows.get(old_ix)?;
            let (new_ix, _) = (rows.iter().enumerate())
                .filter(|(_, row)| same_place(old, row))
                .min_by_key(|(ix, _)| ix.abs_diff(old_ix))?;
            Some((old_ix, new_ix, offset))
        });
        let halves = anchor.and_then(|(old_ix, new_ix, _)| {
            Some((
                self.built.rows.split_at_checked(old_ix)?,
                rows.split_at_checked(new_ix)?,
            ))
        });
        let top = self.list.logical_scroll_top();
        let changed = match halves {
            // Below the hovered row first, so the indexes above it still hold.
            Some(((old_above, old_below), (new_above, new_below))) => {
                let below = splice(&self.list, old_below, new_below, old_above.len());
                splice(&self.list, old_above, new_above, 0).is_some() || below.is_some()
            }
            None => {
                let spliced = splice(&self.list, &self.built.rows, &rows, 0);
                // Cards or linked worktrees trading places, as one dragged by its heading does,
                // leave the list where it is, rather than at the first row they moved, where a
                // splice of the list's top would.
                if let Some(old) = &spliced
                    && old.contains(&top.item_ix)
                    && headings_reordered(&self.built.rows, &rows)
                {
                    let theme = Theme::get(cx);
                    (self.list).scroll_to(as_deep(
                        &self.built.rows,
                        &rows,
                        old.start,
                        top,
                        theme,
                        self.row,
                    ));
                }
                spliced.is_some()
            }
        };
        if at_top && changed {
            self.list.scroll_to(ListOffset::default());
        }
        // While something's being moved or landing, and as that ends, every row slides from
        // where it was, so the room the copies take or leave opens and closes rather than jumps.
        let store = self.store.read(cx);
        let op = (store.moving.as_ref().or_else(|| store.landing())).map(|moving| moving.op);
        let was = std::mem::replace(&mut self.shown_op, op);
        let every = op.is_some() || was.is_some();
        // A squash just ended: landed, what it took goes.
        let ended = matches!(was, Some(Op::Squash(_))) && store.moving.is_none();
        // A move Return landed, until the reload after it, and as that shows where it went. Not
        // a cherry-pick, whose commits stay where they are, as when a move ends without landing,
        // while the copies shown give way to the new commits in their place.
        let leaving = (store.landing()).is_some_and(|landing| landing.op != Op::CherryPick);
        let landing = std::mem::replace(&mut self.shown_landing, leaving) || leaving;
        // The outline of what a squash goes into, or of the branch dragged files start a commit
        // in, slides there from where it was before, first
        // from around what it's of where it started, its edges coming together as it leaves
        // that, and going apart as it grows back around it.
        let home = (store.moving.as_ref()).is_some_and(|moving| {
            !moving.dragged(store)
                && matches!(moving.at, Stop::Into(_))
                && !moving.shows_landing(store)
        });
        let outline = moving::SquashOutline::of(&rows, store, home, self.row);
        if let (Some(was), Some(is)) = (&self.squash_outline, &outline)
            && was.row != is.row
        {
            let (top, bottom) = is.slide(was);
            let offsets = HashMap::from([(RowKey::SquashTop, top), (RowKey::SquashBottom, bottom)]);
            self.reorder.start(offsets, cx);
        }
        self.squash_outline = outline;
        // While a move's shown, the rows' keys change with it where the rows don't, as what's
        // moving is keyed apart as the move starts and ends.
        let reordered = self.built.rows != rows;
        let keys = (reordered || every)
            .then(|| row_keys(&rows, self.store.read(cx)))
            .filter(|keys| reordered || *keys != self.built.row_keys);
        if let Some(keys) = keys {
            let held = anchor.map(|(old_ix, new_ix, _)| (old_ix, new_ix));
            // Those still being closed over stay while the rows they're under do, as when the
            // reload after a move lands partway.
            let mut stand_ins = std::mem::take(&mut self.stand_ins);
            stand_ins.retain(|stand_in| keys.iter().flatten().any(|key| *key == stand_in.under));
            let store = self.store.read(cx);
            let taken = self.stand_ins(&keys, was, (ended, landing), clock::now(), store);
            stand_ins.extend(taken);
            self.stand_ins = stand_ins;
            // Room made or left above the list's top moves nothing in view, as the list keeps
            // its top row where it is, so nothing slides.
            let room = room_came_or_went(&self.built.rows, &rows);
            let above_view = room && top.item_ix != self.list.logical_scroll_top().item_ix;
            let offsets = match above_view {
                true => HashMap::new(),
                false => self.slides(&rows, &keys, held, every || room, landing),
            };
            self.reorder.start(offsets, cx);
            if let Some(id) = crossed_cards(&self.built.rows, &rows) {
                self.crossing = Some(RowKey::Terminal(id));
            }
            self.built.row_keys = keys;
        }
        self.built.rows = rows;
        // The bar goes with the rows that made it a stop, or once something moves.
        if self.off_row == Some(OffRow::DrawerBar)
            && !(self.drawer_toggles(cx) && self.store.read(cx).moving.is_none())
        {
            self.off_row = None;
        }
        if changed {
            // The clone shares the list's state.
            self.list.clone().with_uniform_item_height(row);
            if let Some((_, item_ix, offset)) = anchor {
                self.list.scroll_to(ListOffset {
                    item_ix,
                    offset_in_item: Pixels::ZERO,
                });
                self.list.scroll_by(-offset);
            }
        }
        let store = self.store.read(cx);
        let ghost = |row: &Row| shows_where_it_lands(store, row);
        // Not drawn yet, the list has no height to scroll a row into, which would leave the row
        // just above its top, so what's to show waits for the frame after its first.
        let drawn = self.list_drawn();
        if drawn
            && std::mem::take(&mut self.reveal_selection)
            && let Some(selection) = store.selection.clone()
        {
            let selected = |row: &Row| matches!(row, Row::Item(it) if *it == selection);
            if std::mem::take(&mut self.reveal_source) {
                self.scroll_to_row(selected, row);
            }
            if !self.scroll_to_row(ghost, row) && !self.scroll_to_row(selected, row) {
                // A project just added has its rows once it's loaded.
                self.reveal_selection =
                    (store.project(selection.project())).is_some_and(|entry| entry.is_loading())
            }
        }
        // Up from where Go to anything goes, as ⇧← steps, to the first row listed.
        let going_row = (self.going_to.clone()).and_then(|to| {
            std::iter::successors(Some(to), |item| holder(store, item))
                .find(|item| self.listed(item))
        });
        if drawn && going_row != self.going_row {
            if let Some(item) = &going_row {
                self.scroll_to_row(|row| matches!(row, Row::Item(it) if it == item), row);
            }
            self.going_row = going_row;
        }
    }

    /// Whether the list has been drawn, which gives it its height.
    pub(super) fn list_drawn(&self) -> bool {
        self.list.viewport_bounds().size.height > Pixels::ZERO
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Fold, Row, folds_changed, headings_reordered, room_came_or_went};
    use crate::backend::Checkout;
    use crate::state::{ProjectId, Selection};

    #[test]
    fn a_fold_shut_between_builds_of_other_rows_is_a_change() {
        let id = ProjectId::from("p");
        let worktree = Fold::Under(Selection::Checkout(id.clone(), Checkout::Main));
        let project = Fold::Under(Selection::Project(id));
        let mut record = HashMap::new();
        // Across the top, a project's card and the selection's worktree tab are built in turn,
        // each with folds of its own.
        folds_changed(&mut record, &[(worktree.clone(), true, None)]);
        folds_changed(&mut record, &[(project.clone(), true, None)]);
        let shut = [(worktree.clone(), false, None)];
        let changed = folds_changed(&mut record, &shut);
        assert!(
            matches!(changed.as_slice(), [(fold, false, _)] if *fold == worktree),
            "the worktree's fold shut since its card was last built"
        );
        assert!(
            folds_changed(&mut record, &[(project, true, None)]).is_empty(),
            "the project's fold is as its card last saw it"
        );
    }

    #[test]
    fn a_fold_seen_for_the_first_time_is_no_change() {
        let fold = Fold::Under(Selection::Project(ProjectId::from("p")));
        let mut record = HashMap::new();
        assert!(
            folds_changed(&mut record, &[(fold, false, None)]).is_empty(),
            "a fold that wasn't built before comes as it is"
        );
    }

    #[test]
    fn a_drag_s_zone_coming_or_going_makes_room_as_changes_do() {
        let id = ProjectId::from("a");
        let card = || Row::Item(Selection::Project(id.clone()));
        let worktree = || {
            Row::Item(Selection::Checkout(
                id.clone(),
                Checkout::Linked("x".into()),
            ))
        };
        let zone = || Row::NewBranch(id.clone());
        let at_rest = [card(), worktree()];
        let dragging = [card(), zone(), worktree()];
        assert!(
            room_came_or_went(&at_rest, &dragging),
            "the rows under the zone slide down as a drag starts"
        );
        assert!(
            room_came_or_went(&dragging, &at_rest),
            "and back up as it ends"
        );
        assert!(
            !room_came_or_went(&at_rest, &[worktree(), zone(), card()]),
            "the zone coming as the rows reorder is a reorder"
        );
    }

    #[test]
    fn cards_or_worktrees_trading_places_are_a_reorder() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let card = |id: &ProjectId| Row::Item(Selection::Project(id.clone()));
        let worktree = |name: &str| {
            Row::Item(Selection::Checkout(
                a.clone(),
                Checkout::Linked(name.into()),
            ))
        };
        let commit = || Row::Item(Selection::Uncommitted(a.clone(), Checkout::Main));
        let listed = [
            card(&a),
            worktree("x"),
            commit(),
            worktree("y"),
            Row::Gap,
            card(&b),
        ];
        let swapped = [
            card(&a),
            worktree("y"),
            worktree("x"),
            commit(),
            Row::Gap,
            card(&b),
        ];
        assert!(
            headings_reordered(&listed, &swapped),
            "worktrees trading places keep the list where it is, as cards do"
        );
        let cards = [
            card(&b),
            Row::Gap,
            card(&a),
            worktree("x"),
            commit(),
            worktree("y"),
        ];
        assert!(
            headings_reordered(&listed, &cards),
            "and cards trading places"
        );
        let gone = [card(&a), worktree("y"), Row::Gap, card(&b)];
        assert!(
            !headings_reordered(&listed, &gone),
            "a worktree going is no reorder"
        );
        assert!(
            !headings_reordered(&listed, &listed),
            "nor rows that stay in order"
        );
    }
}
