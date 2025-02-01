#![allow(missing_docs)]
use crate::IgnoredWorktreeChange;
use bstr::BString;
use gitbutler_serde::BStringForFrontend;
use gix::object::tree::EntryKind;
use serde::{Deserialize, Serialize};

/// The type returned by [`crate::diff::worktree_status()`].
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
}

impl From<crate::WorktreeChanges> for WorktreeChanges {
    fn from(
        crate::WorktreeChanges {
            changes,
            ignored_changes,
        }: crate::WorktreeChanges,
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

impl From<TreeStatus> for crate::TreeStatus {
    fn from(value: TreeStatus) -> Self {
        match value {
            TreeStatus::Addition {
                state,
                is_untracked,
            } => crate::TreeStatus::Addition {
                state: state.into(),
                is_untracked,
            },
            TreeStatus::Deletion { previous_state } => crate::TreeStatus::Deletion {
                previous_state: previous_state.into(),
            },
            TreeStatus::Modification {
                previous_state,
                state,
                flags,
            } => crate::TreeStatus::Modification {
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
            } => crate::TreeStatus::Rename {
                previous_path: previous_path_bytes,
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
        }
    }
}

impl From<crate::TreeStatus> for TreeStatus {
    fn from(value: crate::TreeStatus) -> Self {
        match value {
            crate::TreeStatus::Addition {
                state,
                is_untracked,
            } => TreeStatus::Addition {
                state: state.into(),
                is_untracked,
            },
            crate::TreeStatus::Deletion { previous_state } => TreeStatus::Deletion {
                previous_state: previous_state.into(),
            },
            crate::TreeStatus::Modification {
                previous_state,
                state,
                flags,
            } => TreeStatus::Modification {
                previous_state: previous_state.into(),
                state: state.into(),
                flags: flags.map(Into::into),
            },
            crate::TreeStatus::Rename {
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

impl From<TreeChange> for crate::TreeChange {
    fn from(
        TreeChange {
            path: _lossy,
            path_bytes,
            status,
        }: TreeChange,
    ) -> Self {
        crate::TreeChange {
            path: path_bytes,
            status: status.into(),
        }
    }
}

impl From<crate::TreeChange> for TreeChange {
    fn from(crate::TreeChange { path, status }: crate::TreeChange) -> Self {
        TreeChange {
            path: path.clone().into(),
            path_bytes: path,
            status: status.into(),
        }
    }
}

impl From<ChangeState> for crate::ChangeState {
    fn from(ChangeState { id, kind }: ChangeState) -> Self {
        crate::ChangeState { id, kind }
    }
}

impl From<crate::ChangeState> for ChangeState {
    fn from(crate::ChangeState { id, kind }: crate::ChangeState) -> Self {
        ChangeState { id, kind }
    }
}

impl From<ModeFlags> for crate::ModeFlags {
    fn from(value: ModeFlags) -> Self {
        match value {
            ModeFlags::ExecutableBitAdded => crate::ModeFlags::ExecutableBitAdded,
            ModeFlags::ExecutableBitRemoved => crate::ModeFlags::ExecutableBitRemoved,
            ModeFlags::TypeChangeFileToLink => crate::ModeFlags::TypeChangeFileToLink,
            ModeFlags::TypeChangeLinkToFile => crate::ModeFlags::TypeChangeLinkToFile,
            ModeFlags::TypeChange => crate::ModeFlags::TypeChange,
        }
    }
}

impl From<crate::ModeFlags> for ModeFlags {
    fn from(value: crate::ModeFlags) -> Self {
        match value {
            crate::ModeFlags::ExecutableBitAdded => ModeFlags::ExecutableBitAdded,
            crate::ModeFlags::ExecutableBitRemoved => ModeFlags::ExecutableBitRemoved,
            crate::ModeFlags::TypeChangeFileToLink => ModeFlags::TypeChangeFileToLink,
            crate::ModeFlags::TypeChangeLinkToFile => ModeFlags::TypeChangeLinkToFile,
            crate::ModeFlags::TypeChange => ModeFlags::TypeChange,
        }
    }
}
