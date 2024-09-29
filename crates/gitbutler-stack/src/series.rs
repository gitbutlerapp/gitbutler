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
