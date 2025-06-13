use crate::CommitIndex;
use bitflags::bitflags;
use gix::bstr::BString;
use std::ops::{Deref, DerefMut};

/// A commit with must useful information extracted from the Git commit itself.
///
/// Note that additional information can be computed and placed in the [`LocalCommit`] and [`RemoteCommit`]
#[derive(Clone, Eq, PartialEq)]
pub struct Commit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// The complete message, verbatim.
    pub message: BString,
    /// The signature at which the commit was authored.
    pub author: gix::actor::Signature,
    /// The references pointing to this commit, even after dereferencing tag objects.
    /// These can be names of tags and branches.
    pub refs: Vec<gix::refs::FullName>,
    /// Additional properties to help classify this commit.
    pub flags: CommitFlags,
    // TODO: bring has_conflict: bool here, then remove `RemoteCommit` type.
}

impl Commit {
    /// Read the object of the `commit_id` and extract relevant values, while setting `flags` as well.
    pub fn new_from_id(commit_id: gix::Id<'_>, flags: CommitFlags) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.into_commit();
        // Decode efficiently, no need to own this.
        let commit = commit.decode()?;
        Ok(Commit {
            id: commit_id.detach(),
            parent_ids: commit.parents().collect(),
            message: commit.message.to_owned(),
            author: commit.author.to_owned()?,
            refs: Vec::new(),
            flags,
        })
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({hash}, {msg:?}{flags})",
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
            flags = if self.flags.is_empty() {
                "".to_string()
            } else {
                format!(", {}", self.flags.debug_string())
            }
        )
    }
}

impl From<but_core::Commit<'_>> for Commit {
    fn from(value: but_core::Commit<'_>) -> Self {
        Commit {
            id: value.id.into(),
            parent_ids: value.parents.iter().cloned().collect(),
            message: value.inner.message,
            author: value.inner.author,
            refs: Vec::new(),
            flags: CommitFlags::empty(),
        }
    }
}

bitflags! {
    /// Provide more information about a commit, as gathered during traversal.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CommitFlags: u8 {
        /// Following the graph upward will lead to at least one tip that is a workspace.
        ///
        /// Note that if this flag isn't present, this means the commit isn't reachable
        /// from a workspace.
        const InWorkspace = 1;
    }
}

impl CommitFlags {
    /// Return a less verbose debug string
    pub fn debug_string(&self) -> String {
        if self.is_empty() {
            "".into()
        } else {
            let string = format!("{:?}", self);
            let out = &string["CommitFlags(".len()..];
            out[..out.len() - 1].to_string()
        }
    }
}

/// A commit that is reachable through the *local tracking branch*, with additional, computed information.
#[derive(Clone, Eq, PartialEq)]
pub struct LocalCommit {
    /// The simple commit.
    pub inner: Commit,
    /// Provide additional information on how this commit relates to other points of reference, like its remote branch,
    /// or the target branch to integrate with.
    pub relation: LocalCommitRelation,
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    pub has_conflicts: bool,
}

impl std::fmt::Debug for LocalCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let refs = self
            .refs
            .iter()
            .map(|rn| format!("►{}", rn.shorten()))
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "LocalCommit({conflict}{hash}, {msg:?}, {relation}{refs})",
            conflict = if self.has_conflicts { "💥" } else { "" },
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
            relation = self.relation.display(self.id),
            refs = if refs.is_empty() {
                "".to_string()
            } else {
                format!(", {refs}")
            }
        )
    }
}

impl LocalCommit {
    /// Create a new branch-commit, along with default values for the non-commit fields.
    // TODO: remove this function once ref_info code doesn't need it anymore (i.e. mapping is implemented).
    pub fn new_from_id(value: gix::Id<'_>, flags: CommitFlags) -> anyhow::Result<Self> {
        Ok(LocalCommit {
            inner: Commit::new_from_id(value, flags)?,
            relation: LocalCommitRelation::LocalOnly,
            has_conflicts: false,
        })
    }
}

/// The state of the [local commit](LocalCommit) in relation to its remote tracking branch or its integration branch.
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy)]
pub enum LocalCommitRelation {
    /// The commit is only local
    #[default]
    LocalOnly,
    /// The commit is also present in the remote tracking branch.
    ///
    /// This is the case if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased
    /// or updated in another way.
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated.
    /// This should happen when the commit or the contents of this commit is already part of the base.
    Integrated,
}

impl LocalCommitRelation {
    /// Convert this relation into something displaying, mainly for debugging.
    pub fn display(&self, id: gix::ObjectId) -> &'static str {
        match self {
            LocalCommitRelation::LocalOnly => "local",
            LocalCommitRelation::LocalAndRemote(remote_id) => {
                if *remote_id == id {
                    "local/remote(identity)"
                } else {
                    "local/remote(similarity)"
                }
            }
            LocalCommitRelation::Integrated => "integrated",
        }
    }
}

impl Deref for LocalCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LocalCommit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A commit that is reachable only through the *remote tracking branch*, with additional, computed information.
///
/// TODO: Remote commits can also be integrated, without the local branch being all caught up. Currently we can't represent that.
#[derive(Clone, Eq, PartialEq)]
pub struct RemoteCommit {
    /// The simple commit.
    pub inner: Commit,
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    /// For remote commits, this only happens if someone manually pushed them.
    pub has_conflicts: bool,
}

impl std::fmt::Debug for RemoteCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RemoteCommit({conflict}{hash}, {msg:?}",
            conflict = if self.has_conflicts { "💥" } else { "" },
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
        )
    }
}

impl Deref for RemoteCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for RemoteCommit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A segment of a commit graph, representing a set of commits exclusively.
#[derive(Default, Clone, Eq, PartialEq)]
pub struct Segment {
    /// The name of the branch at the tip of the segment, i.e. at the first commit.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    pub ref_name: Option<gix::refs::FullName>,
    /// An ID which can uniquely identify this segment among all segments within the graph that owned it.
    /// Note that it's not suitable to permanently identify the segment, so should not be persisted.
    pub id: usize,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
    /// for that stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
    pub commits: Vec<LocalCommit>,
    /// Commits that are reachable from the remote-tracking branch associated with this branch,
    /// but are not reachable from this branch or duplicated by a commit in it.
    /// Note that commits that are also similar to commits in `commits` are pruned, and not present here.
    ///
    /// Note that remote commits along with their remote tracking branch should always retain a shared history
    /// with the local tracking branch. If these diverge, we can represent this in data, but currently there is
    /// no derived value to make this visible explicitly.
    // TODO: remove this in favor of having a UI-only variant of the segment that contains these.
    pub commits_unique_in_remote_tracking_branch: Vec<RemoteCommit>,
    /// Read-only metadata with additional information, or `None` if nothing was present.
    pub metadata: Option<SegmentMetadata>,
}

/// Metadata for segments, which are either dedicated branches or represent workspaces.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SegmentMetadata {
    /// [Segments](Segment) with this data are considered a branch in the workspace.
    Branch(but_core::ref_metadata::Branch),
    /// [Segments](Segment) with this data are considered the tip of the workspace.
    Workspace(but_core::ref_metadata::Workspace),
}

/// Direct Access (without graph)
impl Segment {
    /// Return the top-most commit id of the segment.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits.first().map(|commit| commit.id)
    }

    /// Return the index of the last (present) commit, or `None` if there is no commit stored in this segment.
    pub fn last_commit_index(&self) -> Option<usize> {
        self.commits.len().checked_sub(1)
    }

    /// Try to find the index of `id` in our list of local commits.
    pub fn commit_index_of(&self, id: gix::ObjectId) -> Option<usize> {
        self.commits
            .iter()
            .enumerate()
            .find_map(|(cidx, c)| (c.id == id).then_some(cidx))
    }

    /// Find the commit associated with the given `commit_index`, which for convenience is optional.
    pub fn commit_by_index(&self, idx: Option<CommitIndex>) -> Option<&LocalCommit> {
        self.commits.get(idx?)
    }

    /// Return the flags of the first commit if non-empty, which is the top-most commit in the stack assuming
    /// it grows upwards into the future.
    pub fn flags_of_first_commit(&self) -> Option<CommitFlags> {
        let commit = self.commits.first()?;
        (!commit.flags.is_empty()).then_some(commit.flags)
    }

    /// Return `Some(md)` if this segment contains workspace metadata, which makes it governing a workspace.
    pub fn workspace_metadata(&self) -> Option<&but_core::ref_metadata::Workspace> {
        self.metadata.as_ref().and_then(|md| match md {
            SegmentMetadata::Workspace(md) => Some(md),
            _ => None,
        })
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Segment {
            ref_name,
            id,
            commits,
            commits_unique_in_remote_tracking_branch,
            remote_tracking_ref_name,
            metadata,
        } = self;
        f.debug_struct("StackSegment")
            .field("id", &id)
            .field(
                "ref_name",
                &match ref_name.as_ref() {
                    None => "None".to_string(),
                    Some(name) => name.to_string(),
                },
            )
            .field(
                "remote_tracking_ref_name",
                &match remote_tracking_ref_name.as_ref() {
                    None => "None".to_string(),
                    Some(name) => name.to_string(),
                },
            )
            .field("commits", &commits)
            .field(
                "commits_unique_in_remote_tracking_branch",
                &commits_unique_in_remote_tracking_branch,
            )
            .field(
                "metadata",
                match metadata {
                    None => &"None",
                    Some(SegmentMetadata::Branch(m)) => m,
                    Some(SegmentMetadata::Workspace(m)) => m,
                },
            )
            .finish()
    }
}
