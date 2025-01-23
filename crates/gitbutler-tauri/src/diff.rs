use crate::error::Error;
use crate::from_json::HexHash;
use but_core::IgnoredWorktreeChange;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use gix::bstr::BString;
use gix::object::tree::EntryKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::instrument;

/// The array of unified diffs matches `changes`, so that `result[n] = unified_diff_of(changes[n])`.
#[tauri::command(async)]
#[instrument(skip(projects, change), err(Debug))]
pub fn tree_change_diffs(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    change: TreeChange,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    let change: but_core::TreeChange = change.into();
    let project = projects.get(project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    change.unified_diff(&repo).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn commit_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    old_commit_id: Option<HexHash>,
    new_commit_id: HexHash,
) -> anyhow::Result<Vec<TreeChange>, Error> {
    let project = projects.get(project_id)?;
    commit_changes_by_worktree_dir(
        project.path,
        old_commit_id.map(Into::into),
        new_commit_id.into(),
    )
    .map_err(Into::into)
}

pub fn commit_changes_by_worktree_dir(
    worktree_dir: PathBuf,
    old_commit_id: Option<gix::ObjectId>,
    new_commit_id: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    let repo = gix::open(worktree_dir).map_err(anyhow::Error::from)?;
    but_core::diff::commit_changes(&repo, old_commit_id, new_commit_id)
        .map(|c| c.into_iter().map(Into::into).collect())
}

/// This UI-version of [`but_core::diff::worktree_status()`] simplifies the `git status` information for display in
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
    Ok(worktree_changes_by_worktree_dir(project.path)?)
}

pub fn worktree_changes_by_worktree_dir(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir)?;
    Ok(but_core::diff::worktree_changes(&repo)?.into())
}

/// The type returned by [`but_core::diff::worktree_status()`].
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
}

impl From<but_core::WorktreeChanges> for WorktreeChanges {
    fn from(
        but_core::WorktreeChanges {
            changes,
            ignored_changes,
        }: but_core::WorktreeChanges,
    ) -> Self {
        WorktreeChanges {
            changes: changes.into_iter().map(Into::into).collect(),
            ignored_changes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeChange {
    pub path: BStringForFrontend,
    /// Something silently carried back and forth between the frontend and the backend.
    pub path_bytes: BString,
    pub status: TreeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject")]
pub enum TreeStatus {
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
        flags: Option<ModeFlags>,
    },
    Rename {
        #[serde(rename = "previousPath")]
        previous_path: BStringForFrontend,
        /// Something silently carried back and forth between the frontend and the backend.
        #[serde(rename = "previousPathBytes")]
        previous_path_bytes: BString,
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
        state: ChangeState,
        flags: Option<ModeFlags>,
    },
}

impl From<TreeStatus> for but_core::TreeStatus {
    fn from(value: TreeStatus) -> Self {
        match value {
            TreeStatus::Addition {
                state,
                is_untracked,
            } => but_core::TreeStatus::Addition {
                state: state.into(),
                is_untracked,
            },
            TreeStatus::Deletion { previous_state } => but_core::TreeStatus::Deletion {
                previous_state: previous_state.into(),
            },
            TreeStatus::Modification {
                previous_state,
                state,
                flags,
            } => but_core::TreeStatus::Modification {
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
            TreeStatus::Rename {
                previous_path: _lossy,
                previous_path_bytes,
                previous_state,
                state,
                flags,
            } => but_core::TreeStatus::Rename {
                previous_path: previous_path_bytes,
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
        }
    }
}

impl From<but_core::TreeStatus> for TreeStatus {
    fn from(value: but_core::TreeStatus) -> Self {
        match value {
            but_core::TreeStatus::Addition {
                state,
                is_untracked,
            } => TreeStatus::Addition {
                state: state.into(),
                is_untracked,
            },
            but_core::TreeStatus::Deletion { previous_state } => TreeStatus::Deletion {
                previous_state: previous_state.into(),
            },
            but_core::TreeStatus::Modification {
                previous_state,
                state,
                flags,
            } => TreeStatus::Modification {
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
            but_core::TreeStatus::Rename {
                previous_path,
                previous_state,
                state,
                flags,
            } => TreeStatus::Rename {
                previous_path: previous_path.clone().into(),
                previous_path_bytes: previous_path,
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ChangeState {
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    pub kind: EntryKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum ModeFlags {
    ExecutableBitAdded,
    ExecutableBitRemoved,
    TypeChangeFileToLink,
    TypeChangeLinkToFile,
    TypeChange,
}

impl From<TreeChange> for but_core::TreeChange {
    fn from(
        TreeChange {
            path: _lossy,
            path_bytes,
            status,
        }: TreeChange,
    ) -> Self {
        but_core::TreeChange {
            path: path_bytes,
            status: status.into(),
        }
    }
}

impl From<but_core::TreeChange> for TreeChange {
    fn from(but_core::TreeChange { path, status }: but_core::TreeChange) -> Self {
        TreeChange {
            path: path.clone().into(),
            path_bytes: path,
            status: status.into(),
        }
    }
}

impl From<ChangeState> for but_core::ChangeState {
    fn from(ChangeState { id, kind }: ChangeState) -> Self {
        but_core::ChangeState { id, kind }
    }
}

impl From<but_core::ChangeState> for ChangeState {
    fn from(but_core::ChangeState { id, kind }: but_core::ChangeState) -> Self {
        ChangeState { id, kind }
    }
}

impl From<ModeFlags> for but_core::ModeFlags {
    fn from(value: ModeFlags) -> Self {
        match value {
            ModeFlags::ExecutableBitAdded => but_core::ModeFlags::ExecutableBitAdded,
            ModeFlags::ExecutableBitRemoved => but_core::ModeFlags::ExecutableBitRemoved,
            ModeFlags::TypeChangeFileToLink => but_core::ModeFlags::TypeChangeFileToLink,
            ModeFlags::TypeChangeLinkToFile => but_core::ModeFlags::TypeChangeLinkToFile,
            ModeFlags::TypeChange => but_core::ModeFlags::TypeChange,
        }
    }
}

impl From<but_core::ModeFlags> for ModeFlags {
    fn from(value: but_core::ModeFlags) -> Self {
        match value {
            but_core::ModeFlags::ExecutableBitAdded => ModeFlags::ExecutableBitAdded,
            but_core::ModeFlags::ExecutableBitRemoved => ModeFlags::ExecutableBitRemoved,
            but_core::ModeFlags::TypeChangeFileToLink => ModeFlags::TypeChangeFileToLink,
            but_core::ModeFlags::TypeChangeLinkToFile => ModeFlags::TypeChangeLinkToFile,
            but_core::ModeFlags::TypeChange => ModeFlags::TypeChange,
        }
    }
}
