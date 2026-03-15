//! Commands in need of being ported to the non-legacy world.
//! Doing so means that no legacy-APIs are used.

use anyhow::Result;
use bstr::ByteSlice;

pub mod absorb;
pub mod actions;
pub mod ai;
pub mod branch;
pub mod commit;
pub mod diff;
pub mod discard;
pub mod forge;
pub mod mark;
pub mod mcp;
pub mod mcp_internal;
pub mod merge;
pub mod oplog;
pub mod pick;
pub mod pull;
pub mod push;
pub mod refresh;
pub mod resolve;
pub mod reword;
pub mod rub;
pub mod setup;
pub mod show;
pub mod status;
pub mod teardown;
pub mod unapply;
pub mod worktree;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ShowDiffInEditor {
    /// The user requested we always show the diff.
    Always,
    /// The user requested we never show the diff.
    Never,
    /// The user didn't specify a preference.
    Unspecified,
}

impl ShowDiffInEditor {
    pub(crate) fn from_args(diff: bool, no_diff: bool) -> Option<Self> {
        match (diff, no_diff) {
            (true, true) => None,
            (true, false) => Some(Self::Always),
            (false, true) => Some(Self::Never),
            (false, false) => Some(Self::Unspecified),
        }
    }

    /// Decide whether the diff should be shown in the editor.
    ///
    /// For `Always`/`Never` the answer is immediate. For `Unspecified`, the provided
    /// `estimate_blob_size` closure is called to compute the total blob size and compare
    /// it against `MAX_DIFF_BLOB_SIZE_FOR_EDITOR_IF_UNSPECIFIED`.
    pub(crate) fn should_show_diff(
        self,
        estimate_blob_size: impl FnOnce() -> Result<u64>,
    ) -> Result<bool> {
        match self {
            Self::Always => Ok(true),
            Self::Never => Ok(false),
            Self::Unspecified => {
                let total_blob_size = estimate_blob_size()?;
                Ok(total_blob_size <= MAX_DIFF_BLOB_SIZE_FOR_EDITOR_IF_UNSPECIFIED)
            }
        }
    }
}

/// The maximum total blob size (in bytes) for which we'll show the diff in the editor
/// when the user hasn't specified a preference. This uses object header lookups
/// which are cheap compared to actually computing diffs.
///
/// 900KB is very roughly 15,000 lines at ~60 bytes per line. Just to protect the user from
/// stalling their system if they accidentally commit a big log file.
pub(crate) const MAX_DIFF_BLOB_SIZE_FOR_EDITOR_IF_UNSPECIFIED: u64 = 900_000;

/// Sum the blob sizes involved in the given tree changes using cheap object header lookups.
/// For modifications/renames, uses the larger of the two sides as an upper bound.
pub(crate) fn estimate_diff_blob_size(
    changes: &[but_core::TreeChange],
    ctx: &but_ctx::Context,
) -> Result<u64> {
    fn blob_size(repo: &gix::Repository, id: gix::ObjectId) -> u64 {
        repo.find_header(id).map(|h| h.size()).unwrap_or(0)
    }

    let repo = ctx.repo.get()?;

    Ok(changes
        .iter()
        .map(|change| match &change.status {
            but_core::TreeStatus::Addition { state, .. } => blob_size(&repo, state.id),
            but_core::TreeStatus::Deletion { previous_state } => {
                blob_size(&repo, previous_state.id)
            }
            but_core::TreeStatus::Modification {
                previous_state,
                state,
                ..
            }
            | but_core::TreeStatus::Rename {
                previous_state,
                state,
                ..
            } => {
                let a = blob_size(&repo, previous_state.id);
                let b = blob_size(&repo, state.id);
                a.max(b)
            }
        })
        .fold(0, |a, b| a.saturating_add(b)))
}

/// Return the commit message summary, or `(no message)` if the summary is empty.
pub(crate) fn commit_summary(commit: &gix::objs::CommitRef<'_>) -> String {
    let summary = commit.message().summary();
    if summary.is_empty() {
        "(no message)".to_string()
    } else {
        summary.to_string()
    }
}

/// A summarized file change used by legacy show-style command output.
#[derive(Debug, serde::Serialize)]
pub(crate) struct FileChange {
    /// The repository-relative path of the changed file.
    path: String,
    /// The simplified change kind, for example `added` or `modified`.
    status: String,
    /// The number of inserted lines in the rendered patch, if any.
    insertions: usize,
    /// The number of deleted lines in the rendered patch, if any.
    deletions: usize,
}

/// Convert a tree change into the legacy file summary shape.
pub(crate) fn file_change_from_tree_change(
    repo: &gix::Repository,
    change: but_core::TreeChange,
) -> Result<FileChange> {
    let (insertions, deletions) = match change.unified_patch(repo, 0)? {
        Some(but_core::UnifiedPatch::Patch {
            lines_added,
            lines_removed,
            ..
        }) => (
            usize::try_from(lines_added).unwrap_or(usize::MAX),
            usize::try_from(lines_removed).unwrap_or(usize::MAX),
        ),
        _ => (0, 0),
    };

    Ok(FileChange {
        path: change.path.to_str_lossy().to_string(),
        status: change_status(&change).to_string(),
        insertions,
        deletions,
    })
}

/// Convert a tree change kind to the human and JSON status strings used by legacy show output.
fn change_status(change: &but_core::TreeChange) -> &'static str {
    match change.status {
        but_core::TreeStatus::Addition { .. } => "added",
        but_core::TreeStatus::Deletion { .. } => "deleted",
        but_core::TreeStatus::Modification { .. } => "modified",
        but_core::TreeStatus::Rename { .. } => "renamed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_diff_in_editor() {
        assert_eq!(
            Some(ShowDiffInEditor::Always),
            ShowDiffInEditor::from_args(true, false)
        );

        assert_eq!(
            Some(ShowDiffInEditor::Never),
            ShowDiffInEditor::from_args(false, true)
        );

        assert_eq!(
            Some(ShowDiffInEditor::Unspecified),
            ShowDiffInEditor::from_args(false, false)
        );

        assert_eq!(None, ShowDiffInEditor::from_args(true, true));
    }
}
