use bitflags::bitflags;
use bstr::{BString, ByteSlice};

use crate::{CommitIndex, SegmentIndex};

/// A commit with must useful information extracted from the Git commit itself.
#[derive(Clone, Eq, PartialEq)]
pub struct Commit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// Additional properties to help classify this commit.
    pub flags: CommitFlags,
    /// The references pointing to this commit, even after dereferencing tag objects.
    /// These can be names of tags and branches.
    pub refs: Vec<RefInfo>,
}

impl Commit {
    /// Return an iterator over all reference names that point to this commit.
    pub fn ref_iter(&self) -> impl Iterator<Item = &gix::refs::FullName> + Clone {
        self.refs.iter().map(|ri| &ri.ref_name)
    }
}

/// A structure to inform about a reference which was present at a commit.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RefInfo {
    /// The name of the reference.
    pub ref_name: gix::refs::FullName,
    /// If `Some`, provide information about the worktree that checks out the reference at `ref_name`,
    /// i.e. its `HEAD` points to `ref_name` directly or indirectly due to chains of .
    ///
    /// It is `None` if no worktree needs to be updated if this reference is changed.
    pub worktree: Option<Worktree>,
}

/// Describes which worktree is checked out.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Worktree {
    /// The main worktree, i.e. the primary workspace associated with this repository, is checked out.
    ///
    /// It cannot be removed.
    Main,
    /// The identifier of the worktree, which is always `.git/worktrees/<id>`,
    /// indicating that this is a linked worktree that can be removed.
    LinkedId(BString),
}

impl Worktree {
    /// Produce a string that identifies this instance concisely, and visually distinguishable.
    /// Use `ref_name` to deduplicate the name we chose.
    pub fn debug_string(&self, ref_name: &gix::refs::FullNameRef) -> String {
        match self {
            Worktree::Main => "[🌳]".to_owned(),
            Worktree::LinkedId(id) => {
                format!(
                    "[📁{id}]",
                    id = if ref_name.shorten() != id {
                        id.as_bstr()
                    } else {
                        "".into()
                    }
                )
            }
        }
    }
}

impl RefInfo {
    /// Produce a string that identifies this instance concisely, and visually distinguishable.
    pub fn debug_string(&self) -> String {
        let ws = self
            .worktree
            .as_ref()
            .map(|ws| ws.debug_string(self.ref_name.as_ref()))
            .unwrap_or_default();
        format!("►{}{ws}", self.ref_name.shorten())
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let refs = self
            .refs
            .iter()
            .map(|ri| ri.debug_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "Commit({hash}, {flags}{refs})",
            hash = self.id.to_hex_with_len(7),
            flags = self.flags.debug_string()
        )
    }
}

bitflags! {
    /// Provide more information about a commit, as gathered during traversal.
    ///
    /// Note that unknown bits beyond this list are used to track individual goals that we want to discover.
    /// This is useful for when they are ahead of the tip that looks for them.
    /// If they are below, the goal will be propagated downward automatically.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CommitFlags: u32 {
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
            let flags = *self & Self::all();
            let extra = (self.bits() & !Self::all().bits()) >> Self::all().iter().count();
            let string = format!("{flags:?}");
            let out = &string["CommitFlags(".len()..];
            let mut out = out[..out.len() - 1]
                .to_string()
                .replace("NotInRemote", "⌂")
                .replace("InWorkspace", "🏘")
                .replace("Integrated", "✓")
                .replace(" ", "");
            if extra != 0 {
                out.push_str(&format!("|{extra:b}"));
            }
            out
        }
    }

    /// Return `true` if this flag denotes a remote commit, i.e. a commit that isn't reachable from anything
    /// but a remote tracking branch tip.
    pub fn is_remote(&self) -> bool {
        !self.contains(CommitFlags::NotInRemote)
    }
}

/// A segment of a commit graph, representing a set of commits exclusively.
#[derive(Default, Clone, Eq, PartialEq)]
pub struct Segment {
    /// An ID which can uniquely identify this segment among all segments within the graph that owned it.
    /// Note that it's not suitable to permanently identify the segment, so should not be persisted.
    pub id: SegmentIndex,
    /// A non-null number, and starting at `1`, to indicate how high up the segment is in the graph past the root nodes.
    /// Thus, higher numbers mean they are further down.
    /// If `0`, this is a root node, i.e. one without any incoming connections.
    pub generation: usize,
    /// The unambiguous or disambiguated name of the branch *or tag* at the tip of the segment, i.e. at the first commit,
    /// along with its worktree if one happens to point at it.
    ///
    /// Even though most of the time this will be local branches, when setting the entrypoint onto a commit with a *tag*,
    /// it will be used for naming it.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    /// Alternatively, the naming would have been ambiguous.
    /// Finally, this is `None` of the original name can be found searching upwards, finding exactly one
    /// named segment.
    pub ref_info: Option<RefInfo>,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// If `remote_tracking_ref_name` is set, this field is also set to make accessing the respective segment easy,
    /// avoiding a search through the entire graph.
    /// If `remote_tracking_ref_name` is `None`, and `ref_name` is a remote tracking branch, then this is set to be
    /// the segment id of the local tracking branch, effectively doubly-linking them for ease of traversal.
    /// If `ref_name` is `None` and this segment is the ancestor of a named segment that is known to a workspace,
    /// this id is pointing to that named segment to allow the reconstruction of the originally desired workspace.
    pub sibling_segment_id: Option<SegmentIndex>,
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
    /// Return the name of the reference that points to the first commit reachable through this segment.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_info.as_ref().map(|ri| ri.ref_name.as_ref())
    }
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
    pub fn non_empty_flags_of_first_commit(&self) -> Option<CommitFlags> {
        let commit = self.commits.first()?;
        (!commit.flags.is_empty()).then_some(commit.flags)
    }

    /// Return `Some(md)` if this segment contains workspace metadata, which makes it governing a workspace.
    ///
    /// Note that we assume that this kind of metadata is only assigned to portions of the graph which don't include
    /// each other *outside* of integrated portions of the graph, i.e. workspaces can't be nested.
    pub fn workspace_metadata(&self) -> Option<&but_core::ref_metadata::Workspace> {
        self.metadata.as_ref().and_then(|md| match md {
            SegmentMetadata::Workspace(md) => Some(md),
            _ => None,
        })
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let Segment {
                ref_info,
                generation,
                id,
                commits,
                remote_tracking_ref_name,
                sibling_segment_id,
                metadata,
            } = self;
            f.debug_struct("StackSegment")
                .field("id", id)
                .field("generation", generation)
                .field(
                    "ref_name",
                    &match ref_info.as_ref() {
                        None => "None".to_string(),
                        Some(name) => name.debug_string(),
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
                    "sibling_segment_id",
                    &match sibling_segment_id.as_ref() {
                        None => "None".to_string(),
                        Some(id) => id.index().to_string(),
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
        } else {
            f.debug_struct(
                "StackSegment(empty for 'dot' program to not get past 2^16 max label size)",
            )
            .finish()
        }
    }
}
