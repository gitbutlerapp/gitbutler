/// A bundle of information about a commit.
#[derive(Debug, Clone)]
pub struct CommitDetails {
    /// The fully decoded commit.
    pub commit: crate::CommitOwned,
    /// The changes between the tree of the first parent and this commit.
    pub diff_with_first_parent: Vec<crate::TreeChange>,
    /// The stats of the changes, which are computed only when explicitly requested.
    pub line_stats: Option<LineStats>,
    /// Represents what was causing a particular commit to conflict when rebased.
    pub conflict_entries: Option<crate::commit::ConflictEntries>,
}

/// Line statistics obtained from diffing the blobs of one or more [TreeChange](crate::TreeChange).
#[derive(Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, serde::Serialize)]
pub struct LineStats {
    /// The total amount of lines added in the between blobs of the two trees.
    pub lines_added: u64,
    /// The total amount of lines removed in the between blobs of the two trees.
    pub lines_removed: u64,
    /// The number of files that contributed to these statistics as they were added, removed or modified.
    pub files_changed: u64,
}

/// Lifecycle
impl CommitDetails {
    ///Compute the tree-diff for `commit_id` with its first parent and optionally calculate `line_stats`.
    pub fn from_commit_id(commit_id: gix::Id, line_stats: bool) -> anyhow::Result<Self> {
        let repo = commit_id.repo;
        let commit = repo.find_commit(commit_id)?;
        let first_parent_commit_id = commit.parent_ids().map(|id| id.detach()).next();

        let changes =
            crate::diff::TreeChanges::from_trees(repo, first_parent_commit_id, commit_id.detach())?;
        let line_stats = line_stats
            .then(|| changes.compute_line_stats(repo))
            .transpose()?;

        let commit = crate::Commit::try_from(commit)?;
        let conflict_entries = commit.conflict_entries()?;
        Ok(CommitDetails {
            commit: commit.detach(),
            diff_with_first_parent: changes.into_tree_changes(),
            line_stats: line_stats.map(Into::into),
            conflict_entries,
        })
    }
}

impl From<gix::object::tree::diff::Stats> for LineStats {
    fn from(
        gix::object::tree::diff::Stats {
            lines_added,
            lines_removed,
            files_changed,
        }: gix::object::tree::diff::Stats,
    ) -> Self {
        LineStats {
            lines_added,
            lines_removed,
            files_changed,
        }
    }
}
