//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{
    App, Context, Entity, Modifiers, Pixels, Point, SharedString, Window, div, prelude::*, px,
};

use crate::backend::{self, CommitId, Destination, Selected, short_ref};
use crate::state::{Cut, NewAt, Overlay, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::{Icon, popover, small_icon};

pub enum Transfer {
    Uncommit(CommitId),
    UncommitFiles {
        from: CommitId,
        files: Arc<[Selected]>,
    },
    MoveCommit {
        id: CommitId,
        to: Destination,
    },
    Squash {
        id: CommitId,
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
    /// A copy of the commit, leaving it where it is.
    CherryPick {
        id: CommitId,
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

impl Transfer {
    /// What `payload` does when it lands on `target`, if anything. A commit landing on
    /// another goes above it; with ⌥ held it squashes into it instead, and with ⇧ it goes
    /// below it. A branch landing on another stacks on it, and with ⇧ goes below it.
    pub fn resolve(
        payload: &Cut,
        target: &Selection,
        held: Modifiers,
        store: &Store,
    ) -> Option<Self> {
        if payload.project() != target.project() {
            return None;
        }
        // An unnamed branch has no ref to put things on or stack onto.
        if matches!(target, Selection::Branch(_, branch) if branch.is_empty()) {
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
        Some(match (payload, target) {
            (Cut::Commit(project, id), Selection::Uncommitted(_) | Selection::Worktree(..))
                if uncommits_into(store, project, *id, target) =>
            {
                Transfer::Uncommit(*id)
            }
            (Cut::Commit(_, id), Selection::Branch(_, branch)) => {
                let (current, _) = workspace.find_commit(*id)?;
                // Already the top of that branch.
                if current.ref_name == *branch && current.commits.first().map(|c| c.id) == Some(*id)
                {
                    return None;
                }
                Transfer::MoveCommit {
                    id: *id,
                    to: Destination::Branch(branch.clone()),
                }
            }
            (Cut::Commit(_, id), Selection::Commit(_, target_id)) if id != target_id => {
                if held.alt {
                    return Some(Transfer::Squash {
                        id: *id,
                        into: *target_id,
                    });
                }
                // Already right above it, or right below.
                let next = |after: CommitId| {
                    let (branch, _) = workspace.find_commit(after)?;
                    let mut commits = branch.commits.iter().skip_while(|c| c.id != after);
                    commits.nth(1).map(|c| c.id)
                };
                let (already, to) = if held.shift {
                    (
                        next(*target_id) == Some(*id),
                        Destination::BelowCommit(*target_id),
                    )
                } else {
                    (
                        next(*id) == Some(*target_id),
                        Destination::AboveCommit(*target_id),
                    )
                };
                if already {
                    return None;
                }
                Transfer::MoveCommit { id: *id, to }
            }
            (Cut::CherryPick(_, id), Selection::Branch(_, branch)) => Transfer::CherryPick {
                id: *id,
                to: Destination::Branch(branch.clone()),
            },
            (Cut::CherryPick(_, id), Selection::Commit(_, target_id)) if id != target_id => {
                Transfer::CherryPick {
                    id: *id,
                    to: if held.shift {
                        Destination::BelowCommit(*target_id)
                    } else {
                        Destination::AboveCommit(*target_id)
                    },
                }
            }
            (Cut::Branch(_, branch), Selection::Branch(_, above))
                if held.shift && branch != above =>
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
                (None, Selection::Commit(_, into)) => Transfer::Amend {
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
                (Some(from), Selection::Commit(_, to)) if from != *to => Transfer::MoveFiles {
                    from,
                    to: *to,
                    files: files.clone(),
                },
                _ => return None,
            },
            _ => return None,
        })
    }

    /// What a drop does, said from the target row's point of view.
    pub fn verb(&self) -> &'static str {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit",
            Transfer::MoveCommit {
                to: Destination::Branch(_),
                ..
            } => "Move to top",
            Transfer::MoveCommit {
                to: Destination::AboveCommit(_),
                ..
            } => "Move above",
            Transfer::MoveCommit {
                to: Destination::BelowCommit(_),
                ..
            } => "Move below",
            Transfer::CherryPick {
                to: Destination::Branch(_),
                ..
            } => "Cherry-pick to top",
            Transfer::CherryPick {
                to: Destination::BelowCommit(_),
                ..
            } => "Cherry-pick below",
            Transfer::CherryPick { .. } => "Cherry-pick above",
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
            Transfer::MoveCommit { id, to } => !in_worktree(*id) && !lands_in_worktree(to),
            Transfer::CherryPick { to, .. } => !lands_in_worktree(to),
            Transfer::Squash { id, into } => !in_worktree(*id) && !in_worktree(*into),
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
            Transfer::Uncommit(id) => store.uncommit(project, vec![id], cx),
            Transfer::UncommitFiles { from, files } => store.run(
                project,
                label,
                Overlay::commit_files(store.key_of(project, from), files.iter()),
                move |path, remap| backend::uncommit_files(path, remap.get(from)?, &files),
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Moved to uncommitted changes", cx)
                },
                cx,
            ),
            // It leaves where it was and shows dimmed where it goes.
            Transfer::MoveCommit { id, to } => store.run(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, id, &to),
                    ..Overlay::commits(store.key_of(project, id))
                },
                move |path, remap| {
                    backend::move_commits(path, &[remap.get(id)?], &remap.destination(to)?)
                },
                move |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_after(undoable, "Commit moved", cx)
                },
                cx,
            ),
            Transfer::CherryPick { id, to } => store.run(
                project,
                label,
                Overlay {
                    new_commit: store.arriving(project, id, &to),
                    ..Overlay::default()
                },
                move |path, remap| {
                    // A commit from outside the workspace can't have been rewritten.
                    let id = remap.get(id).unwrap_or(id);
                    backend::cherry_pick(path, id, &remap.destination(to)?)
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
            Transfer::Squash { id, into } => store.run(
                project,
                label,
                Overlay::commits(store.key_of(project, id)),
                move |path, remap| backend::squash(path, &[remap.get(id)?], remap.get(into)?),
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
                    backend::move_files(path, remap.get(from)?, remap.get(to)?, &files)
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
    move |payload, _, _, cx| {
        store.update(cx, |store, cx| {
            store.dragging = Some(payload.clone());
            cx.notify()
        });
        let preview = cx.new(|_| DragPreview {
            label: label.clone(),
            payload: payload.clone(),
        });
        // GPUI drops the preview when the drag ends, dropped or not. Let go over a row that
        // won't take it, nothing else redraws the navigator, which would keep showing the
        // drag until the next change.
        let store = store.clone();
        cx.observe_release(&preview, move |_, cx| {
            store.update(cx, |store, cx| {
                store.dragging = None;
                cx.notify()
            });
        })
        .detach();
        preview
    }
}

/// The pill that follows the pointer while dragging, and what it carries.
pub struct DragPreview {
    pub label: SharedString,
    pub payload: Cut,
}

impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let held = window.modifiers();
        let hint = match &self.payload {
            Cut::Commit(..) if held.alt => Some("Squash"),
            Cut::Commit(..) | Cut::CherryPick(..) if held.shift => Some("Below"),
            Cut::Commit(..) => Some("⌥ squash · ⇧ below"),
            Cut::CherryPick(..) => Some("⇧ below"),
            Cut::Branch(..) | Cut::Files { .. } => None,
        };
        popover(theme)
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
            .child(
                small_icon(match self.payload {
                    Cut::Commit(..) | Cut::CherryPick(..) => Icon::Commit,
                    Cut::Branch(..) => Icon::Branch,
                    Cut::Files { .. } => Icon::File,
                })
                .text_color(theme.pop),
            )
            .child(div().truncate().child(self.label.clone()))
            .when_some(hint, |el, hint| {
                el.child(div().flex_none().text_color(theme.text_2).child(hint))
            })
    }
}
