//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{Context, SharedString};

use crate::backend::{self, CommitId, Destination, Selected};
use crate::state::{Cut, Overlay, Remap, Selection, Store};

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
    Amend {
        into: CommitId,
        files: Arc<[Selected]>,
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
    /// Uncommitted files dropped on a branch: get them ready to commit there.
    StageOnBranch {
        branch: String,
        files: Arc<[Selected]>,
    },
}

impl Transfer {
    /// What `payload` does when it lands on `target`, if anything. `alternate` (⌥ held)
    /// turns moving a commit onto another commit into squashing it.
    pub fn resolve(
        payload: &Cut,
        target: &Selection,
        alternate: bool,
        store: &Store,
    ) -> Option<Self> {
        if payload.project() != target.project() {
            return None;
        }
        let workspace = store.workspace(target.project())?;
        Some(match (payload, target) {
            (Cut::Commit(_, id), Selection::Uncommitted(_)) => Transfer::Uncommit(*id),
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
                if alternate {
                    Transfer::Squash {
                        id: *id,
                        into: *target_id,
                    }
                } else {
                    // Already right above it.
                    let (branch, _) = workspace.find_commit(*id)?;
                    let below = branch.commits.iter().skip_while(|c| c.id != *id).nth(1);
                    if below.is_some_and(|commit| commit.id == *target_id) {
                        return None;
                    }
                    Transfer::MoveCommit {
                        id: *id,
                        to: Destination::AboveCommit(*target_id),
                    }
                }
            }
            (Cut::Pick(_, id), Selection::Branch(_, branch)) => Transfer::CherryPick {
                id: *id,
                to: Destination::Branch(branch.clone()),
            },
            (Cut::Pick(_, id), Selection::Commit(_, target_id)) if id != target_id => {
                Transfer::CherryPick {
                    id: *id,
                    to: Destination::AboveCommit(*target_id),
                }
            }
            (Cut::Branch(_, branch), Selection::Branch(_, onto)) if branch != onto => {
                // Already right on top of it.
                let (stack, at) = workspace.stack_of(branch)?;
                if stack
                    .branches
                    .get(at + 1)
                    .is_some_and(|b| b.ref_name == *onto)
                {
                    return None;
                }
                Transfer::StackBranch {
                    branch: branch.clone(),
                    onto: onto.clone(),
                }
            }
            (Cut::Files { from, files, .. }, target) => match (*from, target) {
                (None, Selection::Commit(_, into)) => Transfer::Amend {
                    into: *into,
                    files: files.clone(),
                },
                (None, Selection::Branch(_, branch)) => Transfer::StageOnBranch {
                    branch: branch.clone(),
                    files: files.clone(),
                },
                (Some(from), Selection::Uncommitted(_)) => Transfer::UncommitFiles {
                    from,
                    files: files.clone(),
                },
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
                to: Destination::AboveCommit(_) | Destination::AboveBranch(_),
                ..
            } => "Move above",
            Transfer::MoveCommit {
                to: Destination::BelowCommit(_),
                ..
            } => "Move below",
            Transfer::CherryPick {
                to: Destination::Branch(_),
                ..
            } => "Copy to top",
            Transfer::CherryPick { .. } => "Copy above",
            Transfer::StackBranch { .. } => "Stack on top",
            Transfer::Squash { .. } => "Squash into",
            Transfer::Amend { .. } => "Amend",
            Transfer::MoveFiles { .. } => "Move into",
            Transfer::StageOnBranch { .. } => "Commit here",
        }
    }

    /// A short verb phrase for hints, like "Squash into a1b2c3d".
    pub fn describe(&self) -> SharedString {
        match self {
            Transfer::Uncommit(_) | Transfer::UncommitFiles { .. } => "Uncommit".into(),
            Transfer::MoveCommit { to, .. } => format!("Move {}", place(to)).into(),
            Transfer::CherryPick { to, .. } => format!("Copy {}", place(to)).into(),
            Transfer::StackBranch { onto, .. } => format!("Stack on {}", short_ref(onto)).into(),
            Transfer::Squash { into, .. } => format!("Squash into {}", into.short()).into(),
            Transfer::Amend { into, .. } => format!("Amend {}", into.short()).into(),
            Transfer::MoveFiles { to, .. } => format!("Move into {}", to.short()).into(),
            Transfer::StageOnBranch { branch, .. } => {
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
        match self {
            Transfer::Uncommit(id) => store.uncommit(project, vec![id], cx),
            Transfer::UncommitFiles { from, files } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| backend::uncommit_files(path, remap.get(from)?, &files),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Moved to uncommitted changes", cx)
                },
                cx,
            ),
            Transfer::MoveCommit { id, to } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    backend::move_commits(path, &[remap.get(id)?], &remapped(to, remap)?)
                },
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::CherryPick { id, to } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    // A commit from outside the workspace can't have been rewritten.
                    let id = remap.get(id).unwrap_or(id);
                    backend::cherry_pick(path, id, &remapped(to, remap)?)
                },
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Copied", cx)
                },
                cx,
            ),
            Transfer::StackBranch { branch, onto } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, _| backend::stack_branch(path, &branch, &onto),
                |store, (), cx| store.snack_undoable("Branch moved", cx),
                cx,
            ),
            Transfer::Squash { id, into } => store.run(
                project,
                label,
                Overlay::commits(store.key_of(project, id)),
                move |path, remap| backend::squash(path, &[remap.get(id)?], remap.get(into)?),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Squashed", cx)
                },
                cx,
            ),
            Transfer::Amend { into, files } => store.run(
                project,
                label,
                Overlay::files(files.iter()),
                move |path, remap| backend::amend(path, remap.get(into)?, &files),
                |store, (rewritten, left_out), cx| {
                    store.follow(&rewritten, cx);
                    store.report_left_out("Amended", &left_out, cx)
                },
                cx,
            ),
            Transfer::MoveFiles { from, to, files } => store.run(
                project,
                label,
                Overlay::default(),
                move |path, remap| {
                    backend::move_files(path, remap.get(from)?, remap.get(to)?, &files)
                },
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::StageOnBranch { branch, files } => return Some((branch, files)),
        }
        None
    }
}

/// Where `to` is, said after a verb: "to feature-x", "above a1b2c3d".
fn place(to: &Destination) -> String {
    match to {
        Destination::Branch(branch) => format!("to {}", short_ref(branch)),
        Destination::AboveBranch(branch) => format!("above {}", short_ref(branch)),
        Destination::AboveCommit(id) => format!("above {}", id.short()),
        Destination::BelowCommit(id) => format!("below {}", id.short()),
    }
}

/// `to`, with the commit it's relative to as it is after the operations queued before.
pub fn remapped(to: Destination, remap: &Remap) -> anyhow::Result<Destination> {
    Ok(match to {
        Destination::AboveCommit(id) => Destination::AboveCommit(remap.get(id)?),
        Destination::BelowCommit(id) => Destination::BelowCommit(remap.get(id)?),
        branch => branch,
    })
}

pub fn short_ref(ref_name: &str) -> &str {
    ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name)
}
