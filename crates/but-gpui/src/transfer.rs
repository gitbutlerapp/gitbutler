//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{
    App, Context, Div, Entity, Modifiers, Pixels, Point, SharedString, Window, div, prelude::*,
};

use crate::backend::{
    self, Checkout, CommitId, CommitTo, Destination, FullName, Selected, short_ref,
};
use crate::state::{
    Cut, Label, NewAt, Overlay, ProjectId, Selection, Store, branch_moved, move_before,
};
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
    /// Stack a branch, and those above it, on another.
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

/// Commits being moved with the keys: the navigator steps through the places they can land,
/// and Return lands them where it's at.
#[derive(Debug, Clone)]
pub struct Move {
    pub project: ProjectId,
    /// As listed, newest first.
    pub commits: Arc<[CommitId]>,
    pub at: Stop,
}

/// A place commits being moved can land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// In a branch: at its top, or right below one of its commits.
    In(Destination),
    /// In a new branch right above `beside` in its stack, or right below it with `below`.
    NewBranch { beside: FullName, below: bool },
    /// In a new branch in a stack of its own, after the others, or before them with `first`.
    NewStack { first: bool },
}

impl Move {
    /// Landing in a branch as a drop: the commits as what's dropped, and the row and the place
    /// on it they're dropped at. `None` in a new branch, which has no row yet.
    pub fn drop(&self) -> Option<(Cut, Selection, Place)> {
        let Stop::In(to) = &self.at else {
            return None;
        };
        let project = self.project.clone();
        let (target, place) = match to {
            Destination::Branch(branch) => (
                Selection::Branch(project.clone(), Some(branch.clone())),
                Place::Into,
            ),
            Destination::AboveCommit(id) => (Selection::Commit(project.clone(), *id), Place::Above),
            Destination::BelowCommit(id) => (Selection::Commit(project.clone(), *id), Place::Below),
        };
        Some((Cut::Commit(project, self.commits.clone()), target, place))
    }

    /// Whether landing moves the commits, as it doesn't where they already are.
    pub fn lands(&self, store: &Store) -> bool {
        self.drop().is_none_or(|(payload, target, place)| {
            Transfer::resolve(&payload, &target, place, store).is_some()
        })
    }

    /// Where landing puts them, said after a verb: "to a new branch below feature-x".
    pub fn place(&self) -> String {
        match &self.at {
            Stop::In(Destination::Branch(branch)) => {
                format!("to the top of {}", short_ref(branch))
            }
            Stop::In(to) => place(to),
            Stop::NewBranch { beside, below } => {
                let side = if *below { "below" } else { "above" };
                format!("to a new branch {side} {}", short_ref(beside))
            }
            Stop::NewStack { first: false } => "to a new stack".into(),
            Stop::NewStack { first: true } => "to a new stack at the top".into(),
        }
    }
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
                // Already the top of that branch.
                if let [id] = ids[..] {
                    let (current, _) = workspace.find_commit(id)?;
                    let top = current.commits.first().map(|c| c.id);
                    if current.ref_name.as_ref() == Some(branch) && top == Some(id) {
                        return None;
                    }
                }
                Transfer::MoveCommit {
                    ids: ids.clone(),
                    to: Destination::Branch(branch.clone()),
                }
            }
            (Cut::Commit(_, ids), Selection::Commit(_, target_id)) if !ids.contains(target_id) => {
                // Already right above it, or right below, as one commit can be; several move
                // together wherever they are.
                let next = |after: CommitId| {
                    let (branch, _) = workspace.find_commit(after)?;
                    let mut commits = branch.commits.iter().skip_while(|c| c.id != after);
                    commits.nth(1).map(|c| c.id)
                };
                let one = match ids[..] {
                    [id] => Some(id),
                    _ => None,
                };
                let (already, to) = match place {
                    // Not with a conflicted commit on either side, whose change the squash
                    // would drop, or the engine refuses.
                    Place::Into => {
                        let clean = !(ids.iter().chain([target_id])).any(|id| conflicted(*id));
                        return clean.then(|| Transfer::Squash {
                            ids: ids.clone(),
                            into: *target_id,
                        });
                    }
                    Place::Below => (
                        one.is_some_and(|id| next(*target_id) == Some(id)),
                        Destination::BelowCommit(*target_id),
                    ),
                    Place::Above => (
                        one.is_some_and(|id| next(id) == Some(*target_id)),
                        Destination::AboveCommit(*target_id),
                    ),
                };
                if already {
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
    pub fn perform(
        self,
        project: &SharedString,
        store: &mut Store,
        cx: &mut Context<Store>,
    ) -> Option<(FullName, Arc<[Selected]>)> {
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
                Overlay::default(),
                backend::StackBranch { branch, onto },
                move |store, conflicted, cx| {
                    store.snack_undoable(branch_moved("Branch moved", &conflicted), cx)
                },
                cx,
            ),
            Transfer::BranchBelow { branch, commit } => store.apply(
                project,
                label,
                Overlay::default(),
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
}
