use crate::{CommitIndex, SegmentIndex};
use bitflags::bitflags;
use gix::bstr::BString;

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
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    pub has_conflicts: bool,
}

impl Commit {
    /// Read the object of the `commit_id` and extract relevant values, while setting `flags` as well.
    pub fn new_from_id(
        commit_id: gix::Id<'_>,
        flags: CommitFlags,
        has_conflicts: bool,
    ) -> anyhow::Result<Self> {
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
            has_conflicts,
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
            has_conflicts: false,
        }
    }
}

bitflags! {
    /// Provide more information about a commit, as gathered during traversal.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CommitFlags: u8 {
        /// Identify commits that have never been owned *only* by a remote.
        /// It may be that a remote is directly pointing at them though.
        /// Note that this flag is negative as all flags are propagated through the graph,
        /// a property we don't want for this trait.
        const NotInRemote = 1 << 0;
        /// Following the graph upward will lead to at least one tip that is a workspace.
        ///
        /// Note that if this flag isn't present, this means the commit isn't reachable
        /// from a workspace.
        const InWorkspace = 1 << 1;
        /// The commit is reachable from either the target branch (usually `refs/remotes/origin/main`).
        /// Note that when multiple workspaces are included in the traversal, this flag is set by
        /// any of many target branches.
        const Integrated = 1 << 2;
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
            out[..out.len() - 1]
                .to_string()
                .replace("NotInRemote", "⌂")
                .replace("InWorkspace", "🏘️")
                .replace("Integrated", "✓")
                .replace(" ", "")
        }
    }
}

/// A segment of a commit graph, representing a set of commits exclusively.
#[derive(Default, Clone, Eq, PartialEq)]
pub struct Segment {
    /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    /// Alternatively, the naming would have been ambiguous.
    /// Finally, this is `None` of the original name can be found searching upwards, finding exactly one
    /// named segment.
    pub ref_name: Option<gix::refs::FullName>,
    /// An ID which can uniquely identify this segment among all segments within the graph that owned it.
    /// Note that it's not suitable to permanently identify the segment, so should not be persisted.
    pub id: SegmentIndex,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
    /// for that stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
    pub commits: Vec<Commit>,
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
    pub fn commit_id_by_index(&self, idx: Option<CommitIndex>) -> Option<gix::ObjectId> {
        self.commits.get(idx?).map(|c| c.id)
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
