use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};

/// Series or (patch) Series is a set of patches (commits) that are dependent on each other.
/// This is effectively a sub-branch within a (series) stack.
/// The difference from a branch is that only the patches (commits) unique to the series are included.
///
/// The `pushed` status, as well as the `remote_reference` can be obtained from the methods on `head` (PatchReference).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Series {
    /// The GitButler-managed head reference for this series. It points to a commit ID or a change ID in the stack.
    /// This head may or may not be part of the commits that are in the series
    /// There may be multiple "series" that point to the same head (e.g. when a new series / head is created)
    pub head: PatchReference,
    /// The local commits that are part of this series.
    /// The commits in one "series" never overlap with the commits in another series.
    /// Topologically ordered, the first entry is the newest in the series.
    pub local_commits: Vec<CommitOrChangeId>,
    /// The remote commits that are part of this series.
    /// If the branch/series have never been pushed, this list will be empty.
    /// Topologically ordered, the first entry is the newest in the series.
    pub remote_commits: Vec<CommitOrChangeId>,
}

impl Series {
    /// Returns `true` if the provided patch is part of the remote commits in this series (i.e. has been pushed).
    pub fn remote(&self, patch: &CommitOrChangeId) -> bool {
        self.remote_commits.contains(patch)
    }
    /// Returns a list of patches that are only in the upstream (remote) and not in the local commits,
    /// as determined by the commit ID or change ID.
    /// This comparison is peformed against the full stack of series.
    pub fn upstream_only(&self, stack_series: &[Series]) -> Vec<CommitOrChangeId> {
        let mut upstream_only = vec![];
        for commit in &self.remote_commits {
            if !stack_series
                .iter()
                .any(|s| s.local_commits.contains(commit))
            {
                upstream_only.push(commit.clone());
            }
        }
        upstream_only
    }
}
