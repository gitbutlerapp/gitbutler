use crate::error::Error;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use serde::Serialize;
use std::path::PathBuf;
use tracing::instrument;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ChangeState {
    #[serde(with = "gitbutler_serde::object_id")]
    id: gix::ObjectId,
    kind: gix::object::tree::EntryKind,
}

impl From<but_core::worktree::ChangeState> for ChangeState {
    fn from(but_core::worktree::ChangeState { id, kind }: but_core::worktree::ChangeState) -> Self {
        ChangeState { id, kind }
    }
}

/// Computed using the file kinds/modes of two [`ChangeState`] instances to represent
/// the *dominant* change to display. Note that it can stack with a content change,
/// but *should not only in case of a `TypeChange*`*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Flags {
    ExecutableBitAdded,
    ExecutableBitRemoved,
    TypeChangeFileToLink,
    TypeChangeLinkToFile,
    TypeChange,
}

impl Flags {
    fn calculate(
        old: &but_core::worktree::ChangeState,
        new: &but_core::worktree::ChangeState,
    ) -> Option<Self> {
        Self::calculate_inner(old.kind, new.kind)
    }

    fn calculate_inner(
        old: gix::object::tree::EntryKind,
        new: gix::object::tree::EntryKind,
    ) -> Option<Self> {
        use gix::object::tree::EntryKind as E;
        Some(match (old, new) {
            (E::Blob, E::BlobExecutable) => Flags::ExecutableBitAdded,
            (E::BlobExecutable, E::Blob) => Flags::ExecutableBitRemoved,
            (E::Blob | E::BlobExecutable, E::Link) => Flags::TypeChangeFileToLink,
            (E::Link, E::Blob | E::BlobExecutable) => Flags::TypeChangeLinkToFile,
            (a, b) if a != b => Flags::TypeChange,
            _ => return None,
        })
    }
}

/// For docs, see [`but_core::worktree::Status`].
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum Status {
    Addition {
        state: ChangeState,
        #[serde(rename = "isUntracked")]
        is_untracked: bool,
    },
    Deletion {
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
    },
    Modification {
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
        state: ChangeState,
        flags: Option<Flags>,
    },
    Rename {
        #[serde(rename = "previousPath")]
        previous_path: BStringForFrontend,
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
        state: ChangeState,
        flags: Option<Flags>,
    },
}

impl From<but_core::worktree::Status> for Status {
    fn from(value: but_core::worktree::Status) -> Self {
        use but_core::worktree::Status as E;
        match value {
            E::Conflict(_) => unreachable!("BUG: must have been extracted beforehand"),
            E::Untracked { state } => Status::Addition {
                state: state.into(),
                is_untracked: true,
            },
            E::Addition { origin: _, state } => Status::Addition {
                state: state.into(),
                is_untracked: false,
            },
            E::Deletion {
                origin: _,
                previous_state,
            } => Status::Deletion {
                previous_state: previous_state.into(),
            },
            E::Modification {
                origin: _,
                previous_state,
                state,
            } => Status::Modification {
                flags: Flags::calculate(&previous_state, &state),
                previous_state: previous_state.into(),
                state: state.into(),
            },
            E::Rename {
                origin: _,
                previous_path,
                previous_state,
                state,
            } => Status::Rename {
                flags: Flags::calculate(&previous_state, &state),
                previous_path: previous_path.into(),
                previous_state: previous_state.into(),
                state: state.into(),
            },
        }
    }
}

/// For documentation, see [`but_core::Change`].
#[derive(Debug, Clone, Serialize)]
pub struct TreeChange {
    path: BStringForFrontend,
    status: Status,
}

impl From<but_core::TreeChange> for TreeChange {
    fn from(but_core::TreeChange { path, status }: but_core::TreeChange) -> Self {
        TreeChange {
            path: path.into(),
            status: status.into(),
        }
    }
}

/// The status we can't handle.
#[derive(Debug, Clone, Serialize)]
pub enum IgnoredChangeStatus {
    /// A conflicting entry in the index. The worktree state of the entry is unclear.
    Conflict,
    /// A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.
    TreeIndex,
}

/// A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.
#[derive(Debug, Clone, Serialize)]
pub struct IgnoredChange {
    /// The worktree-relative path to the change.
    path: BStringForFrontend,
    status: IgnoredChangeStatus,
}

/// Keeps simplified [`Changes`] along with changes that were applied to the index that we chose to ignore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredChange>,
}

/// This UI-version of [`but_core::worktree::changes()`] simplifies the `git status` information for display in
/// the user interface as it is right now. From here, it's always possible to add more information as the need arises.
///
/// ### Notable Transformations
/// * There is no notion of an index (`.git/index`) - all changes seem to have happened in the worktree.
/// * Modifications that were made to the index will be ignored *only if* there is a worktree modification to the same file.
/// * conflicts are ignored
///
/// All ignored status changes are also provided so they can be displayed separately.
#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn worktree_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
) -> anyhow::Result<WorktreeChanges, Error> {
    let project = projects.get(project_id)?;
    Ok(changes_in_worktree(project.path)?)
}

fn changes_in_worktree(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir).map_err(anyhow::Error::new)?;
    let detailed_changes = but_core::worktree::changes(&repo)?;

    let (mut changes, mut ignored_changes) = (Vec::new(), Vec::new());
    let mut last_path = None;
    for change in detailed_changes {
        if last_path.as_ref() == Some(&change.path) {
            assert_eq!(
                change.status.origin(),
                but_core::worktree::Origin::TreeIndex,
                "worktree-changes should happen before tree-index changes, sorting is guaranteed by the API"
            );
            ignored_changes.push(IgnoredChange {
                path: change.path.into(),
                status: IgnoredChangeStatus::TreeIndex,
            });
            continue;
        }
        if matches!(change.status, but_core::worktree::Status::Conflict(_)) {
            ignored_changes.push(IgnoredChange {
                path: change.path.into(),
                status: IgnoredChangeStatus::Conflict,
            });
            continue;
        }
        last_path = Some(change.path.clone());
        changes.push(change.into());
    }
    Ok(WorktreeChanges {
        changes,
        ignored_changes,
    })
}

#[cfg(test)]
mod tests;
