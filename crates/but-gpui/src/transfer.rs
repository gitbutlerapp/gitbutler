//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{
    App, Context, Div, Entity, Modifiers, Pixels, Point, SharedString, Window, div, prelude::*,
};

use crate::backend::{
    self, Checkout, CommitId, CommitTo, Destination, FullName, Selected, Workspace, short_ref,
};
use crate::state::{
    Cut, Label, NewAt, Overlay, ProjectId, Selection, Store, branch_moved, move_before,
};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::diff::split_path;
use crate::ui::file_icons::{file_icon, folder_icon};
use crate::ui::{Icon, count_badge, dp, h_flex, popover, small_icon};

/// What a drop or paste does. Commits move, squash and uncommit as many as were picked.
pub enum Transfer {
    Uncommit(Arc<[CommitId]>),
    UncommitFiles {
        from: CommitId,
        files: Arc<[Selected]>,
    },
    MoveCommit {
        ids: Arc<[CommitId]>,
        to: Destination,
    },
    Squash {
        ids: Arc<[CommitId]>,
        into: CommitId,
    },
    /// Uncommitted files of `worktree` into a commit.
    Amend {
        into: CommitId,
        files: Arc<[Selected]>,
        worktree: Checkout,
    },
    MoveFiles {
        from: CommitId,
        to: CommitId,
        files: Arc<[Selected]>,
    },
    /// Copies of the commits, as listed, leaving them where they are.
    CherryPick {
        ids: Arc<[CommitId]>,
        to: Destination,
    },
    /// Stack a branch on another, leaving those above it where they were.
    StackBranch {
        branch: FullName,
        onto: FullName,
    },
    /// Put a branch under another, right below that one's last commit.
    BranchBelow {
        branch: FullName,
        commit: CommitId,
    },
    /// Uncommitted files dropped on a branch: get them ready to commit there.
    StageOnBranch {
        branch: FullName,
        files: Arc<[Selected]>,
    },
    /// Uncommitted files the composer can't take, a linked worktree's or bound for one,
    /// committed on a branch with no message yet, as Lite does.
    CommitOnBranch {
        branch: FullName,
        files: Arc<[Selected]>,
        worktree: Checkout,
    },
    /// Put a linked worktree before another in the navigator, or after them all.
    MoveWorktree {
        name: bstr::BString,
        before: Option<bstr::BString>,
    },
}

/// Where on its target a drop or paste lands. Only a commit tells all three apart: above or
/// below it, or squashed into it. Anything else takes what lands on it, or with ⇧ a branch
/// goes below another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Above,
    Into,
    Below,
}

impl Place {
    /// Where a paste of `payload` with `held` lands on `target`: a commit, or a copy of one,
    /// goes above a commit, below it with ⇧, or into it with ⌥, and files go into it; into
    /// anything else, or below a branch with ⇧.
    pub fn from_keys(held: Modifiers, payload: &Cut, target: &Selection) -> Self {
        let commit = matches!(payload, Cut::Commit(..) | Cut::CherryPick(..));
        match target {
            Selection::Commit(..) if held.alt || !commit => Place::Into,
            Selection::Commit(..) if !held.shift => Place::Above,
            _ if held.shift => Place::Below,
            _ => Place::Into,
        }
    }
}

/// Something being moved with the keys, in `project`: the navigator steps through the places
/// it can land, and Return lands it where it's at.
#[derive(Debug, Clone)]
pub struct Move {
    pub project: ProjectId,
    pub what: Moving,
    pub at: Stop,
}

/// What a [`Move`] moves. Commits and branches show a copy where they'd land while they stay
/// where they are, as landing them rewrites history. A project's card and a terminal go where
/// they'd land themselves, the others making room, as a drag shows a card, as their places are
/// only the app's to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moving {
    /// Commits, as listed, newest first.
    Commits(Arc<[CommitId]>),
    /// A branch, with its commits: between branches, as a new branch made of commits goes.
    Branch(FullName),
    /// A watched project's card, among the others in `order`, as they were listed when the
    /// move started, which they keep while it lasts.
    Project(Arc<[ProjectId]>),
    /// Terminal `id`, among the terminals of the checkouts `lists` has, each with those open on
    /// it in order, as they were listed when the move started, which they keep while it lasts.
    /// Landing it on another checkout lists it there; its shell stays where it started.
    Terminal {
        id: TerminalId,
        lists: Arc<[(Checkout, Vec<TerminalId>)]>,
    },
}

/// A place what's being moved can land. A branch lands between branches, where a new branch
/// would, a project between cards, and a terminal between the terminals of a worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// In a branch: at its top, or right below one of its commits.
    In(Destination),
    /// In a new branch right above `beside` in its stack, or right below it with `below`.
    NewBranch { beside: FullName, below: bool },
    /// In a new branch in a stack of its own, after the others, or before them with `first`.
    NewStack { first: bool },
    /// Among the watched projects' cards.
    Card(CardSlot),
    /// Among the terminals listed under `checkout`.
    Terminal {
        checkout: Checkout,
        slot: Slot<TerminalId>,
    },
}

/// Where something the app keeps in an order of its own goes among the others like it: before
/// one of them, or after them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot<T> {
    Before(T),
    End,
}

/// Where a watched project's card goes.
pub type CardSlot = Slot<ProjectId>;

impl<T: Clone + PartialEq> Slot<T> {
    /// `list` with `id` put in this slot, taken from wherever it was in it.
    fn place(&self, list: &[T], id: &T) -> Vec<T> {
        let mut list: Vec<T> = (list.iter())
            .filter(|other| *other != id)
            .cloned()
            .collect();
        let at = match self {
            Slot::Before(before) => list.iter().position(|other| other == before),
            Slot::End => None,
        };
        list.insert(at.unwrap_or(list.len()), id.clone());
        list
    }
}

/// The places `id` can go among those `order` lists, and which of them is where it is: before
/// each of the others, or after them all.
pub fn slots<T: Clone + PartialEq>(order: &[T], id: &T) -> Option<(Vec<Slot<T>>, usize)> {
    let (slots, start) = group_slots(&[((), order.to_vec())], id)?;
    Some((slots.into_iter().map(|((), slot)| slot).collect(), start))
}

/// Places in groups, each by its group's key, and which of them is where what's moving is.
pub type GroupSlots<K, T> = (Vec<(K, Slot<T>)>, usize);

/// The places `id` can go among the groups `groups` has, each by its key with what's in it, in
/// order: in each group before each of the others in it, or after them all. And which of them
/// is where it is.
pub fn group_slots<K: Clone + PartialEq, T: Clone + PartialEq>(
    groups: &[(K, Vec<T>)],
    id: &T,
) -> Option<GroupSlots<K, T>> {
    let (mut slots, mut home) = (Vec::new(), None);
    for (key, list) in groups {
        if let Some(at) = list.iter().position(|other| other == id) {
            let next = list.get(at + 1).cloned();
            home = Some((key.clone(), next.map_or(Slot::End, Slot::Before)));
        }
        let others = (list.iter()).filter(|other| *other != id);
        slots.extend(others.map(|other| (key.clone(), Slot::Before(other.clone()))));
        slots.push((key.clone(), Slot::End));
    }
    let home = home?;
    let start = slots.iter().position(|slot| *slot == home)?;
    Some((slots, start))
}

impl Move {
    /// What's moving, as what's cut or dragged. A project's card and a terminal aren't cut.
    fn cut(&self) -> Option<Cut> {
        Some(match &self.what {
            Moving::Commits(ids) => Cut::Commit(self.project.clone(), ids.clone()),
            Moving::Branch(branch) => Cut::Branch(self.project.clone(), branch.clone()),
            Moving::Project(_) | Moving::Terminal { .. } => return None,
        })
    }

    /// What's moving, as the status bar names it.
    pub fn describe(&self, store: &Store) -> String {
        match &self.what {
            Moving::Project(_) => project_name(store, &self.project),
            Moving::Terminal { id, .. } => terminal_title(store, *id),
            Moving::Commits(_) | Moving::Branch(_) => {
                (self.cut()).map_or_else(String::new, |cut| cut.describe(store))
            }
        }
    }

    /// Whether `item` is what's moving: one of the commits, the branch, the project or the
    /// terminal.
    pub fn holds(&self, item: &Selection) -> bool {
        match (&self.what, item) {
            (Moving::Commits(ids), Selection::Commit(project, id)) => {
                *project == self.project && ids.contains(id)
            }
            (Moving::Branch(branch), Selection::Branch(project, Some(name))) => {
                *project == self.project && name == branch
            }
            (Moving::Project(_), Selection::Project(project)) => *project == self.project,
            (Moving::Terminal { id, .. }, Selection::Terminal(project, terminal)) => {
                *project == self.project && terminal == id
            }
            _ => false,
        }
    }

    /// The watched projects in the order a project's move shows them, its card where it's at.
    pub fn project_order(&self) -> Option<Vec<ProjectId>> {
        let (Moving::Project(order), Stop::Card(slot)) = (&self.what, &self.at) else {
            return None;
        };
        Some(slot.place(order, &self.project))
    }

    /// The checkouts' terminals as a terminal's move shows them, it where it's at.
    pub fn terminal_order(&self) -> Option<Vec<(Checkout, Vec<TerminalId>)>> {
        let (Moving::Terminal { id, lists }, Stop::Terminal { checkout, slot }) =
            (&self.what, &self.at)
        else {
            return None;
        };
        let lists = lists.iter().map(|(on, list)| {
            let list = match on == checkout {
                true => slot.place(list, id),
                false => (list.iter())
                    .filter(|other| *other != id)
                    .copied()
                    .collect(),
            };
            (on.clone(), list)
        });
        Some(lists.collect())
    }

    /// Landing as a drop: what's moving as what's dropped, and the row and the place on it
    /// it's dropped at. `None` in a new branch or stack, which has no row yet.
    pub fn drop(&self) -> Option<(Cut, Selection, Place)> {
        let project = self.project.clone();
        let (target, place) = match (&self.what, &self.at) {
            (Moving::Commits(_), Stop::In(to)) => match to {
                Destination::Branch(branch) => (
                    Selection::Branch(project, Some(branch.clone())),
                    Place::Into,
                ),
                Destination::AboveCommit(id) => (Selection::Commit(project, *id), Place::Above),
                Destination::BelowCommit(id) => (Selection::Commit(project, *id), Place::Below),
            },
            // On top of the branch it goes above, or under the one it goes below.
            (Moving::Branch(_), Stop::NewBranch { beside, below }) => {
                let place = if *below { Place::Below } else { Place::Into };
                (Selection::Branch(project, Some(beside.clone())), place)
            }
            _ => return None,
        };
        Some((self.cut()?, target, place))
    }

    /// Whether landing moves anything, as it doesn't where it already is: a branch alone in
    /// its stack is a stack of its own, and a card is where it was.
    pub fn lands(&self, store: &Store) -> bool {
        if let (Moving::Branch(branch), Stop::NewStack { .. }) = (&self.what, &self.at) {
            let workspace = store.workspace(&self.project);
            return workspace.is_some_and(|ws| crate::edits::BranchEdits::of(ws, branch).stacked);
        }
        match &self.what {
            Moving::Project(order) => {
                return (self.project_order()).is_some_and(|shown| shown[..] != order[..]);
            }
            Moving::Terminal { lists, .. } => {
                return (self.terminal_order()).is_some_and(|shown| shown[..] != lists[..]);
            }
            Moving::Commits(_) | Moving::Branch(_) => {}
        }
        self.drop().is_none_or(|(payload, target, place)| {
            Transfer::resolve(&payload, &target, place, store).is_some()
        })
    }

    /// Where landing puts it, said after a verb: "to a new branch below feature-x".
    pub fn place(&self, store: &Store) -> String {
        let moves_branch = matches!(self.what, Moving::Branch(_));
        match &self.at {
            Stop::In(Destination::Branch(branch)) => {
                format!("to the top of {}", short_ref(branch))
            }
            Stop::In(to) => place(to),
            Stop::NewBranch { beside, below } => {
                let side = if *below { "below" } else { "above" };
                match moves_branch {
                    true => format!("{side} {}", short_ref(beside)),
                    false => format!("to a new branch {side} {}", short_ref(beside)),
                }
            }
            Stop::NewStack { first: false } => "to a new stack".into(),
            Stop::NewStack { first: true } => "to a new stack at the top".into(),
            Stop::Card(Slot::Before(before)) => format!("above {}", project_name(store, before)),
            // Below the last of the others.
            Stop::Card(Slot::End) => {
                let last = match &self.what {
                    Moving::Project(order) => order.iter().rev().find(|id| **id != self.project),
                    _ => None,
                };
                last.map_or_else(
                    || "to the bottom".into(),
                    |last| format!("below {}", project_name(store, last)),
                )
            }
            Stop::Terminal { checkout, slot } => self.terminal_place(checkout, slot, store),
        }
    }

    /// Where landing a terminal at `slot` among `checkout`'s puts it, naming the worktree when
    /// it's another than its own.
    fn terminal_place(
        &self,
        checkout: &Checkout,
        slot: &Slot<TerminalId>,
        store: &Store,
    ) -> String {
        let Moving::Terminal { id, lists } = &self.what else {
            return String::new();
        };
        let home = (lists.iter()).find_map(|(on, list)| list.contains(id).then_some(on));
        let others: Vec<TerminalId> = (lists.iter())
            .find(|(on, _)| on == checkout)
            .map(|(_, list)| list.iter().filter(|other| *other != id).copied().collect())
            .unwrap_or_default();
        let worktree = match checkout {
            Checkout::Main => "the main worktree".to_owned(),
            Checkout::Linked(name) => name.to_string(),
        };
        let at = match (slot, others.last()) {
            (Slot::Before(before), _) => format!("above {}", terminal_title(store, *before)),
            (Slot::End, Some(last)) => format!("below {}", terminal_title(store, *last)),
            (Slot::End, None) => return format!("to {worktree}"),
        };
        match home == Some(checkout) {
            true => at,
            false => format!("{at} in {worktree}"),
        }
    }
}

/// Project `id`'s name, or its id once it's gone.
fn project_name(store: &Store, id: &ProjectId) -> String {
    (store.project(id)).map_or_else(|| id.to_string(), |entry| entry.project.name.clone())
}

/// Terminal `id`'s title, as its row shows it.
fn terminal_title(store: &Store, id: TerminalId) -> String {
    (store.terminals.title(id)).map_or_else(|| "a terminal".into(), ToString::to_string)
}

impl Transfer {
    /// What `payload` does when it lands on `target` at `place`, if anything. A commit landing
    /// on another goes above or below it, or squashes into it. A branch landing on another
    /// stacks on it, or goes below it.
    pub fn resolve(payload: &Cut, target: &Selection, place: Place, store: &Store) -> Option<Self> {
        if payload.project() != target.project() {
            return None;
        }
        // A folder with nothing in it that can move carries nothing.
        if matches!(payload, Cut::Files { files, .. } if files.is_empty()) {
            return None;
        }
        // An unnamed branch has no ref to put things on or stack onto.
        if matches!(target, Selection::Branch(_, None)) {
            return None;
        }
        // Only a commit, or a copy of one, goes above or below another; the rest go into it.
        if matches!(target, Selection::Commit(..))
            && place != Place::Into
            && !matches!(payload, Cut::Commit(..) | Cut::CherryPick(..))
        {
            return None;
        }
        // A branch moves among its own: a linked worktree's within its lane, the workspace's
        // among its stacks, and yours under a worktree's from its heading. Across, the
        // worktree would be left detached and the commits the branch started from would
        // come along.
        if let Cut::Branch(project, branch) = payload {
            let side = |s: &Selection| store.worktree_for(s).map(|wt| wt.name.clone());
            let from = side(&Selection::Branch(project.clone(), Some(branch.clone())));
            let stays = match target {
                Selection::Worktree(..) => from.is_none(),
                _ => from == side(target),
            };
            if !stays {
                return None;
            }
        }
        let workspace = store.workspace(target.project())?;
        // Work goes on a branch of its own, not the default branch a linked worktree can have
        // checked out: commits there would leave it apart from its upstream.
        if let Selection::Branch(_, Some(branch)) = target
            && workspace.target_branch.as_ref() == Some(branch)
        {
            return None;
        }
        let conflicted =
            |id: CommitId| workspace.find_commit(id).is_some_and(|(_, c)| c.conflicted);
        Some(match (payload, target) {
            (Cut::Commit(project, ids), Selection::Uncommitted(_) | Selection::Worktree(..))
                if ids
                    .iter()
                    .all(|id| uncommits_into(store, project, *id, target) && !conflicted(*id)) =>
            {
                Transfer::Uncommit(ids.clone())
            }
            (Cut::Commit(_, ids), Selection::Branch(_, Some(branch))) => {
                let to = Destination::Branch(branch.clone());
                if already_at(workspace, ids, &to) {
                    return None;
                }
                Transfer::MoveCommit {
                    ids: ids.clone(),
                    to,
                }
            }
            (Cut::Commit(_, ids), Selection::Commit(_, target_id)) if !ids.contains(target_id) => {
                let to = match place {
                    // Not with a conflicted commit on either side, whose change the squash
                    // would drop, or the engine refuses.
                    Place::Into => {
                        let clean = !(ids.iter().chain([target_id])).any(|id| conflicted(*id));
                        return clean.then(|| Transfer::Squash {
                            ids: ids.clone(),
                            into: *target_id,
                        });
                    }
                    Place::Below => Destination::BelowCommit(*target_id),
                    Place::Above => Destination::AboveCommit(*target_id),
                };
                if already_at(workspace, ids, &to) {
                    return None;
                }
                Transfer::MoveCommit {
                    ids: ids.clone(),
                    to,
                }
            }
            (Cut::CherryPick(_, ids), Selection::Branch(_, Some(branch))) => Transfer::CherryPick {
                ids: ids.clone(),
                to: Destination::Branch(branch.clone()),
            },
            (Cut::CherryPick(_, ids), Selection::Commit(_, target_id))
                if !ids.contains(target_id) =>
            {
                Transfer::CherryPick {
                    ids: ids.clone(),
                    to: match place {
                        Place::Above => Destination::AboveCommit(*target_id),
                        Place::Below => Destination::BelowCommit(*target_id),
                        Place::Into => return None,
                    },
                }
            }
            (Cut::Branch(_, branch), Selection::Branch(_, Some(above)))
                if place == Place::Below && branch != above =>
            {
                let (lane, at) = workspace.lane_of(Some(above))?;
                // Already right below it.
                if lane
                    .branches
                    .get(at + 1)
                    .is_some_and(|b| b.ref_name.as_ref() == Some(branch))
                {
                    return None;
                }
                Transfer::BranchBelow {
                    branch: branch.clone(),
                    commit: lane.branches.get(at)?.commits.last()?.id,
                }
            }
            (Cut::Branch(_, branch), Selection::Worktree(_, name)) => Transfer::BranchBelow {
                branch: branch.clone(),
                commit: workspace.worktree_lane(name.as_ref())?.end()?,
            },
            (Cut::Branch(_, branch), Selection::Branch(_, Some(onto))) if branch != onto => {
                // Already right on top of it.
                let (lane, at) = workspace.lane_of(Some(branch))?;
                if lane
                    .branches
                    .get(at + 1)
                    .is_some_and(|b| b.ref_name.as_ref() == Some(onto))
                {
                    return None;
                }
                Transfer::StackBranch {
                    branch: branch.clone(),
                    onto: onto.clone(),
                }
            }
            (Cut::Worktree(project, name), Selection::Worktree(_, onto)) => {
                worktree_move(store, project, name.as_ref(), onto.as_ref(), place)?
            }
            (
                Cut::Files {
                    project,
                    from,
                    worktree,
                    files,
                },
                target,
            ) => match (*from, target) {
                // The engine won't amend a conflicted commit, nor move changes in or out of one.
                _ if from.is_some_and(conflicted) => return None,
                (None, Selection::Commit(_, into)) if !conflicted(*into) => Transfer::Amend {
                    into: *into,
                    files: files.clone(),
                    worktree: worktree.clone(),
                },
                // The composer commits the main checkout's files to the workspace's branches.
                (None, Selection::Branch(_, Some(branch)))
                    if !worktree.is_linked() && !store.in_linked(target) =>
                {
                    Transfer::StageOnBranch {
                        branch: branch.clone(),
                        files: files.clone(),
                    }
                }
                (None, Selection::Branch(_, Some(branch))) => Transfer::CommitOnBranch {
                    branch: branch.clone(),
                    files: files.clone(),
                    worktree: worktree.clone(),
                },
                (Some(from), Selection::Uncommitted(_) | Selection::Worktree(..))
                    if uncommits_into(store, project, from, target) =>
                {
                    Transfer::UncommitFiles {
                        from,
                        files: files.clone(),
                    }
                }
                // The worktree's own files, from its heading or its list, move it as it would.
                (None, Selection::Worktree(_, onto)) => {
                    worktree_move(store, project, worktree.linked()?, onto.as_ref(), place)?
                }
                (Some(from), Selection::Commit(_, to)) if from != *to && !conflicted(*to) => {
                    Transfer::MoveFiles {
                        from,
                        to: *to,
                        files: files.clone(),
                    }
                }
                _ => return None,
            },
            _ => return None,
        })
    }

    /// What a drop does, said in the drag's pill: the row under the pointer shows where.
    pub fn verb(&self) -> &'static str {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit",
            Transfer::MoveCommit {
                to: Destination::Branch(_),
                ..
            } => "Move to top",
            Transfer::MoveCommit { .. } => "Move here",
            Transfer::CherryPick {
                to: Destination::Branch(_),
                ..
            } => "Cherry-pick to top",
            Transfer::CherryPick { .. } => "Cherry-pick here",
            Transfer::StackBranch { .. } => "Stack on top",
            Transfer::BranchBelow { .. } => "Move below",
            Transfer::Squash { .. } => "Squash into",
            Transfer::Amend { .. } => "Amend",
            Transfer::MoveFiles { .. } => "Move into",
            Transfer::StageOnBranch { .. } => "Start a commit here",
            Transfer::CommitOnBranch { .. } => "Commit here",
            Transfer::MoveWorktree { .. } => "Move here",
        }
    }

    /// A short verb phrase for hints, like "Squash into a1b2c3d".
    pub fn describe(&self) -> SharedString {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit".into(),
            Transfer::MoveCommit { to, .. } => format!("Move {}", place(to)).into(),
            Transfer::CherryPick { to, .. } => format!("Cherry-pick {}", place(to)).into(),
            Transfer::StackBranch { onto, .. } => format!("Stack on {}", short_ref(onto)).into(),
            Transfer::BranchBelow { commit, .. } => format!("Move below {}", commit.short()).into(),
            Transfer::Squash { into, .. } => format!("Squash into {}", into.short()).into(),
            Transfer::Amend { into, .. } => format!("Amend {}", into.short()).into(),
            Transfer::MoveFiles { to, .. } => format!("Move into {}", to.short()).into(),
            Transfer::StageOnBranch { branch, .. } => {
                format!("Start a commit on {}", short_ref(branch)).into()
            }
            Transfer::CommitOnBranch { branch, .. } => {
                format!("Commit to {}", short_ref(branch)).into()
            }
            Transfer::MoveWorktree { name, .. } => format!("Move {name}").into(),
        }
    }

    /// Carry it out. Staging only changes the composer, so it comes back to the caller.
    /// Do it in `project`, landing the move with the keys `lands` if it's a branch's.
    pub fn perform(
        self,
        project: &SharedString,
        lands: Option<Move>,
        store: &mut Store,
        cx: &mut Context<Store>,
    ) -> Option<(FullName, Arc<[Selected]>)> {
        let lands = lands.filter(|moving| matches!(moving.what, Moving::Branch(_)));
        // The pill's verb names it without a branch or commit.
        let label = Label::new(self.verb(), self.describe());
        // Picked commits that moved, or went into one, are no longer what's picked.
        if let Transfer::MoveCommit { ids, .. } | Transfer::Squash { ids, .. } = &self
            && ids.len() > 1
        {
            store.picked.clear();
        }
        match self {
            Transfer::Uncommit(ids) => store.uncommit(project, ids, cx),
            Transfer::UncommitFiles { from, files } => {
                // Where they land is selected, as the TUI goes on to it.
                let landed = (store.checkout_of(&Selection::Commit(project.clone(), from)))
                    .map(|checkout| Selection::uncommitted_in(project.clone(), checkout));
                store.apply(
                    project,
                    label,
                    Overlay::commit_files(store.key_of(project, from), files.iter()),
                    backend::UncommitFiles {
                        commit: from,
                        files,
                    },
                    move |store, rewritten, cx| {
                        match landed {
                            Some(landed) => store.select(Some(landed), cx),
                            None => store.follow(&rewritten, cx),
                        }
                        store.snack_undoable("Moved to uncommitted changes", cx)
                    },
                    cx,
                )
            }
            // It leaves where it was and shows dimmed where it goes.
            Transfer::MoveCommit { ids, to } => store.apply(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, &ids, &to),
                    ..Overlay::commits(ids.iter().filter_map(|id| store.key_of(project, *id)))
                },
                backend::MoveCommits { commits: ids, to },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Moved", cx)
                },
                cx,
            ),
            Transfer::CherryPick { ids, to } => store.apply(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, &ids, &to),
                    ..Overlay::default()
                },
                backend::CherryPick { commits: ids, to },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Cherry-picked", cx)
                },
                cx,
            ),
            Transfer::StackBranch { branch, onto } => store.apply(
                project,
                label,
                Overlay {
                    lands,
                    ..Overlay::default()
                },
                backend::StackBranch { branch, onto },
                move |store, conflicted, cx| {
                    store.snack_undoable(branch_moved("Branch moved", &conflicted), cx)
                },
                cx,
            ),
            Transfer::BranchBelow { branch, commit } => store.apply(
                project,
                label,
                Overlay {
                    lands,
                    ..Overlay::default()
                },
                backend::MoveBranchBelow { branch, commit },
                move |store, conflicted, cx| {
                    store.snack_undoable(branch_moved("Branch moved", &conflicted), cx)
                },
                cx,
            ),
            Transfer::Squash { ids, into } => store.squash(project, ids, into, cx),
            Transfer::Amend {
                into,
                files,
                worktree,
            } => store.amend(project, into, files, worktree, |_, _| {}, cx),
            Transfer::MoveFiles { from, to, files } => {
                let moved = project.clone();
                store.apply(
                    project,
                    label,
                    Overlay::commit_files(store.key_of(project, from), files.iter()),
                    backend::MoveFiles { from, to, files },
                    // On the commit they went into.
                    move |store, rewritten, cx| {
                        store.follow_on(&moved, &rewritten, to, cx);
                        store.snack_undoable("Changes moved", cx)
                    },
                    cx,
                )
            }
            Transfer::StageOnBranch { branch, files } => return Some((branch, files)),
            Transfer::MoveWorktree { name, before } => store.move_worktree(
                project,
                name.as_ref(),
                before.as_ref().map(|b| b.as_ref()),
                cx,
            ),
            Transfer::CommitOnBranch {
                branch,
                files,
                worktree,
            } => {
                let selected = project.clone();
                let overlay = Overlay {
                    new_commit: Some((NewAt::Top(branch.clone()), SharedString::default())),
                    files_in: worktree.clone(),
                    ..Overlay::files(files.iter())
                };
                let commit = backend::CommitChanges {
                    to: CommitTo::Branch(branch),
                    changes: files,
                    checkout: worktree,
                    message: String::new(),
                    run_hooks: false,
                };
                store.apply(
                    project,
                    label,
                    overlay,
                    commit,
                    move |store, committed, cx| {
                        store.select_made(&selected, committed.made, cx);
                        store.keep_last_snapshot(cx);
                        let done = "Committed. Double-click its title to write a message";
                        store.report_left_out(done, &committed.left_out, cx)
                    },
                    cx,
                )
            }
        }
        None
    }
}

/// Whether `target` is the checkout commit `id`'s changes uncommit into: a linked worktree's
/// commit's go into that worktree, as that's where their checkout is, and the rest into the
/// main one.
fn uncommits_into(store: &Store, project: &ProjectId, id: CommitId, target: &Selection) -> bool {
    let home = store.worktree_for(&Selection::Commit(project.clone(), id));
    match target {
        Selection::Uncommitted(_) => home.is_none(),
        Selection::Worktree(_, name) => home.is_some_and(|worktree| worktree.name == *name),
        _ => false,
    }
}

/// Where `to` is, said after a verb: "to feature-x", "above a1b2c3d".
fn place(to: &Destination) -> String {
    match to {
        Destination::Branch(branch) => format!("to {}", short_ref(branch)),
        Destination::AboveCommit(id) => format!("above {}", id.short()),
        Destination::BelowCommit(id) => format!("below {}", id.short()),
    }
}

/// What a drag's pill names, which what it carries can't say.
#[derive(Clone)]
pub enum Chip {
    /// A row, by its label, beside the glyph of what it carries.
    Row(SharedString),
    /// A folder, by its name, which its files go together as.
    Folder(SharedString),
}

impl From<SharedString> for Chip {
    fn from(label: SharedString) -> Self {
        Chip::Row(label)
    }
}

/// Start a drag of a `Cut` shown as `chip`, letting the store know what's in flight so the
/// rows can say what a drop would do.
pub fn drag_preview(
    store: Entity<Store>,
    chip: impl Into<Chip>,
) -> impl Fn(&Cut, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static {
    let chip = chip.into();
    move |payload, offset, _, cx| {
        store.update(cx, |store, cx| {
            store.dragging = Some(payload.clone());
            cx.notify()
        });
        let preview = cx.new(|_| DragPreview {
            chip: chip.clone(),
            payload: payload.clone(),
            store: store.clone(),
            offset,
        });
        // GPUI drops the preview when the drag ends, dropped or not. Let go over a row that
        // won't take it, nothing else redraws the navigator, which would keep showing the
        // drag until the next change.
        let store = store.clone();
        cx.observe_release(&preview, move |_, cx| {
            store.update(cx, |store, cx| {
                store.dragging = None;
                store.drop_at = None;
                cx.notify()
            });
        })
        .detach();
        preview
    }
}

/// The pill that follows the pointer while dragging, what it carries, and what letting go
/// there does.
/// Linked worktree `name` landing above or below linked worktree `onto` at `place`, as the
/// navigator lists them, unless it's there already or `onto` is listed apart from it, one
/// active and the other inactive.
fn worktree_move(
    store: &Store,
    project: &ProjectId,
    name: &bstr::BStr,
    onto: &bstr::BStr,
    place: Place,
) -> Option<Transfer> {
    let workspace = store.workspace(project)?;
    let inactive = |name| workspace.find_worktree(name).map(|wt| wt.is_inactive());
    if inactive(name)? != inactive(onto)? {
        return None;
    }
    let order: Vec<&bstr::BString> = (store.worktrees_in_order(project, workspace).into_iter())
        .map(|wt| &wt.name)
        .collect();
    let at = order.iter().position(|other| *other == onto)?;
    let before = match place {
        Place::Above => order.get(at).copied(),
        Place::Below => order.get(at + 1).copied(),
        Place::Into => return None,
    };
    let dragged = order.iter().find(|other| **other == name).copied()?;
    let mut moved = order.clone();
    let moves = move_before(&mut moved, &dragged, before.as_ref()) && moved != order;
    moves.then(|| Transfer::MoveWorktree {
        name: name.to_owned(),
        before: before.cloned(),
    })
}

pub struct DragPreview {
    pub chip: Chip,
    pub payload: Cut,
    store: Entity<Store>,
    /// Where the pointer pressed the dragged row, which GPUI draws the pill from.
    offset: Point<Pixels>,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        // What's dragged as the store followed it, as an agent may have rewritten it.
        let transfer = (store.drop_at.as_ref()).and_then(|(target, place)| {
            let payload = store.dragging.as_ref()?;
            Transfer::resolve(payload, target, *place, store)
        });
        let hint = transfer.as_ref().map(Transfer::verb);
        let glyph = |icon| small_icon(icon).text_color(theme.pop).into_any_element();
        let named = |name: SharedString| pill_label(name).into_any_element();
        // A worktree's files landing where they move it name the worktree.
        let (mark, label) = match (&transfer, &self.chip, &self.payload) {
            (Some(Transfer::MoveWorktree { name, .. }), ..) => {
                (glyph(Icon::FolderCopy), named(name.to_string().into()))
            }
            (_, Chip::Folder(name), payload) => (
                folder_icon().into_any_element(),
                h_flex()
                    .min_w_0()
                    .gap(dp(6.))
                    .child(pill_label(name.clone()))
                    .children(match payload {
                        Cut::Files { files, .. } if files.len() > 1 => {
                            Some(count_badge(files.len(), theme))
                        }
                        _ => None,
                    })
                    .into_any_element(),
            ),
            (_, Chip::Row(label), Cut::Commit(..) | Cut::CherryPick(..)) => {
                (glyph(Icon::Commit), named(label.clone()))
            }
            (_, Chip::Row(label), Cut::Branch(..)) => (glyph(Icon::Branch), named(label.clone())),
            (_, Chip::Row(label), Cut::Files { files, .. }) => match &**files {
                // All of a file goes by its name, as its folder says little while dragging.
                [file] => (
                    file_icon(&file.file.path, cx).into_any_element(),
                    named(match file.hunks {
                        None => split_path(&file.file.path).0.to_owned().into(),
                        Some(_) => label.clone(),
                    }),
                ),
                _ => (glyph(Icon::File), named(label.clone())),
            },
            (_, Chip::Row(label), Cut::Worktree(..)) => {
                (glyph(Icon::FolderCopy), named(label.clone()))
            }
        };
        let carried = match &self.payload {
            Cut::Files { files, .. } => files.len(),
            _ => 1,
        };
        drag_pill(self.offset, mark, label, hint, carried, theme)
    }
}

/// A pill's label, cut short where it won't fit.
pub fn pill_label(label: SharedString) -> Div {
    div().truncate().child(label)
}

/// What a dragged row shows under the pointer: a pill with its `mark`, `label` and what
/// letting go does there (`hint`), on a stack of cards when it's `carried` more than one
/// thing, below and right of the pointer as a tooltip sits, clear of the row it's over and the
/// line or outline that row shows. `offset` is where the pointer pressed the row, which GPUI
/// draws from.
pub fn drag_pill(
    offset: Point<Pixels>,
    mark: impl IntoElement,
    label: impl IntoElement,
    hint: Option<&'static str>,
    carried: usize,
    theme: &Theme,
) -> Div {
    let cards = ([6., 12.].into_iter())
        .take(carried.saturating_sub(1))
        .rev()
        .map(|inset| {
            let card = popover(theme).absolute().size_full();
            card.top(dp(inset)).left(dp(inset))
        });
    let pill = popover(theme)
        .flex_row()
        .items_center()
        .gap(dp(8.))
        .px(dp(10.))
        .h(dp(28.))
        .max_w(dp(320.))
        .border_color(theme.pop)
        .font_family(theme.ui_font.clone())
        .text_size(dp(12.))
        .text_color(theme.text_1)
        .child(mark)
        .child(label)
        .when_some(hint, |el, hint| {
            el.child(div().flex_none().text_color(theme.text_2).child(hint))
        });
    div()
        .pl(offset.x + theme.scaled(14.))
        .pt(offset.y + theme.scaled(16.))
        .child(div().relative().children(cards).child(pill))
}

/// Whether commits `ids`, as listed, are already where moving them `to` puts them: that run
/// of one branch, in that order, at its top, or right below or above the commit. Picked
/// commits apart from each other move together wherever they are.
fn already_at(workspace: &Workspace, ids: &[CommitId], to: &Destination) -> bool {
    // The branch holding commit `id`, and its commits from `id` down.
    let down_from = |id: CommitId| {
        let (branch, _) = workspace.find_commit(id)?;
        let down = (branch.commits.iter())
            .map(|c| c.id)
            .skip_while(move |c| *c != id);
        Some((branch, down))
    };
    let first = ids.first().copied();
    let Some((branch, mut down)) = first.and_then(down_from) else {
        return false;
    };
    if down.by_ref().take(ids.len()).ne(ids.iter().copied()) {
        return false;
    }
    match to {
        Destination::Branch(name) => {
            branch.ref_name.as_ref() == Some(name) && branch.commits.first().map(|c| c.id) == first
        }
        Destination::BelowCommit(target) => {
            down_from(*target).and_then(|(_, mut down)| down.nth(1)) == first
        }
        Destination::AboveCommit(target) => down.next() == Some(*target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FileChange;

    #[test]
    fn a_paste_on_a_commit_puts_files_into_it_and_commits_above_it() {
        let (project, id) = (ProjectId::from("p"), CommitId(gix::ObjectId::from([1; 20])));
        let commit = Selection::Commit(project.clone(), id);
        let files = Cut::Files {
            project: project.clone(),
            from: None,
            worktree: Checkout::Main,
            files: Arc::from([]),
        };
        let held = Modifiers::default();
        assert_eq!(
            Place::from_keys(held, &files, &commit),
            Place::Into,
            "files amend the commit, as only commits go above or below one"
        );
        assert_eq!(
            Place::from_keys(held, &Cut::Commit(project, Arc::from([id])), &commit),
            Place::Above,
            "a commit goes above it"
        );
    }

    #[test]
    fn what_cant_land_is_left_out_of_a_cut() {
        let cut = |paths: &[(&str, bool)]| Cut::Files {
            project: ProjectId::from("p"),
            from: None,
            worktree: Checkout::Main,
            files: (paths.iter())
                .map(|(path, conflicted)| {
                    Selected::whole(&FileChange {
                        conflicted: *conflicted,
                        ..FileChange::bare(path)
                    })
                })
                .collect(),
        };
        let Some(Cut::Files { files, .. }) = cut(&[("a", false), ("b", true)]).movable() else {
            panic!("the clean file can land");
        };
        let kept: Vec<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
        assert_eq!(
            kept,
            ["a"],
            "a commit would refuse them all for the conflicted one"
        );
        assert!(
            cut(&[("b", true)]).movable().is_none(),
            "with only conflicted files there's nothing to carry"
        );
    }

    #[test]
    fn commits_are_already_where_they_go_only_as_that_run() {
        use crate::backend::model::build::{branch, full, id, workspace};
        // ui's commits newest first: 5, 4, 3, 2; then api's, 1.
        let ws = workspace(
            vec![vec![branch("ui", &[5, 4, 3, 2]), branch("api", &[1])]],
            vec![],
        );
        let ui = || Destination::Branch(full("refs/heads/ui"));
        let at = |ids: &[u8], to: &Destination| {
            let ids: Vec<CommitId> = ids.iter().map(|n| id(*n)).collect();
            already_at(&ws, &ids, to)
        };
        assert!(at(&[5, 4], &ui()), "the top two are at the top of ui");
        assert!(!at(&[4, 5], &ui()), "in another order they'd move");
        assert!(!at(&[5, 3], &ui()), "picked apart, they'd move together");
        assert!(
            at(&[4, 3], &Destination::BelowCommit(id(5))),
            "right below 5"
        );
        assert!(
            !at(&[3, 2], &Destination::BelowCommit(id(5))),
            "4 is between them"
        );
        assert!(
            at(&[4, 3], &Destination::AboveCommit(id(2))),
            "right above 2"
        );
        assert!(
            !at(&[2], &Destination::AboveCommit(id(1))),
            "1 is the next branch's, so 2 isn't right above it in a branch"
        );
    }

    #[test]
    fn a_card_moved_with_the_keys_shows_the_others_as_they_were_listed() {
        let [a, b, c] = ["a", "b", "c"].map(ProjectId::from);
        let order: Arc<[ProjectId]> = Arc::from([a.clone(), b.clone(), c.clone()]);
        let at = |slot| Move {
            project: b.clone(),
            what: Moving::Project(order.clone()),
            at: Stop::Card(slot),
        };
        assert_eq!(
            at(CardSlot::Before(a.clone())).project_order(),
            Some(vec![b.clone(), a.clone(), c.clone()]),
            "before a, it's first"
        );
        assert_eq!(
            at(CardSlot::End).project_order(),
            Some(vec![a, c.clone(), b.clone()]),
            "at the end, it's last"
        );
        assert_eq!(
            at(CardSlot::Before(c)).project_order().as_deref(),
            Some(&order[..]),
            "before c is where it was"
        );
    }

    #[test]
    fn a_slot_is_before_each_of_the_others_or_last_and_starts_where_it_is() {
        let order = ["a", "b", "c"];
        let (b, start) = slots(&order, &"b").unwrap();
        assert_eq!(
            b,
            [Slot::Before("a"), Slot::Before("c"), Slot::End],
            "b goes before a, before c or last"
        );
        assert_eq!(start, 1, "b is before c");
        let (_, start) = slots(&order, &"c").unwrap();
        assert_eq!(start, 2, "the last is where the end is");
    }
}
