use crate::{ChangeState, TreeStatus};
use crate::{ModeFlags, TreeChange};
use gix::diff::tree_with_rewrites::Change;
use gix::prelude::ObjectIdExt;

/// Produce all changes that are needed to turn `lhs_tree` into `rhs_tree`.
/// If `lhs_tree` is `None`, it will be treated like an empty tree, which is useful if
/// there was no tree to compare `lhs_tree` to (e.g. in case of the first commit).
///
/// They are sorted by their current path.
///
/// Note that the [`TreeChange`] instances returned *are not* ever [conflicts](TreeStatus::Conflict)
/// or [untracked](TreeStatus::Untracked) files.
/// Their origin is always [`TreeIndex`](worktree::Origin::TreeIndex) (even though it doesn't make much sense);
pub fn changes(
    repo: &gix::Repository,
    lhs_tree: Option<gix::ObjectId>,
    rhs_tree: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    let lhs_tree = lhs_tree
        .map(|id| id.attach(&repo).object().map(|obj| obj.into_tree()))
        .transpose()?;
    let rhs_tree = rhs_tree.attach(&repo).object().map(|obj| obj.into_tree())?;

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
