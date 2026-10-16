//! What happens when something cut or dragged lands somewhere. Drag-and-drop and
//! ⌘X/⌘V share this, so both always agree on what a drop means.

use std::sync::Arc;

use gpui::{Context, SharedString};

use crate::backend::{self, CommitId, Destination, Selected};
use crate::state::{Cut, Selection, Store};

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
            (
                Cut::Files {
                    from: None, files, ..
                },
                Selection::Commit(_, into),
            ) => Transfer::Amend {
                into: *into,
                files: files.clone(),
            },
            (
                Cut::Files {
                    from: None, files, ..
                },
                Selection::Branch(_, branch),
            ) => Transfer::StageOnBranch {
                branch: branch.clone(),
                files: files.clone(),
            },
            (
                Cut::Files {
                    from: Some(from),
                    files,
                    ..
                },
                Selection::Uncommitted(_),
            ) => Transfer::UncommitFiles {
                from: *from,
                files: files.clone(),
            },
            (
                Cut::Files {
                    from: Some(from),
                    files,
                    ..
                },
                Selection::Commit(_, to),
            ) if from != to => Transfer::MoveFiles {
                from: *from,
                to: *to,
                files: files.clone(),
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
            Transfer::MoveCommit {
                to: Destination::Branch(branch),
                ..
            } => format!("Move to {}", short_ref(branch)).into(),
            Transfer::MoveCommit {
                to: Destination::AboveCommit(id),
                ..
            } => format!("Move above {}", id.short()).into(),
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
            Transfer::Uncommit(id) => store.run(
                project,
                label,
                move |path| backend::uncommit(path, &[id]),
                |store, (), cx| store.snack_undoable("Uncommitted", cx),
                cx,
            ),
            Transfer::UncommitFiles { from, files } => store.run(
                project,
                label,
                move |path| backend::uncommit_files(path, from, &files),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Moved to uncommitted changes", cx)
                },
                cx,
            ),
            Transfer::MoveCommit { id, to } => store.run(
                project,
                label,
                move |path| backend::move_commits(path, &[id], &to),
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::Squash { id, into } => store.run(
                project,
                label,
                move |path| backend::squash(path, &[id], into),
                |store, rewritten, cx| {
                    store.follow(&rewritten, cx);
                    store.snack_undoable("Squashed", cx)
                },
                cx,
            ),
            Transfer::Amend { into, files } => store.run(
                project,
                label,
                move |path| backend::amend(path, into, &files),
                |store, (rewritten, left_out), cx| {
                    store.follow(&rewritten, cx);
                    store.report_left_out("Amended", &left_out, cx)
                },
                cx,
            ),
            Transfer::MoveFiles { from, to, files } => store.run(
                project,
                label,
                move |path| backend::move_files(path, from, to, &files),
                |store, rewritten, cx| store.follow(&rewritten, cx),
                cx,
            ),
            Transfer::StageOnBranch { branch, files } => return Some((branch, files)),
        }
        None
    }
}

pub fn short_ref(ref_name: &str) -> &str {
    ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name)
}
