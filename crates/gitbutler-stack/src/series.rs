use git2::Commit;
use gitbutler_commit::commit_ext::CommitVecExt;
use std::collections::HashMap;

use crate::PatchReference;

/// Series or (patch) Series is a set of patches (commits) that are dependent on each other.
/// This is effectively a sub-branch within a (series) stack.
/// The difference from a branch is that only the patches (commits) unique to the series are included.
///
/// The `pushed` status, as well as the `remote_reference` can be obtained from the methods on `head` (PatchReference).
#[derive(Debug, Clone)]
pub struct Series<'a> {
    /// The GitButler-managed head reference for this series. It points to a commit ID or a change ID in the stack.
    /// This head may or may not be part of the commits that are in the series
    /// There may be multiple "series" that point to the same head (e.g. when a new series / head is created)
    pub head: PatchReference,
    /// The local commits that are part of this series.
    /// The commits in one "series" never overlap with the commits in another series.
    /// Topologically ordered, the first entry is the newest in the series.
    pub local_commits: Vec<Commit<'a>>,
    /// The remote commits that are part of this series.
    /// If the branch/series have never been pushed, this list will be empty.
    /// Topologically ordered, the first entry is the newest in the series.
    pub remote_commits: Vec<Commit<'a>>,
    /// The list of patches that are only in the upstream (remote) and not in the local commits,
    /// as determined by the commit ID or change ID.
    pub upstream_only_commits: Vec<Commit<'a>>,
    /// The commit IDs of the remote commits that are part of this series, grouped by change id.
    /// Since we don't have a change_id to commit_id index, this is used to determine
    pub remote_commit_ids_by_change_id: HashMap<String, git2::Oid>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    pub archived: bool,
}

impl Series<'_> {
    /// Returns `true` if the provided patch is part of the remote commits in this series (i.e. has been pushed).
    pub fn remote(&self, patch: &Commit<'_>) -> bool {
        self.remote_commits.contains_by_commit_or_change_id(patch)
    }
}
