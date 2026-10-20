//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{
    App, Context, Div, Entity, Modifiers, Pixels, Point, SharedString, Window, div, prelude::*, px,
};

use crate::backend::{self, CommitId, Destination, Selected, short_ref};
use crate::state::{Cut, NewAt, Overlay, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::{Icon, popover, small_icon};

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
    /// Uncommitted files of the main checkout, or of the linked `worktree`, into a commit.
    Amend {
        into: CommitId,
        files: Arc<[Selected]>,
        worktree: Option<Arc<str>>,
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
        branch: String,
        onto: String,
    },
    /// Put a branch under another, right below that one's last commit.
    BranchBelow {
        branch: String,
        commit: CommitId,
    },
    /// Uncommitted files dropped on a branch: get them ready to commit there.
    StageOnBranch {
        branch: String,
        files: Arc<[Selected]>,
    },
    /// Uncommitted files the composer can't take, a linked worktree's or bound for one,
    /// committed on a branch with no message yet, as Lite does.
    CommitOnBranch {
        branch: String,
        files: Arc<[Selected]>,
        worktree: Option<Arc<str>>,
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

impl Transfer {
    /// What `payload` does when it lands on `target` at `place`, if anything. A commit landing
    /// on another goes above or below it, or squashes into it. A branch landing on another
    /// stacks on it, or goes below it.
    pub fn resolve(payload: &Cut, target: &Selection, place: Place, store: &Store) -> Option<Self> {
        if payload.project() != target.project() {
            return None;
        }
        // An unnamed branch has no ref to put things on or stack onto.
        if matches!(target, Selection::Branch(_, branch) if branch.is_empty()) {
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
            let side = |s: &Selection| store.worktree_of(s).map(|wt| wt.name.clone());
            let from = side(&Selection::Branch(project.clone(), branch.clone()));
            let stays = match target {
                Selection::Worktree(..) => from.is_none(),
                _ => from == side(target),
            };
            if !stays {
                return None;
            }
        }
        let workspace = store.workspace(target.project())?;
        let conflicted =
            |id: CommitId| workspace.find_commit(id).is_some_and(|(_, c)| c.conflicted);
        Some(match (payload, target) {
            (Cut::Commit(project, ids), Selection::Uncommitted(_) | Selection::Worktree(..))
                if ids
                    .iter()
                    .all(|id| uncommits_into(store, project, *id, target)) =>
            {
                Transfer::Uncommit(ids.clone())
            }
            (Cut::Commit(_, ids), Selection::Branch(_, branch)) => {
                // Already the top of that branch.
                if let [id] = ids[..] {
                    let (current, _) = workspace.find_commit(id)?;
                    let top = current.commits.first().map(|c| c.id);
                    if current.ref_name == *branch && top == Some(id) {
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
                    Place::Into => {
                        return Some(Transfer::Squash {
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
            (Cut::CherryPick(_, ids), Selection::Branch(_, branch)) => Transfer::CherryPick {
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
            (Cut::Branch(_, branch), Selection::Branch(_, above))
                if place == Place::Below && branch != above =>
            {
                let (lane, at) = workspace.lane_of(above)?;
                // Already right below it.
                if lane.get(at + 1).is_some_and(|b| b.ref_name == *branch) {
                    return None;
                }
                Transfer::BranchBelow {
                    branch: branch.clone(),
                    commit: lane[at].commits.last()?.id,
                }
            }
            (Cut::Branch(_, branch), Selection::Worktree(_, name)) => Transfer::BranchBelow {
                branch: branch.clone(),
                commit: workspace.find_worktree(name)?.lane_end()?,
            },
            (Cut::Branch(_, branch), Selection::Branch(_, onto)) if branch != onto => {
                // Already right on top of it.
                let (lane, at) = workspace.lane_of(branch)?;
                if lane.get(at + 1).is_some_and(|b| b.ref_name == *onto) {
                    return None;
                }
                Transfer::StackBranch {
                    branch: branch.clone(),
                    onto: onto.clone(),
                }
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
                (None, Selection::Branch(_, branch))
                    if worktree.is_none() && store.worktree_of(target).is_none() =>
                {
                    Transfer::StageOnBranch {
                        branch: branch.clone(),
                        files: files.clone(),
                    }
                }
                (None, Selection::Branch(_, branch)) => Transfer::CommitOnBranch {
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
        }
    }

    /// Carry it out. Staging only changes the composer, so it comes back to the caller.
    pub fn perform(
        self,
        project: &SharedString,
        store: &mut Store,
        cx: &mut Context<Store>,
    ) -> Option<(String, Arc<[Selected]>)> {
        let label = self.describe();
        // Picked commits that moved, or went into one, are no longer what's picked.
        if let Transfer::Uncommit(ids)
        | Transfer::MoveCommit { ids, .. }
        | Transfer::Squash { ids, .. } = &self
            && ids.len() > 1
        {
            store.picked.clear();
        }
        // The oplog snapshots only the main checkout, so undoing what moves a linked
        // worktree's changes, or rewrites the branches it has checked out, would lose them.
        let in_worktree = |id| store.in_worktree(project, id);
        let branch_in_worktree = |branch: &str| {
            (store.worktree_of(&Selection::Branch(project.clone(), branch.to_owned()))).is_some()
        };
        let lands_in_worktree = |to: &Destination| match to {
            Destination::Branch(branch) => branch_in_worktree(branch),
            Destination::AboveCommit(id) | Destination::BelowCommit(id) => in_worktree(*id),
        };
        let undoable = match &self {
            Transfer::UncommitFiles { from, .. } => !in_worktree(*from),
            Transfer::MoveFiles { from, to, .. } => !in_worktree(*from) && !in_worktree(*to),
            Transfer::MoveCommit { ids, to } => {
                !ids.iter().any(|id| in_worktree(*id)) && !lands_in_worktree(to)
            }
            Transfer::CherryPick { to, .. } => !lands_in_worktree(to),
            Transfer::Squash { ids, into } => {
                !ids.iter().any(|id| in_worktree(*id)) && !in_worktree(*into)
            }
            Transfer::StackBranch { branch, onto } => {
                !branch_in_worktree(branch) && !branch_in_worktree(onto)
            }
            Transfer::BranchBelow { branch, commit } => {
                !branch_in_worktree(branch) && !in_worktree(*commit)
            }
            Transfer::CommitOnBranch { .. } => false,
            // Uncommitting and amending say for themselves.
            Transfer::Uncommit(_) | Transfer::Amend { .. } | Transfer::StageOnBranch { .. } => true,
        };
        match self {
            Transfer::Uncommit(ids) => store.uncommit(project, ids.to_vec(), cx),
            Transfer::UncommitFiles { from, files } => store.run(
                project,
                label,
                Overlay::commit_files(store.key_of(project, from), files.iter()),
                move |path, remap| {
                    backend::uncommit_files(path, remap.taken_from(from, &files)?, &files)
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Moved to uncommitted changes", cx)
                },
                cx,
            ),
            // It leaves where it was and shows dimmed where it goes.
            Transfer::MoveCommit { ids, to } => store.run(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, &ids, &to),
                    ..Overlay::commits(keys(store, project, &ids))
                },
                move |path, remap| {
                    backend::move_commits(path, &remap.all(&ids)?, &remap.destination(to)?)
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Moved", cx)
                },
                cx,
            ),
            Transfer::CherryPick { ids, to } => store.run(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, &ids, &to),
                    ..Overlay::default()
                },
                move |path, remap| {
                    // A commit from outside the workspace can't have been rewritten.
                    let ids: Vec<_> = (ids.iter())
                        .map(|id| remap.get(*id).unwrap_or(*id))
                        .collect();
                    backend::cherry_pick(path, &ids, &remap.destination(to)?)
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Cherry-picked", cx)
                },
                cx,
            ),
            Transfer::StackBranch { branch, onto } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, _| backend::stack_branch(path, &branch, &onto),
                move |store, (), cx| store.snack_after(undoable, "Branch moved", cx),
                cx,
            ),
            Transfer::BranchBelow { branch, commit } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    // The remap knows the workspace's commits, not a linked worktree's.
                    let commit = remap.get(commit).unwrap_or(commit);
                    backend::move_branch_below(path, &branch, commit)
                },
                move |store, (), cx| store.snack_after(undoable, "Branch moved", cx),
                cx,
            ),
            Transfer::Squash { ids, into } => store.run(
                project,
                label,
                Overlay::commits(keys(store, project, &ids)),
                move |path, remap| {
                    // Oldest first, so their messages follow `into`'s in the order they were
                    // written.
                    let ids: Vec<CommitId> = ids.iter().rev().copied().collect();
                    backend::squash(path, &remap.all(&ids)?, remap.get(into)?)
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Squashed", cx)
                },
                cx,
            ),
            Transfer::Amend {
                into,
                files,
                worktree,
            } => store.amend(project, into, files, worktree, |_, _| {}, cx),
            Transfer::MoveFiles { from, to, files } => store.run(
                project,
                label,
                Overlay::commit_files(store.key_of(project, from), files.iter()),
                move |path, remap| {
                    backend::move_files(
                        path,
                        remap.taken_from(from, &files)?,
                        remap.get(to)?,
                        &files,
                    )
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Changes moved", cx)
                },
                cx,
            ),
            Transfer::StageOnBranch { branch, files } => return Some((branch, files)),
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
                store.run(
                    project,
                    label,
                    overlay,
                    move |path, _| {
                        let to = Destination::Branch(branch);
                        backend::commit(path, &to, &files, worktree.as_deref(), "", false)
                    },
                    move |store, (id, left_out, _), cx| {
                        store.select(Some(Selection::Commit(selected, id)), cx);
                        store.keep_last_snapshot();
                        let done = "Committed. Double-click its title to write a message";
                        store.report_left_out(done, &left_out, cx)
                    },
                    cx,
                )
            }
        }
        None
    }
}

/// The keys an overlay hides commits `ids` of `project` by.
fn keys(store: &Store, project: &ProjectId, ids: &[CommitId]) -> Vec<String> {
    (ids.iter())
        .filter_map(|id| store.key_of(project, *id))
        .collect()
}

/// Whether `target` is the checkout commit `id`'s changes uncommit into: a linked worktree's
/// commit's go into that worktree, as that's where their checkout is, and the rest into the
/// main one.
fn uncommits_into(store: &Store, project: &ProjectId, id: CommitId, target: &Selection) -> bool {
    let home = store.worktree_of(&Selection::Commit(project.clone(), id));
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

/// Start a drag of a `Cut` labelled `label`, letting the store know what's in flight so
/// the rows can say what a drop would do.
pub fn drag_preview(
    store: Entity<Store>,
    label: SharedString,
) -> impl Fn(&Cut, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static {
    move |payload, offset, _, cx| {
        store.update(cx, |store, cx| {
            store.dragging = Some(payload.clone());
            cx.notify()
        });
        let preview = cx.new(|_| DragPreview {
            label: label.clone(),
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
pub struct DragPreview {
    pub label: SharedString,
    pub payload: Cut,
    store: Entity<Store>,
    /// Where the pointer pressed the dragged row, which GPUI draws the pill from.
    offset: Point<Pixels>,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let hint = (store.drop_at.as_ref()).and_then(|(target, place)| {
            Transfer::resolve(&self.payload, target, *place, store).map(|t| t.verb())
        });
        let glyph = match self.payload {
            Cut::Commit(..) | Cut::CherryPick(..) => Icon::Commit,
            Cut::Branch(..) => Icon::Branch,
            Cut::Files { .. } => Icon::File,
        };
        drag_pill(self.offset, glyph, self.label.clone(), hint, theme)
    }
}

/// What a dragged row shows under the pointer: a pill with its glyph, `label` and what
/// letting go does there (`hint`), below and right of the pointer as a tooltip sits, clear of
/// the row it's over and the line or outline that row shows. `offset` is where the pointer
/// pressed the row, which GPUI draws from.
pub fn drag_pill(
    offset: Point<Pixels>,
    glyph: Icon,
    label: SharedString,
    hint: Option<&'static str>,
    theme: &Theme,
) -> Div {
    let pill = popover(theme)
        .flex_row()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .h(px(28.))
        .max_w(px(320.))
        .border_color(theme.pop)
        .font_family(theme.ui_font.clone())
        .text_size(px(12.))
        .text_color(theme.text_1)
        .child(small_icon(glyph).text_color(theme.pop))
        .child(div().truncate().child(label))
        .when_some(hint, |el, hint| {
            el.child(div().flex_none().text_color(theme.text_2).child(hint))
        });
    div()
        .pl(offset.x + px(14.))
        .pt(offset.y + px(16.))
        .child(pill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paste_on_a_commit_puts_files_into_it_and_commits_above_it() {
        let (project, id) = (ProjectId::from("p"), CommitId(gix::ObjectId::from([1; 20])));
        let commit = Selection::Commit(project.clone(), id);
        let files = Cut::Files {
            project: project.clone(),
            from: None,
            worktree: None,
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
}
