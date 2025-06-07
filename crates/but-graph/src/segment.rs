use crate::CommitIndex;
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
    // TODO: bring has_conflict: bool here, then remove `RemoteCommit` type.
}

impl Commit {
    /// Read the object of the `commit_id` and extract relevant values.
    pub fn new_from_id(commit_id: gix::Id<'_>) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.into_commit();
        // Decode efficiently, no need to own this.
        let commit = commit.decode()?;
        Ok(Commit {
            id: commit_id.detach(),
            parent_ids: commit.parents().collect(),
            message: commit.message.to_owned(),
            author: commit.author.to_owned()?,
            refs: Vec::new(),
        })
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({hash}, {msg:?})",
            hash = self.id.to_hex_with_len(7),
            msg = self.message
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
    pub fn new_from_id(value: gix::Id<'_>) -> anyhow::Result<Self> {
        Ok(LocalCommit {
            inner: Commit::new_from_id(value)?,
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

/// A more detailed specification of a reference associated with a workspace, and it's location in comparison to a named reference point.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum RefLocation {
    /// The workspace commit can reach the given reference using a graph-walk.
    ///
    /// This is the common case.
    ReachableFromWorkspaceCommit,
    /// The given reference can reach into this workspace segment, but isn't fully inside it.
    ///
    /// This happens if someone checked out the reference directly and committed into it.
    OutsideOfWorkspace,
}

/// A list of all commits in a stack segment of a [`Stack`].
#[derive(Default, Clone, Eq, PartialEq)]
pub struct Segment {
    /// The name of the branch at the tip of it, and the starting point of the walk.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    pub ref_name: Option<gix::refs::FullName>,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// Specify where the `ref_name` is specifically in relation to a workspace, or `None` if there is no ref-name.
    pub ref_location: Option<RefLocation>,
    /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
    /// for that stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty.
    pub commits_unique_from_tip: Vec<LocalCommit>,
    /// Commits that are reachable from the remote-tracking branch associated with this branch,
    /// but are not reachable from this branch or duplicated by a commit in it.
    /// Note that commits that are also similar to commits in `commits_unique_from_tip` are pruned, and not present here.
    ///
    /// Note that remote commits along with their remote tracking branch should always retain a shared history
    /// with the local tracking branch. If these diverge, we can represent this in data, but currently there is
    /// no derived value to make this visible explicitly.
    // TODO: review this - should branch divergence be a thing? Rare, but not impossible.
    //       We linearize these, pretending a simpler history than there actually is.
    pub commits_unique_in_remote_tracking_branch: Vec<RemoteCommit>,
    /// Metadata with additional information, or `None` if nothing was present.
    ///
    /// Primary use for this is the consumer, as edits are forced to be made on 'connected' data, so refetching is necessary.
    pub metadata: Option<but_core::ref_metadata::Branch>,
}

impl Segment {
    /// Return the top-most commit id of the segment.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits_unique_from_tip.first().map(|commit| commit.id)
    }

    /// Try to find the index of `id` in our list of local commits.
    pub fn commit_index_of(&self, id: gix::ObjectId) -> Option<usize> {
        self.commits_unique_from_tip
            .iter()
            .enumerate()
            .find_map(|(cidx, c)| (c.id == id).then_some(cidx))
    }

    /// Find the commit associated with the given `commit_index`, which for convenience is optional.
    pub fn commit_by_index(&self, idx: Option<CommitIndex>) -> Option<&LocalCommit> {
        idx.and_then(|idx| self.commits_unique_from_tip.get(idx))
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Segment {
            ref_name,
            ref_location,
            commits_unique_from_tip,
            commits_unique_in_remote_tracking_branch,
            remote_tracking_ref_name,
            metadata,
        } = self;
        f.debug_struct("StackSegment")
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
            .field(
                "ref_location",
                &match ref_location {
                    None => "None".to_string(),
                    Some(location) => {
                        format!("{:?}", location)
                    }
                },
            )
            .field("commits_unique_from_tip", &commits_unique_from_tip)
            .field(
                "commits_unique_in_remote_tracking_branch",
                &commits_unique_in_remote_tracking_branch,
            )
            .field("metadata", &metadata)
            .finish()
    }
}
