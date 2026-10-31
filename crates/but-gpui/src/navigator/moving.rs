//! Moving commits or a branch with the keys: the places they can land as the rows list them,
//! stepping through those, the copies that show where they'd land, and landing them.

use std::collections::HashSet;
use std::time::Instant;

use gpui::{AnyElement, App, Context, Div, Window, div, prelude::*};

use super::rows::{Fold, Row, RowKey, Rows, span_height};
use super::{CARD_MARGIN, Navigator, RowStyle};
use crate::app::{MoveHere, MoveSelection};
use crate::backend::{Checkout, Commit, CommitId, Destination, FullName, Workspace, short_ref};
use crate::state::{ProjectId, Section, Selection, Store, snack};
use crate::theme::Theme;
use crate::transfer::{Move, Moving, Stop};
use crate::ui::fold::{self, on_top};
use crate::ui::{Icon, ROW_HEIGHT, dp, fixed, fold_glyph, guide, h_flex, icon, small_icon};

/// The move shown in `project`: the one under way, or one Return landed, until the reload
/// after it shows where it went.
fn shown<'a>(store: &'a Store, project: &ProjectId) -> Option<&'a Move> {
    (store.moving.as_ref().or_else(|| store.landing())).filter(|moving| moving.project == *project)
}

/// The move shown in `project`, when it lands what it moves somewhere at a stop `at` picks
/// out: the rows show copies of it there.
pub(super) fn landing_at<'a>(
    store: &'a Store,
    project: &ProjectId,
    at: impl Fn(&Stop) -> bool,
) -> Option<&'a Move> {
    shown(store, project).filter(|moving| at(&moving.at) && moving.lands(store))
}

/// Whether `item` moves with the move under way and would land elsewhere: `false` while the
/// move is at where it is, and `None` for a row that doesn't move. A branch's commits move
/// with it, dimmed once it'd land elsewhere; its own row shows where it is.
pub(super) fn moving_away(store: &Store, item: &Selection) -> Option<bool> {
    let moving = shown(store, item.project())?;
    if moving.holds(item) {
        return Some(moving.lands(store));
    }
    let (Moving::Branch(branch), Selection::Commit(project, id)) = (&moving.what, item) else {
        return None;
    };
    let (on, _) = store.find_commit(project, *id)?;
    (on.ref_name.as_ref() == Some(branch) && moving.lands(store)).then_some(true)
}

/// Whether `item`'s row is what's moving, left where it is while its copy shows where it'd
/// land elsewhere.
pub(super) fn left_behind(store: &Store, item: &Selection) -> bool {
    shown(store, item.project()).is_some_and(|moving| moving.holds(item) && moving.lands(store))
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
    /// A commit being moved, dimmed where the move left it, as it goes back there or lands.
    Commit(ProjectId, Box<Commit>),
    /// A branch being moved, as [`branch_copy`] draws it, dimmed where the move left it.
    Branch {
        id: String,
        name: String,
        commits: usize,
    },
    /// The new branch commits being moved would land in at that stop.
    NewBranch(Stop),
}

/// The rows keyed `old` that a move takes away as they go to `new`, the commits where it left
/// them and the new branch they'd land in, each by its index and that of the nearest row above
/// it that stays.
fn taken_away(old: &[Option<RowKey>], new: &[Option<RowKey>]) -> Vec<(usize, usize)> {
    let kept: HashSet<&RowKey> = new.iter().flatten().collect();
    let stays =
        |ix: &usize| (old.get(*ix).and_then(Option::as_ref)).is_some_and(|key| kept.contains(key));
    (old.iter().enumerate())
        .filter(|(_, key)| {
            matches!(key, Some(key @ (RowKey::Left(..) | RowKey::GhostBranch(_))) if !kept.contains(key))
        })
        .filter_map(|(ix, _)| Some((ix, (0..ix).rev().find(stays)?)))
        .collect()
}

impl Rows {
    /// Copies of what `moving` moves: its commits, under the new branch they'd land in with
    /// `new_branch`, or its branch's row, which heads its commits wherever it goes.
    pub(super) fn push_ghosts(&mut self, moving: &Move, new_branch: bool) {
        let project = &moving.project;
        let ids = match &moving.what {
            Moving::Commits(ids) => ids,
            Moving::Branch(branch) => {
                let item = Selection::Branch(project.clone(), Some(branch.clone()));
                return self.push(Row::Ghost(item));
            }
        };
        let ghosts = (ids.iter()).map(|id| Row::Ghost(Selection::Commit(project.clone(), *id)));
        if !new_branch {
            return self.extend(ghosts);
        }
        self.push(Row::GhostBranch(moving.project.clone(), moving.at.clone()));
        self.open();
        self.extend(ghosts);
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

impl Navigator {
    /// Stand-ins for the rows as last drawn that a move takes away at `now` as they go to those
    /// keyed `keys`.
    pub(super) fn stand_ins(
        &self,
        keys: &[Option<RowKey>],
        now: Instant,
        store: &Store,
    ) -> Vec<StandIn> {
        (taken_away(&self.row_keys, keys).into_iter())
            .filter_map(|(ix, above)| {
                let of = match self.rows.get(ix)? {
                    Row::Item(Selection::Commit(project, id)) => {
                        let (_, commit) = store.find_commit(project, *id)?;
                        Gone::Commit(project.clone(), Box::new(commit.clone()))
                    }
                    Row::Item(Selection::Branch(project, ref_name @ Some(_))) => {
                        let workspace = store.workspace(project)?;
                        let branch = workspace.find_branch(ref_name.as_ref())?;
                        Gone::Branch {
                            id: format!("stand-in-{project}-{}", branch.name),
                            name: store.name_of(project, branch).to_owned(),
                            commits: branch.commits.len(),
                        }
                    }
                    Row::GhostBranch(_, at) => Gone::NewBranch(at.clone()),
                    _ => return None,
                };
                Some(StandIn {
                    of,
                    under: self.row_keys.get(above)?.clone()?,
                    by: span_height(&self.rows, above..ix),
                    inset: self.insets.get(ix).copied().unwrap_or_default(),
                    guides: self.guides.get(ix).cloned().unwrap_or_default(),
                    since: now,
                    shown: 1.,
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

    /// Where `what` of `project` can land as the rows are now, as [`stops`] says, and which of
    /// those places is where it is.
    fn stops_for(
        &self,
        project: &ProjectId,
        what: &Moving,
        cx: &App,
    ) -> Option<(Vec<Stop>, usize)> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        // As the rows were last built; the filter shows what it finds, folds aside.
        let open = |heading: &Selection| {
            let fold = Fold::Under(heading.clone());
            self.folds_seen.get(&fold).copied().unwrap_or(true)
        };
        let commits = match what {
            Moving::Commits(ids) => &ids[..],
            Moving::Branch(_) => &[],
        };
        let all = stops(self.shown_rows(), project, commits, workspace, open);
        let Moving::Branch(branch) = what else {
            return Some(all);
        };
        let lands = |at: &Stop| {
            let (project, what, at) = (project.clone(), what.clone(), at.clone());
            Move { project, what, at }.lands(store)
        };
        branch_stops(all.0, branch, lands)
    }

    /// The stops of the move under way, and which of them it's at: where what it moves is, if
    /// the one it was at is gone, as when it's folded away.
    fn move_stops(&self, cx: &App) -> Option<(Vec<Stop>, usize)> {
        let moving = self.store.read(cx).moving.as_ref()?;
        let (stops, start) = self.stops_for(&moving.project, &moving.what, cx)?;
        let at = (stops.iter().position(|stop| *stop == moving.at)).unwrap_or(start);
        Some((stops, at))
    }

    /// Start moving the selected commit, with those picked with it, or the selected branch,
    /// from where it is. The keys that step through the rows step through where it can land
    /// until Return lands it or Escape lets go.
    pub(super) fn start_move(
        &mut self,
        _: &MoveSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let (project, what) = match store.selection.clone() {
            Some(Selection::Commit(project, id)) => {
                let commits = store.with_picks(&project, id);
                (project, Moving::Commits(commits))
            }
            Some(Selection::Branch(project, Some(branch))) => (project, Moving::Branch(branch)),
            _ => return,
        };
        // Somewhere besides where it is.
        let start = (self.stops_for(&project, &what, cx))
            .filter(|(stops, _)| stops.len() > 1)
            .and_then(|(stops, start)| stops.into_iter().nth(start));
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to move it", cx);
        };
        let moving = Move { project, what, at };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
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
    /// branches, jumps between the tops of stacks.
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
        let heads = |(_, stop): &(usize, &Stop)| match stop {
            Stop::In(Destination::Branch(_)) | Stop::NewStack { .. } => true,
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

    /// Land what's being moved where the move is at, as dropping it there would.
    pub(super) fn move_here(&mut self, _: &MoveHere, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(moving) = store.moving.clone() else {
            return;
        };
        let lands = moving.lands(store);
        self.store
            .update(cx, |store, cx| store.set_moving(None, cx));
        // Where they already are, there's nothing to do.
        if !lands {
            return;
        }
        // A branch's copy stays where it lands till it's there.
        if let Some((payload, target, place)) = moving.drop() {
            return self.app.update_later(window, cx, move |app, window, cx| {
                app.land(&payload, &target, place, Some(moving), window, cx)
            });
        }
        let lands = Some(moving.clone());
        let Move { project, what, at } = moving;
        self.store.update(cx, |store, cx| match (what, at) {
            (Moving::Commits(commits), Stop::NewBranch { beside, below }) => {
                store.move_to_new_branch(&project, commits, Some((beside, below)), false, cx)
            }
            (Moving::Commits(commits), Stop::NewStack { first }) => {
                store.move_to_new_branch(&project, commits, None, first, cx)
            }
            (Moving::Branch(branch), Stop::NewStack { first }) => {
                store.unstack(&project, branch, first, lands, cx)
            }
            // Those land as drops do, above.
            (Moving::Commits(_), Stop::In(_)) | (Moving::Branch(_), _) => {}
        });
    }
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
            Stop::In(_) => false,
        })
        .collect();
    let start = stops.iter().position(|stop| *stop == home)?;
    Some((stops, start))
}

/// A copy of `item` being moved, a commit or a branch, where landing would put it, drawn as
/// its own row is while the move is at where it is, set in by `inset` as an item row is. A
/// branch's copy is its row alone, folded, which heads its commits wherever it goes.
pub(super) fn ghost_row(item: &Selection, inset: f32, store: &Store, theme: &Theme) -> AnyElement {
    let style = RowStyle {
        held: true,
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
                branch_copy(id, name, branch.commits.len(), style)
            }),
        _ => None,
    };
    row.unwrap_or_else(|| div().h(dp(ROW_HEIGHT)).into_any_element())
}

/// Branch `name`'s row as a copy draws it, row `id`, in `style`: folded, with how many
/// `commits` it holds, as its own row shows them folded, and nothing on it acts.
fn branch_copy(id: String, name: String, commits: usize, style: RowStyle) -> AnyElement {
    (style.row(id.clone(), 6.))
        .child(fold_glyph(format!("{id}-fold"), Some(false), style.quiet()))
        .child(icon(Icon::Branch).text_color(style.muted()))
        .child(div().flex_1().min_w_0().truncate().child(name))
        .children(style.commits(commits))
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
        ..
    } = stand_in;
    let row = match of {
        Gone::Commit(project, commit) => {
            let style = RowStyle {
                cut: true,
                ..RowStyle::plain(theme, *inset)
            };
            let id = format!("stand-in-{project}-{}", commit.id.0);
            style.commit(id, project, commit, store).into_any_element()
        }
        Gone::Branch { id, name, commits } => {
            let style = RowStyle {
                cut: true,
                ..RowStyle::plain(theme, *inset)
            };
            branch_copy(id.clone(), name.clone(), *commits, style)
        }
        Gone::NewBranch(at) => ghost_branch_row(at, *inset, theme),
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

/// The new branch commits being moved would land in at stop `at`, over their copies, set in by
/// `inset` as an item row is. It names the branch it goes above or below in their stack, or
/// says it's independent, in a stack of its own.
pub(super) fn ghost_branch_row(at: &Stop, inset: f32, theme: &Theme) -> AnyElement {
    let beside = match at {
        Stop::NewBranch { beside, below } => {
            let side = if *below { "below" } else { "above" };
            Some((side, short_ref(beside).into_owned()))
        }
        Stop::In(_) | Stop::NewStack { .. } => None,
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
        .border_color(theme.pop)
        .text_color(theme.pop)
        .child(small_icon(Icon::Branch).text_color(theme.pop))
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
    use super::{Row, RowKey, branch_stops, stops, taken_away};
    use crate::backend::model::build::{branch, full, id, workspace, worktree};
    use crate::backend::{CommitId, Destination, Workspace, short_ref};
    use crate::state::{ProjectId, Selection};
    use crate::terminal::TerminalId;
    use crate::transfer::Stop;

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
            taken_away(&above_api, &top_of_api),
            [(3, 2)],
            "stepping on, the new branch goes, under commit 2"
        );
        let landed = [branch("ui"), commit(2), branch("api"), commit(3)];
        assert_eq!(
            taken_away(&top_of_api, &landed),
            [(1, 0)],
            "landing, where it was left goes, under ui"
        );
        let back = [branch("ui"), commit(3), commit(2), branch("api")];
        assert_eq!(
            taken_away(&above_api, &back),
            [(1, 0), (3, 2)],
            "letting go, both go, each under the row above it that stays"
        );
        let both = [
            branch("ui"),
            left(3),
            left(2),
            commit(1),
            commit(3),
            commit(2),
        ];
        let home = [branch("ui"), commit(3), commit(2), commit(1)];
        assert_eq!(
            taken_away(&both, &home),
            [(1, 0), (2, 0)],
            "commits moving together go under the row above the first"
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
            Stop::In(_) | Stop::NewStack { .. } => true,
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
}
