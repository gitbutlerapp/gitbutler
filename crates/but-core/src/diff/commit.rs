use crate::{ChangeState, TreeStatus};
use crate::{Commit, ModeFlags, TreeChange};
use gix::diff::tree_with_rewrites::Change;
use gix::prelude::ObjectIdExt;

/// Produce all changes that are needed to turn the tree of `lhs_commit` into the tree of `rhs_commit`.
/// If `lhs_commit` is `None`, it will be treated like an empty tree, which is useful if
/// there was no tree to compare `lhs_commit` to (e.g. in case of the first commit).
///
/// Note that we deal with conflicted commits correctly by resolving to the actual tree, not the one with meta-data.
///
/// They are sorted by their current path.
pub fn to_commit(
    repo: &gix::Repository,
    lhs_commit: Option<gix::ObjectId>,
    rhs_commit: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    let lhs_tree = lhs_commit
        .map(|commit_id| {
            Commit::from_id(commit_id.attach(repo)).and_then(|commit| {
                let id = commit.tree_id()?;
                Ok(id.object()?.into_tree())
            })
        })
        .transpose()?;
    let rhs_tree = Commit::from_id(rhs_commit.attach(repo))?
        .tree_id()?
        .object()
        .map(|obj| obj.into_tree())?;

    let changes = repo.diff_tree_to_tree(lhs_tree.as_ref(), &rhs_tree, None)?;
    let mut out: Vec<TreeChange> = changes.into_iter().map(Into::into).collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

impl From<gix::object::tree::diff::ChangeDetached> for TreeChange {
    fn from(value: gix::object::tree::diff::ChangeDetached) -> Self {
        match value {
            Change::Addition {
                location,
                entry_mode,
                id,
                ..
            } => TreeChange {
                path: location,
                status: TreeStatus::Addition {
                    state: ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    },
                    is_untracked: false,
                },
            },
            Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            } => TreeChange {
                path: location,
                status: TreeStatus::Deletion {
                    previous_state: ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    },
                },
            },
            Change::Modification {
                location,
                previous_entry_mode,
                previous_id,
                id,
                entry_mode,
            } => {
                let previous_state = ChangeState {
                    id: previous_id,
                    kind: previous_entry_mode.kind(),
                };
                let state = ChangeState {
                    id,
                    kind: entry_mode.kind(),
                };
                TreeChange {
                    path: location,
                    status: TreeStatus::Modification {
                        previous_state,
                        state,
                        flags: ModeFlags::calculate(&previous_state, &state),
                    },
                }
            }
            Change::Rewrite {
                source_location,
                source_entry_mode,
                source_id,
                entry_mode,
                id,
                location,
                diff: _,
                copy: _,
                ..
            } => {
                let previous_state = ChangeState {
                    id: source_id,
                    kind: source_entry_mode.kind(),
                };
                let state = ChangeState {
                    id,
                    kind: entry_mode.kind(),
                };
                TreeChange {
                    path: location,
                    status: TreeStatus::Rename {
                        previous_path: source_location,
                        previous_state,
                        state,
                        flags: ModeFlags::calculate(&previous_state, &state),
                    },
                }
            }
        }
    }
}
