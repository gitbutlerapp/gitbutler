#![allow(missing_docs)]
use bstr::BString;
use but_serde::BStringForFrontend;
use gix::object::tree::EntryKind;
use serde::{Deserialize, Serialize};

use crate::IgnoredWorktreeChange;

/// The type returned by [`crate::diff::worktree_changes()`].
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
}

impl WorktreeChanges {
    pub fn try_to_unidiff(
        &self,
        repo: &gix::Repository,
        context_lines: u32,
    ) -> anyhow::Result<BString> {
        changes_to_unidiff(self.changes.clone(), repo, context_lines)
    }
}

impl From<crate::WorktreeChanges> for WorktreeChanges {
    fn from(
        crate::WorktreeChanges {
            changes,
            ignored_changes,
            index_changes: _,
            index_conflicts: _,
        }: crate::WorktreeChanges,
    ) -> Self {
        WorktreeChanges {
            changes: changes.into_iter().map(Into::into).collect(),
            ignored_changes,
        }
    }
}

/// All the changes that were made to the tree, including stats
#[derive(Default, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeChanges {
    /// The changes that were made to the tree.
    pub changes: Vec<TreeChange>,
    /// The stats of the changes.
    pub stats: TreeStats,
}

impl TreeChanges {
    pub fn try_to_unidiff(
        &self,
        repo: &gix::Repository,
        context_lines: u32,
    ) -> anyhow::Result<BString> {
        changes_to_unidiff(self.changes.clone(), repo, context_lines)
    }
}

/// Notably skip changes that
fn changes_to_unidiff(
    changes: Vec<TreeChange>,
    repo: &gix::Repository,
    context_lines: u32,
) -> anyhow::Result<BString> {
    let mut out = BString::default();
    for change in changes {
        let Some(diff) = crate::TreeChange::from(change).unified_diff(repo, context_lines)? else {
            continue;
        };
        out.extend_from_slice(&diff);
        out.push(b'\n');
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeChange {
    pub path: BStringForFrontend,
    /// Something silently carried back and forth between the frontend and the backend.
    pub path_bytes: BString,
    pub status: TreeStatus,
}

impl From<gix::object::tree::diff::Stats> for TreeStats {
    fn from(stats: gix::object::tree::diff::Stats) -> Self {
        TreeStats {
            lines_added: stats.lines_added,
            lines_removed: stats.lines_removed,
            files_changed: stats.files_changed,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeStats {
    /// The total amount of lines added.
    pub lines_added: u64,
    /// The total amount of lines removed.
    pub lines_removed: u64,
    /// The number of files added, removed or modified.
    pub files_changed: u64,
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
    #[serde(with = "but_serde::object_id")]
    pub id: gix::ObjectId,
    pub kind: EntryKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(missing_docs)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeUnifiedDiff {
    tree_change: TreeChange,
    diff: crate::UnifiedPatch,
}

impl From<&(crate::TreeChange, crate::UnifiedPatch)> for ChangeUnifiedDiff {
    fn from(unified_diff: &(crate::TreeChange, crate::UnifiedPatch)) -> Self {
        ChangeUnifiedDiff {
            tree_change: unified_diff.0.clone().into(),
            diff: unified_diff.1.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatChangeUnifiedDiff {
    pub path: BStringForFrontend,
    pub status: String,
    pub diff: crate::UnifiedPatch,
}

fn status_to_string(status: &crate::TreeStatus) -> String {
    match status {
        crate::TreeStatus::Addition { .. } => "addition".to_string(),
        crate::TreeStatus::Deletion { .. } => "deletion".to_string(),
        crate::TreeStatus::Modification { .. } => "modification".to_string(),
        crate::TreeStatus::Rename { .. } => "rename".to_string(),
    }
}

impl From<&(crate::TreeChange, crate::UnifiedPatch)> for FlatChangeUnifiedDiff {
    fn from(unified_diff: &(crate::TreeChange, crate::UnifiedPatch)) -> Self {
        FlatChangeUnifiedDiff {
            path: unified_diff.0.path.clone().into(),
            status: status_to_string(&unified_diff.0.status),
            diff: unified_diff.1.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatUnifiedWorktreeChanges {
    /// Unified diff changes that could be committed.
    pub changes: Vec<FlatChangeUnifiedDiff>,
}

impl From<&Vec<(crate::TreeChange, crate::UnifiedPatch)>> for FlatUnifiedWorktreeChanges {
    fn from(changes: &Vec<(crate::TreeChange, crate::UnifiedPatch)>) -> Self {
        FlatUnifiedWorktreeChanges {
            changes: changes.iter().map(FlatChangeUnifiedDiff::from).collect(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedWorktreeChanges {
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    ignored_changes: Vec<IgnoredWorktreeChange>,
    /// Unified diff changes that could be committed.
    changes: Vec<ChangeUnifiedDiff>,
}

impl
    From<(
        crate::WorktreeChanges,
        &Vec<(crate::TreeChange, crate::UnifiedPatch)>,
    )> for UnifiedWorktreeChanges
{
    fn from(
        (worktree_changes, changes): (
            crate::WorktreeChanges,
            &Vec<(crate::TreeChange, crate::UnifiedPatch)>,
        ),
    ) -> Self {
        UnifiedWorktreeChanges {
            ignored_changes: worktree_changes.ignored_changes,
            changes: changes.iter().map(ChangeUnifiedDiff::from).collect(),
        }
    }
}
