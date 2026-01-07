use bstr::BString;

/// Information about committed files needed for CLI ID generation.
pub(crate) struct FileInfo {
    /// Committed files paired with their commit IDs, ordered by commit ID then filename.
    pub(crate) committed_files: Vec<(gix::ObjectId, BString)>,
}

impl FileInfo {
    /// Extracts file information from workspace commits and worktree status.
    ///
    /// This function processes workspace commits to find all changed files in each commit.
    pub(crate) fn from_workspace_commits_and_status<'a, F>(
        workspace_commit_and_first_parent_ids: impl Iterator<
            Item = (&'a gix::ObjectId, &'a Option<gix::ObjectId>),
        >,
        mut changed_paths_fn: F,
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
        committed_files.sort();

        Ok(Self { committed_files })
    }
}
