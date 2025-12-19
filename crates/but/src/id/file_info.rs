use std::collections::BTreeSet;

use bstr::BString;
use but_core::ref_metadata::StackId;
use but_hunk_assignment::HunkAssignment;

/// Information about files needed for CLI ID generation.
/// It's really just a named return value.
pub(crate) struct FileInfo {
    /// Uncommitted files paired with their stack assignments, ordered by assignment then filename.
    pub(crate) uncommitted_files: Vec<(Option<StackId>, BString)>,
    /// Committed files paired with their commit IDs, ordered by commit ID then filename.
    pub(crate) committed_files: Vec<(gix::ObjectId, BString)>,
}

impl FileInfo {
    /// Extracts file information from workspace commits and worktree status.
    ///
    /// This function processes workspace commits to find all changed files in each commit,
    /// and combines this with hunk assignment information to identify uncommitted (and
    /// possibly assigned) files in the worktree.
    pub(crate) fn from_workspace_commits_and_status<F>(
        workspace_commit_and_first_parent_ids: &[(gix::ObjectId, Option<gix::ObjectId>)],
        mut changed_paths_fn: F,
        hunk_assignments: &[HunkAssignment],
    ) -> anyhow::Result<Self>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<BString>>,
    {
        let mut committed_files: Vec<(gix::ObjectId, BString)> = Vec::new();
        for (commit_id, parent_id) in workspace_commit_and_first_parent_ids {
            let changed_paths = changed_paths_fn(*commit_id, *parent_id)?;
            for changed_path in changed_paths {
                committed_files.push((*commit_id, changed_path));
            }
        }

        let mut uncommitted_files: BTreeSet<(Option<StackId>, BString)> = BTreeSet::new();
        for assignment in hunk_assignments {
            uncommitted_files.insert((assignment.stack_id, assignment.path_bytes.clone()));
        }

        Ok(Self {
            committed_files,
            uncommitted_files: uncommitted_files.into_iter().collect(),
        })
    }
}
