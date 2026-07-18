use bitflags::bitflags;
use bstr::{BString, ByteSlice};

/// Information about a reference placed in the graph.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RefInfo {
    /// The full reference name.
    pub ref_name: gix::refs::FullName,
    /// The peeled commit ID, or `None` for an unborn reference.
    pub commit_id: Option<gix::ObjectId>,
    /// The worktree that checks out this reference, if any.
    pub worktree: Option<Worktree>,
}

/// The kind of worktree checking out a reference.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorktreeKind {
    /// The repository's main worktree.
    Main,
    /// A linked worktree identified by its administrative directory name.
    LinkedId(BString),
}

/// A worktree associated with a reference.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Worktree {
    /// The kind of worktree.
    pub kind: WorktreeKind,
    /// Whether the repository that produced the graph uses this worktree.
    pub owned_by_repo: bool,
}

impl Worktree {
    /// Return a concise representation relative to `ref_name`.
    pub fn debug_string(&self, ref_name: &gix::refs::FullNameRef) -> String {
        self.debug_string_with_graph_context(ref_name, false)
    }

    /// Return a concise representation, optionally marking graph ownership.
    pub fn debug_string_with_graph_context(
        &self,
        ref_name: &gix::refs::FullNameRef,
        show_owned_by_repo: bool,
    ) -> String {
        let owned_by_repo = if show_owned_by_repo && self.owned_by_repo {
            "@repo"
        } else {
            ""
        };
        match &self.kind {
            WorktreeKind::Main => format!("[🌳{owned_by_repo}]"),
            WorktreeKind::LinkedId(id) => format!(
                "[📁{}{owned_by_repo}]",
                if ref_name.shorten() == id {
                    "".into()
                } else {
                    id.as_bstr()
                }
            ),
        }
    }
}

impl RefInfo {
    /// Return a concise representation of this reference and its worktree.
    pub fn debug_string(&self) -> String {
        let worktree = self
            .worktree
            .as_ref()
            .map(|worktree| worktree.debug_string(self.ref_name.as_ref()))
            .unwrap_or_default();
        format!("►{}{worktree}", self.ref_name.shorten())
    }
}

/// Why a commit parent is represented by a boundary node.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum BoundaryKind {
    /// All traversal tips converged above this parent.
    Convergence,
    /// Git declares the child commit to be a shallow boundary.
    Shallow,
}

impl BoundaryKind {
    /// Return a concise symbolic representation.
    pub fn debug_string(&self) -> &'static str {
        match self {
            BoundaryKind::Convergence => "✂",
            BoundaryKind::Shallow => "⛰",
        }
    }
}

bitflags! {
    /// Commit annotations derived after traversal.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CommitFlags: u32 {
        /// The commit is reachable from the traversal entrypoint.
        const EntrypointSide = 1 << 0;
        /// The commit is reachable from a configured target.
        const TargetSide = 1 << 1;
    }
}

impl CommitFlags {
    /// Return a concise symbolic representation.
    pub fn debug_string(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let string = format!("{self:?}");
        let out = &string["CommitFlags(".len()..string.len() - 1];
        out.replace("EntrypointSide", "→")
            .replace("TargetSide", "←")
            .replace(' ', "")
    }
}

/// Metadata attached to a reference node.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ReferenceMetadata {
    /// Branch metadata.
    Branch(but_core::ref_metadata::Branch),
    /// Workspace metadata.
    Workspace(but_core::ref_metadata::Workspace),
}
