//! How callers address entries, ranges, and parent entries: the anchor, range, and
//! parent entry-selection input types.

use crate::graph_rebase::{CommitIndex, EditorIndex, RefIndex};

/// A location to position an operation against — a commit, a reference, or an index
/// already in hand. the one union input of the editor's surface: operations that anchor
/// work somewhere take `impl Into<Anchor>`, so "insert above this commit" and "insert
/// below that branch" stay one call each, while subject parameters stay typed.
///
/// Resolution happens inside the operation
/// ([`Editor::resolve_anchor`](crate::graph_rebase::Editor::resolve_anchor)): a commit id
/// or reference name that is not in the graph fails there.
#[derive(Debug, Clone)]
pub enum Anchor {
    /// A commit, found by id.
    Commit(gix::ObjectId),
    /// A reference, found by name.
    Reference(gix::refs::FullName),
    /// An entry already held — bound at capture, no resolution needed. Where the
    /// other variants mean "whatever currently holds this id or name", this means "this
    /// entry, whatever happens to it".
    Held(EditorIndex),
}

impl From<gix::ObjectId> for Anchor {
    fn from(value: gix::ObjectId) -> Self {
        Self::Commit(value)
    }
}

impl From<gix::Id<'_>> for Anchor {
    fn from(value: gix::Id<'_>) -> Self {
        Self::Commit(value.detach())
    }
}

impl From<gix::refs::FullName> for Anchor {
    fn from(value: gix::refs::FullName) -> Self {
        Self::Reference(value)
    }
}

impl From<&gix::refs::FullNameRef> for Anchor {
    fn from(value: &gix::refs::FullNameRef) -> Self {
        Self::Reference(value.to_owned())
    }
}

impl From<&gix::refs::FullName> for Anchor {
    fn from(value: &gix::refs::FullName) -> Self {
        Self::Reference(value.to_owned())
    }
}

impl From<EditorIndex> for Anchor {
    fn from(value: EditorIndex) -> Self {
        Self::Held(value)
    }
}

impl From<CommitIndex> for Anchor {
    fn from(value: CommitIndex) -> Self {
        Self::Held(value.into())
    }
}

impl From<RefIndex> for Anchor {
    fn from(value: RefIndex) -> Self {
        Self::Held(value.into())
    }
}

/// Defines the start and end of a range by pointing to its parent-most and child-most entries.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    /// The child-most entry contained within the range being defined.
    pub child: EditorIndex,
    /// The parent-most entry contained within the range being defined.
    pub parent: EditorIndex,
}

impl Range {
    /// The range that is just `entry`: child-most and parent-most bound alike.
    pub fn single(entry: impl Into<EditorIndex>) -> Self {
        let entry = entry.into();
        Self {
            child: entry,
            parent: entry,
        }
    }
}

/// Which of a range's neighboring parent entries
/// [`disconnect`](crate::graph_rebase::Editor::disconnect) severs on
/// one side — the child side or the parent side — each parent entry named by the neighbor
/// across it.
///
/// A commit owns its parent list, as in Git: it names its parents, and nothing
/// names its children. The children/parents symmetry is therefore a view over
/// asymmetric storage — cutting parents edits the range's own parent-most entry, while
/// cutting children edits each severed *child* (it loses the parent-array entry pointing
/// into the range; the editor finds those parent entries through its reverse children index).
/// Rewrites follow the same direction: an edited entry and its descendants get new ids,
/// its ancestors never do.
#[derive(Debug, Clone, Default)]
pub enum Cut {
    /// Sever every parent entry on this side.
    #[default]
    All,
    /// Sever nothing on this side.
    Nothing,
    /// Sever only the parent entries to these neighbors. May not be empty — that would be
    /// [`Cut::Nothing`] in disguise, and the operation rejects it.
    Only(Vec<Anchor>),
}

impl Cut {
    /// Sever only the parent entries to these neighbors.
    pub fn only<T: Into<Anchor>>(neighbors: impl IntoIterator<Item = T>) -> Self {
        Self::Only(neighbors.into_iter().map(Into::into).collect())
    }
}

/// What [`move_range`](crate::graph_rebase::Editor::move_range) wires onto
/// the re-inserted range on the insertion side.
#[derive(Debug, Clone, Default)]
pub enum Connect {
    /// Splice the range into the target's existing parent entries, like a linked-list insert:
    /// the target's neighbors on the insertion side re-anchor their parent entries onto the
    /// range, and the range connects to the target — inserting `X` above `T` turns
    /// `child → T` into `child → X → T`.
    #[default]
    Splice,
    /// Wire only these entries onto the range — any entries, not just the target's
    /// neighbors. May not be empty — a range wired to nothing would dangle, and the
    /// operation rejects it.
    Only(Vec<Anchor>),
}

impl Connect {
    /// Wire only these entries onto the range.
    pub fn only<T: Into<Anchor>>(entries: impl IntoIterator<Item = T>) -> Self {
        Self::Only(entries.into_iter().map(Into::into).collect())
    }
}
