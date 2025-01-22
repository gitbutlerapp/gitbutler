use crate::error::Error;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use gix::bstr::BString;
use gix::object::tree::EntryKind;
use serde::{Deserialize, Serialize};
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
