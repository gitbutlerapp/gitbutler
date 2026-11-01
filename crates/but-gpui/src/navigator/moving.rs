//! Moving commits or a branch with the keys, or squashing them or files into a commit: the
//! places they can land as the rows list them, stepping through those, the copies that show
//! where they'd land, and landing them.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use gpui::{AnyElement, App, Context, Div, Hsla, Window, div, prelude::*};

use super::rows::{COMMIT_INDENT, Fold, Row, RowKey, Rows, span_height};
use super::{CARD_MARGIN, Navigator, RowStyle};
use crate::app::{MoveHere, MoveSelection, SquashIntoSelection, SquashSelection, UseTargetMessage};
use crate::backend::{
    Checkout, Commit, CommitId, Destination, FullName, Messages, Workspace, short_ref,
};
use crate::state::{ProjectId, Section, Selection, Store, snack};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::transfer::{Move, Moving, Op, Stop, Target, Transfer, group_slots, slots};
use crate::ui::fold::{self, on_top};
use crate::ui::{Icon, ROW_HEIGHT, dp, fixed, fold_glyph, guide, h_flex, icon, small_icon};

/// The move of commits or a branch shown in `project`, or the squash of those or files, with
/// copies where it'd land: the one under way, or one Return landed, until the reload after it
/// shows where it went. A project's card and a terminal go where they'd land themselves (see
/// [`Navigator::watched`] and the rows' terminals), held while they move ([`Mark::Held`]).
fn shown<'a>(store: &'a Store, project: &ProjectId) -> Option<&'a Move> {
    (store.moving.as_ref().or_else(|| store.landing())).filter(|moving| {
        moving.project == *project
            && matches!(
                moving.what,
                Moving::Commits(_) | Moving::Branch(_) | Moving::Files { .. }
            )
    })
}

/// The move shown in `project`, when it lands what it moves somewhere at a stop `at` picks
/// out: the rows show copies of it there.
pub(super) fn landing_at<'a>(
    store: &'a Store,
    project: &ProjectId,
    at: impl Fn(&Stop) -> bool,
) -> Option<&'a Move> {
    shown(store, project).filter(|moving| at(&moving.at) && moving.shows_landing(store))
}

/// Whether what lands at `at` shows there as copies of itself, between rows, as commits and
/// a branch being moved do, rather than as an outline over the row it goes into, as a squash
/// does ([`squash_outline`]). A card and a terminal go there themselves.
fn copied_at(at: &Stop) -> bool {
    match at {
        Stop::In(_) | Stop::NewBranch { .. } | Stop::NewStack { .. } => true,
        Stop::Into(_) | Stop::Card(_) | Stop::Terminal { .. } => false,
    }
}

/// How the move shown marks a row, in the colour of its op.
#[derive(Clone, Copy)]
pub(super) enum Mark {
    /// What it's of, dimmed and dashed, as what's cut is: the commits, a branch with its
    /// commits, or the row files are listed under. It is, all of it, wherever the move is,
    /// where it started too, so nothing under what shows where it goes changes as that slides
    /// from row to row, and back over it: a copy of it, or the outline of what a squash goes
    /// into ([`squash_outline`]).
    Cut(Op),
    /// As [`Mark::Cut`], while the move is at where it is: its copy is drawn over it there,
    /// which slides out to where it'd land and back over it.
    Copied(Op),
    /// A terminal being moved, which goes where it'd land itself: held rather than selected.
    Held(Op),
}

/// How the move shown in `item`'s project marks its row, if it does, as [`Mark`] says.
pub(super) fn mark(store: &Store, item: &Selection) -> Option<Mark> {
    if let Some(moving) = (store.moving.as_ref())
        .filter(|moving| matches!(moving.what, Moving::Terminal { .. }) && moving.holds(item))
    {
        return Some(Mark::Held(moving.op));
    }
    let moving = shown(store, item.project())?;
    let holds = moving.holds(item);
    if !holds && !of_branch(moving, store, item) {
        return None;
    }
    let home = copied_at(&moving.at) && !moving.lands(store);
    Some(match holds && home {
        true => Mark::Copied(moving.op),
        false => Mark::Cut(moving.op),
    })
}

/// The colour the move shown in `project` marks rows in, as [`Op::colour`] says.
pub(super) fn mark_of(store: &Store, project: &ProjectId, theme: &Theme) -> Hsla {
    shown(store, project).map_or(theme.pop, |moving| moving.op.colour(theme))
}

/// Whether `item` is a commit of the branch `moving` moves or squashes.
fn of_branch(moving: &Move, store: &Store, item: &Selection) -> bool {
    match (&moving.what, item) {
        (Moving::Branch(branch), Selection::Commit(project, id)) => {
            *project == moving.project
                && (store.find_commit(project, *id))
                    .is_some_and(|(on, _)| on.ref_name.as_ref() == Some(branch))
        }
        _ => false,
    }
}

/// Whether `item`'s row is what's moving, left where it is while its copy shows where it'd
/// land, or over it while that's where it is. A commit being made is no copy of the row its
/// changes are under.
pub(super) fn left_behind(store: &Store, item: &Selection) -> bool {
    shown(store, item.project()).is_some_and(|moving| {
        matches!(moving.what, Moving::Commits(_) | Moving::Branch(_))
            && copied_at(&moving.at)
            && moving.holds(item)
    })
}

/// Where the outline of the squash shown is: around what it goes into, or back where it
/// started, around the rows held there, drawn with the row it started from.
#[derive(Clone)]
pub(super) struct SquashOutline {
    /// The row it's drawn with.
    pub(super) row: Selection,
    /// How far down the rows that row's top and bottom are.
    row_top: f32,
    row_bottom: f32,
    /// How far down the rows its top and bottom are.
    top: f32,
    bottom: f32,
}

impl SquashOutline {
    /// The outline of the squash shown in `rows`: around what it goes into, or with `home`,
    /// back where it started, around the selection's row and those squashed with it, as their
    /// dashed outline goes around them.
    pub(super) fn of(rows: &[Row], store: &Store, home: bool) -> Option<SquashOutline> {
        let squashed = |ix: &usize| matches!(rows.get(*ix), Some(Row::Item(item)) if matches!(mark(store, item), Some(Mark::Cut(_))));
        let (ix, first, last) = (rows.iter().enumerate()).find_map(|(ix, row)| match row {
            Row::Item(item) if squashed_into(store, item) => Some((ix, ix, ix)),
            Row::Item(item) if home && store.selection.as_ref() == Some(item) => {
                let first = (0..ix).rev().take_while(squashed).last().unwrap_or(ix);
                let last = (ix + 1..rows.len())
                    .take_while(squashed)
                    .last()
                    .unwrap_or(ix);
                Some((ix, first, last))
            }
            _ => None,
        })?;
        let Some(Row::Item(row)) = rows.get(ix) else {
            return None;
        };
        Some(SquashOutline {
            row: row.clone(),
            row_top: span_height(rows, 0..ix),
            row_bottom: span_height(rows, 0..ix + 1),
            top: span_height(rows, 0..first),
            bottom: span_height(rows, 0..last + 1),
        })
    }

    /// How far its top and bottom edges slide as it moves from `was` to here.
    pub(super) fn slide(&self, was: &SquashOutline) -> (f32, f32) {
        (was.top - self.top, was.bottom - self.bottom)
    }

    /// Where it's drawn from its row, by its top's and its bottom's distance from the row's,
    /// with its edges `slid` as far as they have yet to slide.
    pub(super) fn edges(&self, slid: (f32, f32)) -> (f32, f32) {
        let (top, bottom) = slid;
        (
            self.top - self.row_top + top,
            self.bottom - self.row_bottom + bottom,
        )
    }
}

/// Whether `item`'s row is what a squash goes into, elsewhere than where it started, which
/// [`squash_outline`] outlines.
pub(super) fn squashed_into(store: &Store, item: &Selection) -> bool {
    shown(store, item.project())
        .is_some_and(|moving| moving.squashes_into(item) && moving.shows_landing(store))
}

/// A row a move took away as it stepped on, landed or ended, still drawn where it was while the
/// rows close over it: `by` below the nearest row above it that stays, `under`, which it moves
/// with, set in by `inset` and with the guides it had.
pub(super) struct StandIn {
    of: Gone,
    pub(super) under: RowKey,
    by: f32,
    inset: f32,
    guides: Vec<f32>,
    /// When it was taken away, which it fades from as the rows close over it, at their pace:
    /// so no sliver of it stays as they come to rest, nor does it brighten if they set off again.
    since: Instant,
    /// How much of it shows in the frame being drawn.
    shown: f32,
    /// What took it away, which marks it in its colour.
    op: Op,
    /// The stand-ins above and below it, taken away with it, are outlined as it is.
    outlined_with: (bool, bool),
}

impl StandIn {
    /// Fade it to the frame drawn at `now`, saying whether any of it still shows.
    pub(super) fn fade(&mut self, now: Instant) -> bool {
        self.shown = fold::openness(false, self.since, now);
        self.shown > 0.
    }
}

/// What a move took away.
enum Gone {
    /// A commit being moved, dimmed where the move left it, as it goes back there or lands, or
    /// squashed away.
    Commit(ProjectId, Box<Commit>),
    /// A branch being moved, as [`branch_copy`] draws it, dimmed where the move left it.
    Branch { id: String, name: String },
    /// The new branch commits being moved would land in at that stop.
    NewBranch(Stop),
}

/// The rows keyed `old` that a move takes away as they go to `new`, the commits where it left
/// them, once it's `landing` them elsewhere, and the new branch they'd land in, each by its
/// index and that of the nearest row above it that stays. As a squash `ended`, every commit
/// that goes too, as it took them into another.
fn taken_away(
    old: &[Option<RowKey>],
    new: &[Option<RowKey>],
    ended: bool,
    landing: bool,
) -> Vec<(usize, usize)> {
    let kept: HashSet<&RowKey> = new.iter().flatten().collect();
    let stays =
        |ix: &usize| (old.get(*ix).and_then(Option::as_ref)).is_some_and(|key| kept.contains(key));
    (old.iter().enumerate())
        .filter(|(_, key)| match key {
            // Unless it lands elsewhere, what's moving takes its own row back where it's left.
            Some(key @ RowKey::Left(own)) => {
                !kept.contains(key) && (landing || !kept.contains(&**own))
            }
            Some(key @ RowKey::GhostBranch(_)) => !kept.contains(key),
            Some(key @ RowKey::Commit(..)) => ended && !kept.contains(key),
            Some(
                RowKey::Item(_)
                | RowKey::RestsOn(_)
                | RowKey::Fold(..)
                | RowKey::NewBranch(_)
                | RowKey::NewCommit(_)
                | RowKey::SquashTop
                | RowKey::SquashBottom
                | RowKey::After(_),
            )
            | None => false,
        })
        .filter_map(|(ix, _)| Some((ix, (0..ix).rev().find(stays)?)))
        .collect()
}

impl Rows {
    /// Copies of what `moving` moves: its commits, or the commit uncommitted changes being
    /// committed make, under the new branch they'd land in with `new_branch`, or its branch's
    /// row, which heads its commits wherever it goes.
    pub(super) fn push_ghosts(&mut self, moving: &Move, new_branch: bool) {
        let project = &moving.project;
        let copies: Vec<Row> = match &moving.what {
            Moving::Commits(ids) => (ids.iter())
                .map(|id| Row::Ghost(Selection::Commit(project.clone(), *id)))
                .collect(),
            Moving::Branch(branch) => {
                let item = Selection::Branch(project.clone(), Some(branch.clone()));
                return self.push(Row::Ghost(item));
            }
            Moving::Files { .. } if moving.op == Op::Commit => {
                Vec::from([Row::NewCommit(project.clone())])
            }
            // A card or a terminal goes where it'd land itself, and files squashed go into a
            // row.
            Moving::Project(_) | Moving::Terminal { .. } | Moving::Files { .. } => return,
        };
        if !new_branch {
            return self.extend(copies);
        }
        self.push(Row::GhostBranch(moving.project.clone(), moving.at.clone()));
        self.open();
        self.extend(copies);
        self.close();
    }
}

/// The places `commits` of `project` can land, in the order `rows` list them, and which of
/// them is where they are: the one just above the first of them. `open` says whether a heading
/// shows what it holds: a folded branch has only the new branches either side of it, so what's
/// folded is stepped over and stays folded, and the main checkout's stacks show only while
/// its heading's open, as a new stack goes before and after them. A linked worktree's lane
/// takes new branches as a stack does.
pub(super) fn stops<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    commits: &[CommitId],
    workspace: &Workspace,
    open: impl Fn(&Selection) -> bool,
) -> (Vec<Stop>, usize) {
    /// A stack's bottom branch, once its rows are walked, takes a new branch under it.
    fn end_stack(stops: &mut Vec<Stop>, bottom: &mut Option<&FullName>) {
        if let Some(beside) = bottom.take() {
            stops.push(Stop::NewBranch {
                beside: beside.clone(),
                below: true,
            });
        }
    }
    let mut stops = Vec::new();
    let mut start = None;
    let mut bottom = None;
    // The commits listed now are in a branch they can land in.
    let mut in_branch = false;
    // The main checkout's stacks are listed now, which a new stack goes after, and before
    // the first of them, until that's listed.
    let (mut in_main, mut before_first) = (false, false);
    for row in rows {
        if before_first && matches!(row, Row::Item(Selection::Branch(p, _)) if p == project) {
            before_first = false;
            stops.push(Stop::NewStack { first: true });
        }
        match row {
            Row::Item(heading @ Selection::Uncommitted(p)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                in_main = open(heading);
                before_first = in_main && workspace.managed;
            }
            // Not an unnamed branch, which has no ref to land on, nor the default branch,
            // which work stays off.
            Row::Item(item @ Selection::Branch(p, Some(name)))
                if p == project && workspace.target_branch.as_ref() != Some(name) =>
            {
                end_stack(&mut stops, &mut bottom);
                // A linked worktree's lane takes new branches, and so does a stack of the
                // workspace, but not the one branch checked out without it.
                let lane = (workspace.lane_of(Some(name)))
                    .filter(|(lane, _)| workspace.managed || lane.checkout != Checkout::Main);
                if let Some((lane, at)) = lane {
                    stops.push(Stop::NewBranch {
                        beside: name.clone(),
                        below: false,
                    });
                    if at + 1 == lane.branches.len() {
                        bottom = Some(name);
                    }
                }
                // Folded, its top is out of sight with its commits.
                if open(item) {
                    stops.push(Stop::In(Destination::Branch(name.clone())));
                }
                in_branch = true;
            }
            Row::Item(Selection::Branch(p, _)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
            }
            Row::Item(Selection::Commit(p, id)) if p == project => {
                if commits.contains(id) {
                    start = start.or(stops.len().checked_sub(1));
                } else if in_branch {
                    stops.push(Stop::In(Destination::BelowCommit(*id)));
                }
            }
            // What a branch lists beside its own commits: its remote's, the folds of its
            // commits, stand-ins for commits on their way, and the copies of those moving.
            Row::Item(
                Selection::Upstream(..) | Selection::Section(_, Section::Upstream(Some(_))),
            )
            | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::NewCommit(_)
            | Row::Between(_) => {}
            // A checkout's terminals, listed above its branches.
            Row::Item(Selection::Terminal(..)) => {}
            _ => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                before_first = false;
                if std::mem::take(&mut in_main) && workspace.managed {
                    stops.push(Stop::NewStack { first: false });
                }
            }
        }
    }
    end_stack(&mut stops, &mut bottom);
    if in_main && workspace.managed {
        stops.push(Stop::NewStack { first: false });
    }
    (stops, start.unwrap_or_default())
}

/// The rows of `project`, in the order `rows` list them, that what's being squashed can go
/// into, as `lands` says, and which of them is `home`, the row it starts from, listed whether
/// it lands there or not. A branch's commits take it, but a folded branch stands in for its
/// newest commit, out of sight, and the branch being squashed squashes into one at its own row.
/// A checkout's heading takes what's uncommitted into it. Not the default branch a linked
/// worktree can have checked out, which work stays off.
pub(super) fn squash_stops<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    workspace: &Workspace,
    home: &Target,
    open: impl Fn(&Selection) -> bool,
    lands: impl Fn(&Stop) -> bool,
) -> Option<(Vec<Stop>, usize)> {
    let mut stops = Vec::new();
    // The commits listed now are in a branch they can go into.
    let mut in_branch = false;
    for row in rows {
        let target = match row {
            Row::Item(Selection::Uncommitted(p)) if p == project => {
                in_branch = false;
                Some(Target::Uncommitted(Checkout::Main))
            }
            Row::Item(Selection::Worktree(p, name)) if p == project => {
                in_branch = false;
                Some(Target::Uncommitted(Checkout::Linked(name.clone())))
            }
            Row::Item(item @ Selection::Branch(p, Some(name)))
                if p == project && workspace.target_branch.as_ref() != Some(name) =>
            {
                in_branch = true;
                let target = Target::Branch(name.clone());
                (!open(item) || target == *home).then_some(target)
            }
            Row::Item(Selection::Branch(p, _)) if p == project => {
                in_branch = false;
                None
            }
            Row::Item(Selection::Commit(p, id)) if p == project => {
                let target = Target::Commit(*id);
                (in_branch || target == *home).then_some(target)
            }
            // What a branch lists beside its own commits, and a checkout's terminals.
            Row::Item(
                Selection::Upstream(..)
                | Selection::Section(_, Section::Upstream(Some(_)))
                | Selection::Terminal(..),
            )
            | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::NewCommit(_)
            | Row::Between(_) => None,
            _ => {
                in_branch = false;
                None
            }
        };
        let Some(target) = target else {
            continue;
        };
        let is_home = target == *home;
        let stop = Stop::Into(target);
        if is_home || lands(&stop) {
            stops.push(stop);
        }
    }
    let start = (stops.iter()).position(|stop| matches!(stop, Stop::Into(at) if at == home))?;
    Some((stops, start))
}

impl Navigator {
    /// Stand-ins for the rows as last drawn, with the move `was` shown, that it takes away at
    /// `now` as they go to those keyed `keys`, as the squash that was shown `ended` if it did,
    /// or the move Return landed shows `landing` it.
    pub(super) fn stand_ins(
        &self,
        keys: &[Option<RowKey>],
        was: Option<Op>,
        (ended, landing): (bool, bool),
        now: Instant,
        store: &Store,
    ) -> Vec<StandIn> {
        let Some(op) = was else {
            return Vec::new();
        };
        let taken = taken_away(&self.row_keys, keys, ended, landing);
        // Those taken away together, next to each other, stay outlined as one.
        let together: HashSet<usize> = (taken.iter())
            .map(|(ix, _)| *ix)
            .filter(|ix| matches!(self.rows.get(*ix), Some(Row::Item(_))))
            .collect();
        (taken.into_iter())
            .filter_map(|(ix, above)| {
                let of = match self.rows.get(ix)? {
                    Row::Item(Selection::Commit(project, id)) => {
                        // Landed, it's another commit where it went, found by its key.
                        let key = match self.row_keys.get(ix).cloned().flatten() {
                            Some(RowKey::Left(own)) => match *own {
                                RowKey::Commit(_, key) => Some(key),
                                _ => None,
                            },
                            _ => None,
                        };
                        let commit = (store.find_commit(project, *id).map(|(_, commit)| commit))
                            .or_else(|| {
                                let key = key?;
                                (store.workspace(project)?.commits())
                                    .find(|commit| commit.key == key)
                            })?;
                        Gone::Commit(project.clone(), Box::new(commit.clone()))
                    }
                    Row::Item(Selection::Branch(project, ref_name @ Some(_))) => {
                        let workspace = store.workspace(project)?;
                        let branch = workspace.find_branch(ref_name.as_ref())?;
                        Gone::Branch {
                            id: format!("stand-in-{project}-{}", branch.name),
                            name: store.name_of(project, branch).to_owned(),
                        }
                    }
                    Row::GhostBranch(_, at) => Gone::NewBranch(at.clone()),
                    _ => return None,
                };
                let outlined_with = (
                    ix.checked_sub(1).is_some_and(|ix| together.contains(&ix)),
                    together.contains(&(ix + 1)),
                );
                Some(StandIn {
                    of,
                    outlined_with,
                    under: self.row_keys.get(above)?.clone()?,
                    by: span_height(&self.rows, above..ix),
                    inset: self.insets.get(ix).copied().unwrap_or_default(),
                    guides: self.guides.get(ix).cloned().unwrap_or_default(),
                    since: now,
                    shown: 1.,
                    op,
                })
            })
            .collect()
    }

    /// The rows as last drawn, but those a closing fold takes away.
    fn shown_rows(&self) -> impl Iterator<Item = &Row> {
        let closing = |fold: &Option<super::rows::Moving>| {
            fold.as_ref()
                .is_some_and(|fold| self.folds.closing(&fold.fold))
        };
        (self.rows.iter().zip(&self.moving))
            .filter(move |(_, fold)| !closing(fold))
            .map(|(row, _)| row)
    }

    /// Where `what` of `project` can land as `op` as the rows are now, and which of those places
    /// is where it is: moved, as [`stops`] says, or squashed, as [`squash_stops`] says, from the
    /// selection's row, which holds it.
    fn stops_for(
        &self,
        project: &ProjectId,
        what: &Moving,
        op: Op,
        cx: &App,
    ) -> Option<(Vec<Stop>, usize)> {
        let store = self.store.read(cx);
        match what {
            Moving::Project(order) => {
                let (slots, start) = slots(order, project)?;
                return Some((slots.into_iter().map(Stop::Card).collect(), start));
            }
            Moving::Terminal { id, lists } => {
                let (slots, start) = group_slots(lists, id)?;
                let stops = slots
                    .into_iter()
                    .map(|(checkout, slot)| Stop::Terminal { checkout, slot });
                return Some((stops.collect(), start));
            }
            // Files are only squashed.
            Moving::Files { .. } if op == Op::Move => return None,
            Moving::Commits(_) | Moving::Branch(_) | Moving::Files { .. } => {}
        }
        let workspace = store.workspace(project)?;
        let open = |heading: &Selection| self.open_as_built(heading);
        let lands = |at: &Stop| {
            // Commits picked together squash at the one selected, where they start, the outline
            // around them all, not into the others one by one.
            let among = matches!((what, at),
                (Moving::Commits(ids), Stop::Into(Target::Commit(id))) if ids.contains(id));
            let (project, what, at) = (project.clone(), what.clone(), at.clone());
            let moving = Move {
                project,
                what,
                at,
                op,
            };
            !among && moving.lands(store)
        };
        match op {
            Op::Move => {
                let commits = match what {
                    Moving::Commits(ids) => &ids[..],
                    Moving::Branch(_)
                    | Moving::Project(_)
                    | Moving::Terminal { .. }
                    | Moving::Files { .. } => &[],
                };
                let all = stops(self.shown_rows(), project, commits, workspace, open);
                let Moving::Branch(branch) = what else {
                    return Some(all);
                };
                branch_stops(all.0, branch, lands)
            }
            Op::Squash(_) => {
                let home = (store.selection.as_ref()).and_then(Target::of)?;
                squash_stops(self.shown_rows(), project, workspace, &home, open, lands)
            }
            // Where commits moved can land, but the top of a folded branch too, which the new
            // commit shows under, as the most likely place for it. It starts where it's
            // started from ([`Self::start_committing`]), not where anything is.
            Op::Commit => {
                let open = |heading: &Selection| {
                    matches!(heading, Selection::Branch(..)) || self.open_as_built(heading)
                };
                let (all, _) = stops(self.shown_rows(), project, &[], workspace, open);
                let mut stops: Vec<Stop> = all.into_iter().filter(|stop| lands(stop)).collect();
                // The main checkout's changes can always go in a new branch of their own, after
                // the rest, as where to commit with no branches, or outside the workspace,
                // where it has no stacks to go beside.
                let new_stack = Stop::NewStack { first: false };
                if !stops.contains(&new_stack) && lands(&new_stack) {
                    stops.push(new_stack);
                }
                Some((stops, 0))
            }
        }
    }

    /// Whether `heading` showed what it holds as the rows were last built; the filter shows
    /// what it finds, folds aside.
    fn open_as_built(&self, heading: &Selection) -> bool {
        let fold = Fold::Under(heading.clone());
        self.folds_seen.get(&fold).copied().unwrap_or(true)
    }

    /// The stops of the move under way, and which of them it's at: where what it moves is, if
    /// the one it was at is gone, as when it's folded away.
    fn move_stops(&self, cx: &App) -> Option<(Vec<Stop>, usize)> {
        let moving = self.store.read(cx).moving.as_ref()?;
        let (stops, start) = self.stops_for(&moving.project, &moving.what, moving.op, cx)?;
        let at = (stops.iter().position(|stop| *stop == moving.at)).unwrap_or(start);
        Some((stops, at))
    }

    /// Start moving the selected commit, with those picked with it, the selected branch, or the
    /// selected watched project's card, from where it is. The keys that step through the rows
    /// step through where it can land until Return lands it or Escape lets go. While it's
    /// being squashed, move it instead.
    pub(super) fn start_move(
        &mut self,
        _: &MoveSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if let Some(moving) = store.moving.clone() {
            return self.switch(moving, Op::Move, cx);
        }
        let (project, what) = match store.selection.clone() {
            // Among the cards as they're listed now, whatever order lists them.
            Some(Selection::Project(project))
                if store.project(&project).is_some_and(|p| p.expanded) =>
            {
                let order: Arc<[ProjectId]> = (self.watched(store).into_iter())
                    .map(|e| e.id.clone())
                    .collect();
                (project, Moving::Project(order))
            }
            // Among the terminals of its project's worktrees as they're listed now.
            Some(Selection::Terminal(project, id)) => {
                let open = |heading: &Selection| self.open_as_built(heading);
                let lists = terminal_lists(self.shown_rows(), &project, open).into();
                (project, Moving::Terminal { id, lists })
            }
            _ => match commits_selected(store) {
                Some(selected) => selected,
                None => return,
            },
        };
        // Somewhere besides where it is.
        let op = Op::Move;
        let start = (self.stops_for(&project, &what, op, cx))
            .filter(|(stops, _)| stops.len() > 1)
            .and_then(|(stops, start)| stops.into_iter().nth(start));
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to move it", cx);
        };
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
    }

    /// Start squashing the selected commit, with those picked with it, the selected branch, or
    /// the selected worktree's uncommitted changes, those checked while they show, from its own
    /// row, as the TUI's squash mode does: the keys step through what it can go into until
    /// Return squashes it there or Escape lets go. While it's being moved, squash it instead.
    pub(super) fn start_squash(
        &mut self,
        _: &SquashSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if let Some(moving) = store.moving.clone() {
            return self.switch(moving, Op::Squash(Messages::default()), cx);
        }
        let (project, what) = match store.selection.clone() {
            Some(item @ (Selection::Uncommitted(_) | Selection::Worktree(..))) => {
                let project = item.project().clone();
                let what = (store.checkout_of(&item))
                    .and_then(|worktree| self.uncommitted(&project, worktree, cx));
                let Some(what) = what else {
                    return snack(&self.store, "Nothing uncommitted to squash", cx);
                };
                (project, what)
            }
            _ => match commits_selected(store) {
                Some(selected) => selected,
                None => return,
            },
        };
        self.squash(project, what, window, cx)
    }

    /// Start squashing the uncommitted changes of the selected commit's worktree into it, or
    /// into the selected branch's newest commit, as the TUI's ⇧R does: from their heading,
    /// which is selected, as it holds them. At a folded branch, which stands in for its newest
    /// commit, out of sight.
    pub(super) fn start_squash_into(
        &mut self,
        _: &SquashIntoSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let (project, into, at) = match store.selection.clone() {
            Some(Selection::Commit(project, id)) => (project, id, Target::Commit(id)),
            Some(item @ Selection::Branch(_, Some(_))) => {
                let newest = (store.branch_for(&item)).and_then(|branch| branch.commits.first());
                let (Some(newest), Selection::Branch(project, Some(name))) = (newest, &item) else {
                    return snack(&self.store, "Nothing to squash into", cx);
                };
                let at = match self.open_as_built(&item) {
                    true => Target::Commit(newest.id),
                    false => Target::Branch(name.clone()),
                };
                (project.clone(), newest.id, at)
            }
            _ => return,
        };
        let Some(worktree) = store.checkout_of(&Selection::Commit(project.clone(), into)) else {
            return;
        };
        let Some(what) = self.uncommitted(&project, worktree.clone(), cx) else {
            return snack(&self.store, "Nothing uncommitted to squash", cx);
        };
        let heading = Selection::uncommitted_in(project.clone(), worktree);
        let moving = Move {
            project,
            what,
            at: Stop::Into(at),
            op: Op::Squash(Messages::default()),
        };
        if !moving.lands(self.store.read(cx)) {
            return snack(&self.store, "Those changes can't go in there", cx);
        }
        self.store
            .update(cx, |store, cx| store.select(Some(heading), cx));
        self.reveal_selection = true;
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
    }

    /// Where a commit started from `from` in `project` is placed first: on top of the selected
    /// branch, or right above the selected commit; `None` from the changes' own row, which
    /// places it on top of the branch below it ([`Self::start_committing`]). The branch shown
    /// open stays open as the selection leaves it for the changes', so the commit shows where
    /// it's placed.
    pub(crate) fn commit_start(
        &mut self,
        project: &ProjectId,
        from: &Selection,
        cx: &mut Context<Self>,
    ) -> Option<Stop> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        let (branch, start) = match from {
            Selection::Branch(_, Some(name)) => {
                (name.clone(), Stop::In(Destination::Branch(name.clone())))
            }
            Selection::Commit(_, id) => {
                let (branch, _) = workspace.find_commit(*id)?;
                let name = branch.ref_name.clone()?;
                let at = branch.commits.iter().position(|commit| commit.id == *id)?;
                let above = at
                    .checked_sub(1)
                    .and_then(|above| branch.commits.get(above));
                let start = match above {
                    Some(above) => Stop::In(Destination::BelowCommit(above.id)),
                    None => Stop::In(Destination::Branch(name.clone())),
                };
                (name, start)
            }
            _ => return None,
        };
        let row = Selection::Branch(project.clone(), Some(branch.clone()));
        if self.open_as_built(&row) {
            self.set_open(
                super::ClickOpened::Here((project.clone(), Some(branch))),
                true,
                cx,
            );
        }
        Some(start)
    }

    /// Start placing a commit of `worktree`'s uncommitted changes in `project`, from `start`
    /// if it can go there, else on top of the topmost branch it can go on, as the TUI's commit
    /// mode does: the keys step through where it can go until Return commits it there, its
    /// message written first, or Escape lets go. The composer shows what goes in, and the
    /// message. So c and Return commit on top of the branch below the changes. The keys come
    /// here to place it.
    pub(crate) fn start_committing(
        &mut self,
        project: ProjectId,
        worktree: Checkout,
        start: Option<Stop>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let files = (self.detail.read(cx)).files_to_move(&project, worktree.clone(), cx);
        let what = Moving::Files {
            from: None,
            worktree,
            files: files.into(),
        };
        let stops = (self.stops_for(&project, &what, Op::Commit, cx))
            .map(|(stops, _)| stops)
            .unwrap_or_default();
        let topmost = (stops.iter())
            .position(|stop| matches!(stop, Stop::In(Destination::Branch(_))))
            .unwrap_or_default();
        let at = (start.filter(|start| stops.contains(start)))
            .or_else(|| stops.into_iter().nth(topmost));
        let Some(at) = at else {
            return snack(&self.store, "Nowhere to commit to", cx);
        };
        let moving = Move {
            project,
            what,
            at,
            op: Op::Commit,
        };
        self.reveal_selection = true;
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
    }

    /// `worktree`'s uncommitted changes, as what's squashed: those checked while they show, but
    /// not conflicted ones. `None` when there are none.
    fn uncommitted(&self, project: &ProjectId, worktree: Checkout, cx: &App) -> Option<Moving> {
        let files = self
            .detail
            .read(cx)
            .files_to_move(project, worktree.clone(), cx);
        (!files.is_empty()).then(|| Moving::Files {
            from: None,
            worktree,
            files: files.into(),
        })
    }

    /// Start squashing `what` of `project` from the selection's row, which holds it.
    pub(crate) fn squash(
        &mut self,
        project: ProjectId,
        what: Moving,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let op = Op::Squash(Messages::default());
        let store = self.store.read(cx);
        // Somewhere besides where it is, unless it goes somewhere there, as a branch squashing
        // into one commit does.
        let start = (self.stops_for(&project, &what, op, cx)).and_then(|(stops, start)| {
            // Uncommitted changes go first into what's on top of the branch below their row, as
            // a commit of them goes there first: s and Return amend it.
            let start = match &what {
                Moving::Files { from: None, .. } => (stops.iter().enumerate().skip(start + 1))
                    .find(|(_, stop)| {
                        matches!(stop, Stop::Into(Target::Commit(_) | Target::Branch(_)))
                    })
                    .map_or(start, |(at, _)| at),
                Moving::Commits(_)
                | Moving::Branch(_)
                | Moving::Project(_)
                | Moving::Terminal { .. }
                | Moving::Files { .. } => start,
            };
            let at = stops.get(start)?.clone();
            let home = Move {
                project: project.clone(),
                what: what.clone(),
                at: at.clone(),
                op,
            };
            (stops.len() > 1 || home.lands(store)).then_some(at)
        });
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to squash it", cx);
        };
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
    }

    /// Squash what `moving` moves, or move what it squashes, as `op` says: from what matches
    /// where it's at, below a commit or into it, which show where it goes in the same place,
    /// else from where it is.
    fn switch(&mut self, moving: Move, op: Op, cx: &mut Context<Self>) {
        let same_op = matches!(
            (moving.op, op),
            (Op::Move, Op::Move) | (Op::Squash(_), Op::Squash(_))
        );
        if same_op || !moving.of_commits() {
            return;
        }
        let Move {
            project, what, at, ..
        } = moving;
        let Some((stops, start)) = self.stops_for(&project, &what, op, cx) else {
            return;
        };
        let same = match (&at, op) {
            (Stop::In(Destination::BelowCommit(id)), Op::Squash(_)) => {
                Some(Stop::Into(Target::Commit(*id)))
            }
            (Stop::Into(Target::Commit(id)), Op::Move) => {
                Some(Stop::In(Destination::BelowCommit(*id)))
            }
            _ => None,
        };
        let Some(at) =
            (same.filter(|stop| stops.contains(stop))).or_else(|| stops.into_iter().nth(start))
        else {
            return;
        };
        self.reveal_selection = true;
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
    }

    /// Keep only the message of what a squash goes into, or both again, as the TUI's u does.
    pub(super) fn use_target_message(
        &mut self,
        _: &UseTargetMessage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.store.update(cx, |store, cx| {
            if let Some(Move {
                op: Op::Squash(keep),
                ..
            }) = &mut store.moving
            {
                *keep = match keep {
                    Messages::Both => Messages::Target,
                    Messages::Target => Messages::Both,
                };
                cx.notify();
            }
        });
    }

    /// Step the move `delta` places on, stopping at either end.
    pub(super) fn step_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((stops, at)) = self.move_stops(cx) else {
            return;
        };
        let last = stops.len().saturating_sub(1) as isize;
        let next = (at as isize).saturating_add(delta).clamp(0, last) as usize;
        self.move_to(stops, next, cx);
    }

    /// Jump the move to the top of the next branch, or with `forward` false the one before, or
    /// to a new stack, as ⇧J and ⇧K jump between sections. A branch, which lands only between
    /// branches, jumps between the tops of stacks, and a card from card to card, as it steps.
    pub(super) fn jump_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some((stops, at)) = self.move_stops(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let workspace = (store.moving.as_ref()).and_then(|moving| store.workspace(&moving.project));
        let tops = |beside: &FullName| {
            workspace
                .and_then(|ws| ws.lane_of(Some(beside)))
                .is_some_and(|(_, at)| at == 0)
        };
        // A branch's newest commit, which a squash goes into for it while it's open.
        let newest = |id: &CommitId| {
            (workspace.and_then(|ws| ws.find_commit(*id)))
                .is_some_and(|(branch, _)| branch.commits.first().map(|c| c.id) == Some(*id))
        };
        let heads = |(_, stop): &(usize, &Stop)| match stop {
            Stop::In(Destination::Branch(_))
            | Stop::NewStack { .. }
            | Stop::Card(_)
            | Stop::Terminal { .. }
            | Stop::Into(Target::Branch(_) | Target::Uncommitted(_)) => true,
            Stop::Into(Target::Commit(id)) => newest(id),
            Stop::NewBranch {
                beside,
                below: false,
            } => tops(beside),
            Stop::In(_) | Stop::NewBranch { .. } => false,
        };
        let next = match forward {
            true => stops.iter().enumerate().skip(at + 1).find(heads),
            false => stops.iter().enumerate().take(at).rev().find(heads),
        };
        if let Some((next, _)) = next {
            self.move_to(stops, next, cx);
        }
    }

    fn move_to(&mut self, stops: Vec<Stop>, ix: usize, cx: &mut Context<Self>) {
        let Some(at) = stops.into_iter().nth(ix) else {
            return;
        };
        self.reveal_selection = true;
        self.store.update(cx, |store, cx| {
            if let Some(moving) = &mut store.moving
                && moving.at != at
            {
                moving.at = at;
                cx.notify();
            }
        });
    }

    /// Land what's being moved where the move is at, as dropping it there would, or squash it
    /// there.
    pub(super) fn move_here(&mut self, _: &MoveHere, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(moving) = store.moving.clone() else {
            return;
        };
        // A commit's made as ⌘↩ makes it: from the composer, its message written first, or
        // as it is without one, where the navigator came out for it.
        if moving.op == Op::Commit {
            return window.dispatch_action(Box::new(crate::app::StartCommit), cx);
        }
        let lands = moving.lands(store);
        let squashed = moving.squashed(store);
        self.store
            .update(cx, |store, cx| store.set_moving(None, cx));
        // Where they already are, there's nothing to do.
        if !lands {
            return;
        }
        // On what it goes into, as the TUI ends there: the commit, a branch squashed into one
        // too, or the uncommitted changes. It's selected before what goes is hidden, which
        // would leave a selected commit be, so what goes fades as the rows close over it.
        if let (Some(transfer), Stop::Into(target)) = (squashed, &moving.at) {
            let project = moving.project.clone();
            let on = match &transfer {
                Transfer::Squash { into, .. }
                | Transfer::Amend { into, .. }
                | Transfer::MoveFiles { to: into, .. } => Selection::Commit(project.clone(), *into),
                Transfer::Uncommit { .. }
                | Transfer::UncommitFiles { .. }
                | Transfer::MoveCommit { .. }
                | Transfer::CherryPick { .. }
                | Transfer::StackBranch { .. }
                | Transfer::BranchBelow { .. }
                | Transfer::StageOnBranch { .. }
                | Transfer::CommitOnBranch { .. }
                | Transfer::MoveWorktree { .. } => target.row(&project),
            };
            return self.store.update(cx, |store, cx| {
                store.select(Some(on), cx);
                // Only staging comes back, which a squash never is.
                let _ = transfer.perform(&project, None, store, cx);
            });
        }
        // A card or a terminal, already shown where it goes, goes there in the order they keep.
        if let Some(order) = moving.project_order() {
            return self
                .store
                .update(cx, |store, cx| store.order_watched(&order, cx));
        }
        if let Some(order) = moving.terminal_order() {
            let project = moving.project.clone();
            return (self.store)
                .update(cx, |store, cx| store.order_terminals(&project, &order, cx));
        }
        // A branch's copy stays where it lands till it's there.
        if let Some((payload, target, place)) = moving.drop() {
            return self.app.update_later(window, cx, move |app, window, cx| {
                app.land(&payload, &target, place, Some(moving), window, cx)
            });
        }
        let lands = Some(moving.clone());
        let Move {
            project, what, at, ..
        } = moving;
        self.store.update(cx, |store, cx| match (what, at) {
            (Moving::Commits(commits), Stop::NewBranch { beside, below }) => {
                let beside = Some((beside, below));
                store.move_to_new_branch(&project, commits, beside, false, lands, cx)
            }
            (Moving::Commits(commits), Stop::NewStack { first }) => {
                store.move_to_new_branch(&project, commits, None, first, lands, cx)
            }
            (Moving::Branch(branch), Stop::NewStack { first }) => {
                store.unstack(&project, branch, first, lands, cx)
            }
            // Those land as drops do, cards in their order, and squashes, above.
            (
                Moving::Commits(_),
                Stop::In(_) | Stop::Card(_) | Stop::Terminal { .. } | Stop::Into(_),
            )
            | (
                Moving::Branch(_)
                | Moving::Project(_)
                | Moving::Terminal { .. }
                | Moving::Files { .. },
                _,
            ) => {}
        });
    }
}

/// The selected commit, with those picked with it, or the selected branch, as what moves or
/// squashes, in its project.
fn commits_selected(store: &Store) -> Option<(ProjectId, Moving)> {
    match store.selection.clone()? {
        Selection::Commit(project, id) => {
            let commits = store.with_picks(&project, id);
            Some((project, Moving::Commits(commits)))
        }
        Selection::Branch(project, Some(branch)) => Some((project, Moving::Branch(branch))),
        Selection::Branch(_, None)
        | Selection::Project(_)
        | Selection::Uncommitted(_)
        | Selection::Worktree(..)
        | Selection::Stack(..)
        | Selection::Upstream(..)
        | Selection::Section(..)
        | Selection::Branches(_)
        | Selection::History(_)
        | Selection::Graph(_)
        | Selection::Terminal(..)
        | Selection::Mesh(..) => None,
    }
}

/// The terminals `rows` list for `project`, under each of its worktrees' headings, the main one's
/// and its linked ones', in the order they're listed: those whose heading `open` says shows
/// what it holds, as a terminal landing under a folded one would be out of sight.
fn terminal_lists<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    open: impl Fn(&Selection) -> bool,
) -> Vec<(Checkout, Vec<TerminalId>)> {
    let mut lists: Vec<(Checkout, Vec<TerminalId>)> = Vec::new();
    // Under a heading of the project's that's open now.
    let mut under = false;
    for row in rows {
        let checkout = match row {
            Row::Item(heading @ Selection::Uncommitted(p)) if p == project => {
                Some((heading, Checkout::Main))
            }
            Row::Item(heading @ Selection::Worktree(p, name)) if p == project => {
                Some((heading, Checkout::Linked(name.clone())))
            }
            Row::Item(Selection::Terminal(p, id)) if p == project && under => {
                if let Some((_, list)) = lists.last_mut() {
                    list.push(*id);
                }
                None
            }
            _ => None,
        };
        if let Some((heading, checkout)) = checkout {
            under = open(heading);
            if under {
                lists.push((checkout, Vec::new()));
            }
        }
    }
    lists
}

/// The stops of `all` a branch lands at, between branches, as a new branch would, and where it
/// is: the place above it, and the place below it, just as where it is, no stop. `lands` says
/// whether landing at a stop moves it, as it doesn't where it already is.
fn branch_stops(
    all: Vec<Stop>,
    branch: &FullName,
    lands: impl Fn(&Stop) -> bool,
) -> Option<(Vec<Stop>, usize)> {
    let home = Stop::NewBranch {
        beside: branch.clone(),
        below: false,
    };
    let stops: Vec<Stop> = (all.into_iter())
        .filter(|stop| match stop {
            _ if *stop == home => true,
            Stop::NewBranch { .. } | Stop::NewStack { .. } => lands(stop),
            Stop::In(_) | Stop::Card(_) | Stop::Terminal { .. } | Stop::Into(_) => false,
        })
        .collect();
    let start = stops.iter().position(|stop| *stop == home)?;
    Some((stops, start))
}

/// A copy of `item` being moved, a commit or a branch, where landing would put it, drawn as
/// its own row is while the move is at where it is, set in by `inset` as an item row is, and
/// outlined with the copies `outlined_with` says are above and below it. A branch's copy is its
/// row alone, folded, which heads its commits wherever it goes.
pub(super) fn ghost_row(
    item: &Selection,
    inset: f32,
    outlined_with: (bool, bool),
    store: &Store,
    theme: &Theme,
) -> AnyElement {
    let style = RowStyle {
        held: true,
        outlined_with,
        ..RowStyle::plain(theme, inset)
    };
    let row = match item {
        Selection::Commit(project, id) => (store.find_commit(project, *id)).map(|(_, commit)| {
            let id = format!("ghost-{project}-{}", id.0);
            style.commit(id, project, commit, store).into_any_element()
        }),
        Selection::Branch(project, ref_name) => (store.workspace(project))
            .and_then(|ws| ws.find_branch(ref_name.as_ref()))
            .map(|branch| {
                let id = format!("ghost-{project}-{}", branch.name);
                let name = store.name_of(project, branch).to_owned();
                branch_copy(id, name, style)
            }),
        _ => None,
    };
    row.unwrap_or_else(|| div().h(dp(ROW_HEIGHT)).into_any_element())
}

/// The commit uncommitted changes being committed in `project` would make, where it'd go,
/// set in by `inset` as a commit row is: titled as its message is written, outlined in the
/// colour of committing, as copies of what's moving are in theirs.
pub(super) fn new_commit_row(
    project: &ProjectId,
    inset: f32,
    store: &Store,
    theme: &Theme,
) -> AnyElement {
    let style = RowStyle {
        held: true,
        mark: Op::Commit.colour(theme),
        ..RowStyle::plain(theme, inset)
    };
    let title = shown(store, project).and_then(|moving| moving.commit_title(store));
    // Untitled till its message is written, quiet as an empty title is.
    let title = match title {
        Some(title) => div().child(title.to_owned()),
        None => div().text_color(style.muted()).child("New commit"),
    };
    (style.row(format!("new-commit-{project}"), COMMIT_INDENT))
        .gap(dp(10.))
        .child(small_icon(Icon::Commit).text_color(style.meta(theme.blue)))
        .child(title.flex_1().min_w_0().truncate())
        .into_any_element()
}

/// What a squash goes into outlined over its row, as dropping on it outlines it but in a
/// squash's colour, its top and bottom `edges` from the row's, as [`SquashOutline::edges`]
/// says, over the rows it passes.
pub(super) fn squash_outline(edges: (f32, f32), theme: &Theme) -> AnyElement {
    let (top, bottom) = edges;
    let outline = div()
        .absolute()
        .top(dp(top))
        .bottom(dp(-bottom))
        .left(dp(3.))
        .right(dp(3.))
        .rounded(dp(4.))
        .border_1()
        .border_color(theme.blue)
        .bg(theme.blue.opacity(0.14));
    on_top(outline).priority(2).into_any_element()
}

/// Branch `name`'s row as a copy draws it, row `id`, in `style`: folded, as its own row is,
/// and nothing on it acts.
fn branch_copy(id: String, name: String, style: RowStyle) -> AnyElement {
    (style.row(id.clone(), 6.))
        .child(fold_glyph(format!("{id}-fold"), Some(false), style.quiet()))
        .child(icon(Icon::Branch).text_color(style.muted()))
        .child(div().flex_1().min_w_0().truncate().child(name))
        .into_any_element()
}

/// What a move took away, drawn where it was below the row drawing it, as it was: inside the
/// card's edges, which the row's slice draws a hairline in, on the card's ground with its
/// guides, and over what the card's slices stretch over, so they don't cover it, while the rows
/// after it, closing over it, go over it.
pub(super) fn stand_in(stand_in: &StandIn, store: &Store, theme: &Theme) -> Div {
    let StandIn {
        of,
        by,
        inset,
        guides,
        shown,
        op,
        outlined_with,
        ..
    } = stand_in;
    let style = RowStyle {
        cut: true,
        mark: op.colour(theme),
        outlined_with: *outlined_with,
        ..RowStyle::plain(theme, *inset)
    };
    let row = match of {
        Gone::Commit(project, commit) => {
            let id = format!("stand-in-{project}-{}", commit.id.0);
            style.commit(id, project, commit, store).into_any_element()
        }
        Gone::Branch { id, name } => branch_copy(id.clone(), name.clone(), style),
        Gone::NewBranch(at) => ghost_branch_row(at, *inset, op.colour(theme), theme),
    };
    let guides = guides.iter().map(|at| guide(*at, theme));
    div()
        .absolute()
        .top(dp(*by))
        .left(dp(CARD_MARGIN))
        .right(dp(CARD_MARGIN))
        .px(fixed(1.))
        .child(on_top(
            (div().relative().bg(theme.bg_1).opacity(*shown))
                .child(row)
                .children(guides),
        ))
}

/// The new branch commits being moved or made would land in at stop `at`, over their copies,
/// set in by `inset` as an item row is, marked in `mark`, the colour of what's doing it. It
/// names the branch it goes above or below in their stack, or says it's independent, in a stack
/// of its own.
pub(super) fn ghost_branch_row(at: &Stop, inset: f32, mark: Hsla, theme: &Theme) -> AnyElement {
    let beside = match at {
        Stop::NewBranch { beside, below } => {
            let side = if *below { "below" } else { "above" };
            Some((side, short_ref(beside).into_owned()))
        }
        Stop::In(_)
        | Stop::NewStack { .. }
        | Stop::Card(_)
        | Stop::Terminal { .. }
        | Stop::Into(_) => None,
    };
    let said = match &beside {
        Some((side, _)) => format!("New branch {side}"),
        None => "New independent branch".into(),
    };
    h_flex()
        .min_h(dp(ROW_HEIGHT))
        .mx(dp(3.))
        .pl(dp(8. + inset))
        .pr(dp(12.))
        .gap(dp(8.))
        .rounded(dp(4.))
        .border_1()
        .border_dashed()
        .border_color(mark)
        .text_color(mark)
        .child(small_icon(Icon::Branch).text_color(mark))
        .child(
            // One phrase, a space apart, the branch's name as it's written in its row.
            h_flex()
                .min_w_0()
                .gap(dp(4.))
                .child(div().flex_none().child(said))
                .children(beside.map(|(_, name)| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.text_1)
                        .child(name)
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Row, RowKey, branch_stops, squash_stops, stops, taken_away, terminal_lists};
    use crate::backend::model::build::{branch, full, id, workspace, worktree};
    use crate::backend::{Checkout, CommitId, Destination, Workspace, short_ref};
    use crate::state::{ProjectId, Selection};
    use crate::terminal::TerminalId;
    use crate::transfer::{Slot, Stop, Target, group_slots};

    /// Whether `heading` shows what it holds, as [`listed`] lists them: all but typo.
    fn open(heading: &Selection) -> bool {
        !matches!(heading, Selection::Branch(_, Some(name)) if short_ref(name) == "typo")
    }

    /// Each stop on a line, commits by their number, `<` at `start`.
    fn describe(stops: &[Stop], start: usize) -> String {
        let commit = |c: &CommitId| (1..=9).find(|n| id(*n) == *c).unwrap_or_default();
        let short = |name| short_ref(name).into_owned();
        (stops.iter().enumerate())
            .map(|(ix, stop)| {
                let said = match stop {
                    Stop::NewBranch { beside, below } => {
                        let side = if *below { "below" } else { "above" };
                        format!("new branch {side} {}", short(beside))
                    }
                    Stop::In(Destination::Branch(name)) => format!("top of {}", short(name)),
                    Stop::In(Destination::BelowCommit(c)) => format!("below {}", commit(c)),
                    Stop::In(Destination::AboveCommit(c)) => format!("above {}", commit(c)),
                    Stop::NewStack { first: false } => "new stack".into(),
                    Stop::NewStack { first: true } => "new stack first".into(),
                    Stop::Card(Slot::Before(project)) => format!("before {project}"),
                    Stop::Terminal { checkout, slot } => {
                        let on = match checkout {
                            Checkout::Main => "main".to_owned(),
                            Checkout::Linked(name) => name.to_string(),
                        };
                        match slot {
                            Slot::Before(id) => format!("before terminal {} on {on}", id.0),
                            Slot::End => format!("last on {on}"),
                        }
                    }
                    Stop::Card(Slot::End) => "last".into(),
                    Stop::Into(Target::Commit(c)) => format!("into {}", commit(c)),
                    Stop::Into(Target::Branch(name)) => format!("into {}", short(name)),
                    Stop::Into(Target::Uncommitted(Checkout::Main)) => {
                        "into uncommitted on main".into()
                    }
                    Stop::Into(Target::Uncommitted(Checkout::Linked(name))) => {
                        format!("into uncommitted on {name}")
                    }
                };
                let here = if ix == start { " <" } else { "" };
                format!("{said}{here}\n")
            })
            .collect()
    }

    /// Two stacks, `ui` on `api` and `typo` alone, and a linked worktree `wt` on `agent`, and
    /// the rows the navigator lists for them, with a terminal open and `typo` folded.
    fn listed(project: &ProjectId) -> (Workspace, Vec<Row>) {
        let ws = Workspace {
            managed: true,
            ..workspace(
                vec![
                    vec![branch("ui", &[3]), branch("api", &[2, 1])],
                    vec![branch("typo", &[4])],
                ],
                vec![worktree("wt", None, vec![branch("agent", &[5])])],
            )
        };
        let p = || project.clone();
        let named = |name: &str| Some(full(&format!("refs/heads/{name}")));
        let rows = vec![
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Uncommitted(p())),
            // Listed above the stacks, which it doesn't part from the heading either.
            Row::Item(Selection::Terminal(p(), TerminalId(1))),
            Row::Item(Selection::Branch(p(), named("ui"))),
            Row::Item(Selection::Commit(p(), id(3))),
            Row::Item(Selection::Branch(p(), named("api"))),
            Row::Item(Selection::Commit(p(), id(2))),
            Row::Item(Selection::Commit(p(), id(1))),
            // The line between stacks, which parts nothing it walks.
            Row::Between(p()),
            Row::Item(Selection::Branch(p(), named("typo"))),
            Row::WorktreeDivider,
            Row::Item(Selection::Worktree(p(), "wt".into())),
            Row::Item(Selection::Branch(p(), named("agent"))),
            Row::Item(Selection::Commit(p(), id(5))),
            Row::WorktreeDivider,
            Row::Item(Selection::Branches(p())),
        ];
        (ws, rows)
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_commit_stops_between_every_branch_and_commit_but_in_what_is_folded() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (stops, start) = stops(&rows, &project, &[id(2)], &ws, open);
        // A new stack before the main checkout's stacks, a new branch above every branch of a
        // stack and below its last, each branch's top and the place below each of its commits
        // but the one moving, which starts below 3 at the top of api; then a new stack once the
        // stacks end. Folded, typo has only the new branches either side of it. The linked
        // worktree's lane takes new branches as a stack does.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack first
new branch above ui
top of ui
below 3
new branch above api
top of api <
below 1
new branch below api
new branch above typo
new branch below typo
new stack
new branch above agent
top of agent
below 5
new branch below agent

"#]]
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_folded_main_checkout_and_a_workspace_of_one_branch_take_no_new_branches() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (folded, _) = stops(rows.iter().take(2), &project, &[id(2)], &ws, |_| false);
        assert!(
            folded.is_empty(),
            "nothing shows under the folded heading, and no new stack goes after it"
        );
        let unmanaged = Workspace {
            managed: false,
            ..ws
        };
        let (stops, start) = stops(&rows, &project, &[id(1)], &unmanaged, open);
        // Outside the workspace there's no stack to put a branch in, so folded typo has no
        // stops, but a linked worktree's lane still takes one.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
top of ui
below 3
top of api
below 2 <
new branch above agent
top of agent
below 5
new branch below agent

"#]]
        );
    }

    #[test]
    fn what_a_move_takes_away_stays_under_the_nearest_row_that_stays() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let branch = |name: &str| {
            let name = full(&format!("refs/heads/{name}"));
            Some(RowKey::Item(Selection::Branch(p(), Some(name))))
        };
        let commit = |n: usize| Some(RowKey::Commit(p(), n.to_string()));
        let left = |n: usize| {
            let own = RowKey::Commit(p(), n.to_string());
            Some(RowKey::Left(Box::new(own)))
        };
        let new_branch = || Some(RowKey::GhostBranch(p()));
        // Commit 3 moving from the top of ui to a new branch above api.
        let above_api = [
            branch("ui"),
            left(3),
            commit(2),
            new_branch(),
            commit(3),
            branch("api"),
        ];
        let top_of_api = [branch("ui"), left(3), commit(2), branch("api"), commit(3)];
        assert_eq!(
            taken_away(&above_api, &top_of_api, false, false),
            [(3, 2)],
            "stepping on, the new branch goes, under commit 2"
        );
        let landed = [branch("ui"), commit(2), branch("api"), commit(3)];
        assert_eq!(
            taken_away(&top_of_api, &landed, false, true),
            [(1, 0)],
            "landing, where it was left goes, under ui"
        );
        let back = [branch("ui"), commit(3), commit(2), branch("api")];
        assert_eq!(
            taken_away(&above_api, &back, false, false),
            [(3, 2)],
            "letting go, the new branch goes, and the commit takes its own row back"
        );
        let both = [
            branch("ui"),
            left(3),
            left(2),
            commit(1),
            commit(3),
            commit(2),
        ];
        let below_one = [branch("ui"), commit(1), commit(3), commit(2)];
        assert_eq!(
            taken_away(&both, &below_one, false, true),
            [(1, 0), (2, 0)],
            "commits landing together go under the row above the first"
        );
    }

    #[test]
    fn what_a_squash_takes_away_stays_under_the_row_above_it_as_it_ends() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let ui = || {
            let name = full("refs/heads/ui");
            Some(RowKey::Item(Selection::Branch(p(), Some(name))))
        };
        let commit = |n: usize| Some(RowKey::Commit(p(), n.to_string()));
        // Commit 3 squashed into 1, or 3 and 2 squashed where they're held into 1 below them.
        let before = [ui(), commit(3), commit(2), commit(1)];
        assert_eq!(
            taken_away(&before, &[ui(), commit(2), commit(1)], true, false),
            [(1, 0)],
            "it goes under ui, as the rows close over it"
        );
        let into_one = [ui(), commit(1)];
        assert_eq!(
            taken_away(&before, &into_one, true, false),
            [(1, 0), (2, 0)],
            "commits going together go under the row above the first"
        );
        assert_eq!(
            taken_away(&before, &into_one, false, false),
            [],
            "while no squash ends, commits that go are another's to show going"
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_squash_stops_at_every_commit_folded_branch_and_heading_it_can_go_into() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let home = Target::Commit(id(2));
        let lands = |stop: &Stop| *stop != Stop::Into(Target::Commit(id(5)));
        let (stops, start) = squash_stops(&rows, &project, &ws, &home, open, lands).unwrap();
        // Each heading, uncommitting; every commit, but 5 where it doesn't land, and 2, where
        // it starts; and folded typo for its newest commit, out of sight. Open branches have
        // their newest commits listed.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
into uncommitted on main
into 3
into 2 <
into 1
into typo
into uncommitted on wt

"#]]
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_branch_squashes_into_one_commit_at_its_own_row_but_never_into_the_target() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let ws = Workspace {
            target_branch: Some(full("refs/heads/agent")),
            ..ws
        };
        let home = Target::Branch(full("refs/heads/api"));
        let lands = |stop: &Stop| matches!(stop, Stop::Into(Target::Commit(_)));
        let (stops, start) = squash_stops(&rows, &project, &ws, &home, open, lands).unwrap();
        // Open api is listed as it's what's squashed; the headings don't land, and agent is
        // the target, which work stays off, its commits too.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
into 3
into api <
into 2
into 1

"#]]
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_branch_stops_only_between_branches_and_where_it_is() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (all, _) = stops(&rows, &project, &[], &ws, open);
        let api = full("refs/heads/api");
        // Below itself it doesn't move, and a branch keeps to its own side, out of wt.
        let lands = |stop: &Stop| match stop {
            Stop::NewBranch { beside, below } => match short_ref(beside).as_ref() {
                "api" => !below,
                "agent" => false,
                _ => true,
            },
            Stop::In(_)
            | Stop::NewStack { .. }
            | Stop::Card(_)
            | Stop::Terminal { .. }
            | Stop::Into(_) => true,
        };
        let (stops, start) = branch_stops(all, &api, lands).unwrap();
        // The places between branches it lands at, where a new branch would go, and where it
        // is, the place right above it.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack first
new branch above ui
new branch above api <
new branch above typo
new branch below typo
new stack

"#]]
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_terminal_stops_among_the_terminals_of_every_open_worktree() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let terminal = |n| Row::Item(Selection::Terminal(p(), TerminalId(n)));
        let rows = [
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Uncommitted(p())),
            terminal(1),
            terminal(2),
            Row::Item(Selection::Branch(p(), Some(full("refs/heads/api")))),
            Row::Item(Selection::Worktree(p(), "wt".into())),
            Row::Item(Selection::Worktree(p(), "folded".into())),
            terminal(3),
            Row::Item(Selection::Worktree(p(), "empty".into())),
        ];
        // `folded` lists terminal 3 as it closes.
        let open = |heading: &Selection| !matches!(heading, Selection::Worktree(_, name) if name == "folded");
        let lists = terminal_lists(&rows, &project, open);
        let (stops, start) = group_slots(&lists, &TerminalId(1)).unwrap();
        let stops: Vec<Stop> = (stops.into_iter())
            .map(|(checkout, slot)| Stop::Terminal { checkout, slot })
            .collect();
        // Before 2 or after it on main, where 1 is before 2, and first on each open linked
        // worktree, with terminals or none, but the folded one.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
before terminal 2 on main <
last on main
last on wt
last on empty

"#]]
        );
        assert_eq!(
            lists.first(),
            Some(&(Checkout::Main, vec![TerminalId(1), TerminalId(2)])),
            "the main worktree's terminals, as listed"
        );
    }
}
