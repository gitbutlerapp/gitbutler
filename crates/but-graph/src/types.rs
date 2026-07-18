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

bitflags! {
    /// Why traversal omitted a commit's parents.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StopCondition: u8 {
        /// Traversal stopped at a configured limit.
        const Limit = 1 << 0;
        /// Traversal reached a root commit.
        const FirstCommit = 1 << 1;
        /// Traversal reached a Git shallow boundary.
        const ShallowBoundary = 1 << 2;
    }
}

impl StopCondition {
    /// Return a concise symbolic representation.
    pub fn debug_string(&self, hard_limit: bool) -> String {
        let mut out = String::new();
        if self.contains(Self::Limit) {
            out.push_str(if hard_limit { "❌" } else { "✂" });
        }
        if self.contains(Self::FirstCommit) {
            out.push('🏁');
        }
        if self.contains(Self::ShallowBoundary) {
            out.push('⛰');
        }
        out
    }
}

bitflags! {
    /// Commit annotations gathered during traversal.
    ///
    /// Bits above the declared flags are used transiently to track traversal goals.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CommitFlags: u32 {
        /// The commit is reachable from a non-remote tip.
        const NotInRemote = 1 << 0;
        /// The commit is reachable from a workspace.
        const InWorkspace = 1 << 1;
        /// The commit is reachable from a target branch.
        const Integrated = 1 << 2;
        /// The commit is listed in the repository's shallow boundary file.
        const ShallowBoundary = 1 << 3;
    }
}

impl CommitFlags {
    /// Return a concise symbolic representation, including transient goal bits.
    pub fn debug_string(&self, max_goals: Option<usize>) -> String {
        if self.is_empty() {
            return String::new();
        }
        let flags = *self & Self::all();
        let extra = (self.bits() & !Self::all().bits()) >> Self::all().iter().count();
        let string = format!("{flags:?}");
        let out = &string["CommitFlags(".len()..string.len() - 1];
        let mut out = out
            .replace("NotInRemote", "⌂")
            .replace("InWorkspace", "🏘")
            .replace("Integrated", "✓")
            .replace("ShallowBoundary", "⛰")
            .replace(' ', "");
        if extra != 0 {
            out.push_str(&format!(
                "|{extra:>0width$b}",
                width = max_goals.unwrap_or(0)
            ));
        }
        out
    }

    /// Return whether this commit is reachable only from remote tips.
    pub fn is_remote(&self) -> bool {
        !self.contains(Self::NotInRemote)
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
