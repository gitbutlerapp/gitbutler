use bitflags::bitflags;
use bstr::{BString, ByteSlice};

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
    pub fn ref_name_iter(&self) -> impl Iterator<Item = &gix::refs::FullName> + Clone {
        self.refs.iter().map(|ri| &ri.ref_name)
    }

    /// Return information about the reference that matches `name`.
    pub fn ref_by_name(&self, name: &gix::refs::FullNameRef) -> Option<&RefInfo> {
        self.refs.iter().find(|ri| ri.ref_name.as_ref() == name)
    }
}

/// A structure to inform about a reference which was present at a commit.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RefInfo {
    /// The name of the reference.
    pub ref_name: gix::refs::FullName,
    /// The peeled commit id the reference pointed to when the graph was built.
    ///
    /// This is `None` if the reference was known only by name, for example for
    /// unborn branches or synthetic segments created without a resolved ref tip.
    ///
    /// It's useful if the segment with this ref-info instance doesn't actually
    /// own a commit, and can't (always) discover it by walking past empty segments.
    /// Workspace queries use it as a fallback when resolving a segment's tip commit
    /// (e.g. [`StackSegment::tip_commit_id`](crate::workspace::StackSegment::tip_commit_id)).
    pub commit_id: Option<gix::ObjectId>,
    /// If `Some`, provide information about the worktree that checks out the reference at `ref_name`,
    /// i.e. its `HEAD` points to `ref_name` directly or indirectly due to chains of .
    ///
    /// It is `None` if no worktree needs to be updated if this reference is changed.
    pub worktree: Option<Worktree>,
}

/// Describes which kind of worktree is checked out by a [Ref](RefInfo).
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorktreeKind {
    /// The main worktree, i.e. the primary workspace associated with this repository, is checked out.
    ///
    /// It cannot be removed.
    Main,
    /// The identifier of the worktree, which is always `.git/worktrees/<id>`,
    /// indicating that this is a linked worktree that can be removed.
    LinkedId(BString),
}

/// Describes which worktree is checked out and how it relates to the repository that produced the graph.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Worktree {
    /// The kind of worktree that checks out the reference.
    pub kind: WorktreeKind,
    /// The repository that produced the graph is using this worktree.
    ///
    /// Only one worktree in a graph should have this flag set.
    pub owned_by_repo: bool,
}

impl Worktree {
    /// Produce a string that identifies this instance concisely, and visually distinguishable.
    /// `ref_name` is the name from the [`RefInfo`] that owns this worktree,
    /// used to deduplicate the name we chose.
    ///
    /// For example, `refs/heads/foo` in linked worktree id `foo` prints
    /// `foo[📁]`, not `foo[📁foo]`.
    pub fn debug_string(&self, ref_name: &gix::refs::FullNameRef) -> String {
        self.debug_string_with_graph_context(ref_name, false)
    }

    /// Like [`Self::debug_string()`], but includes graph-contextual worktree ownership markers.
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
        self.kind.debug_string(ref_name, owned_by_repo)
    }
}

impl WorktreeKind {
    fn debug_string(&self, ref_name: &gix::refs::FullNameRef, owned_by_repo: &str) -> String {
        match self {
            WorktreeKind::Main => format!("[🌳{owned_by_repo}]"),
            WorktreeKind::LinkedId(id) => {
                format!(
                    "[📁{id}{owned_by_repo}]",
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
            flags = self.flags.debug_string(None)
        )
    }
}

bitflags! {
    /// The reason a segment stops without an outgoing graph edge.
    ///
    /// Multiple flags can be present at once if multiple conditions apply to the same commit.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StopCondition: u8 {
        /// Traversal stopped before following parents due to configured traversal limits.
        ///
        /// If this was a *hard* limit, the graph may not contain all the *interesting* portions of the commit-graph,
        /// see [`hard_limit`](crate::init::Options::hard_limit)
        const Limit = 1 << 0;
        /// Traversal reached the first commit in history, which has no parents, and is an orphan.
        /// There can be more than one in one graph if unrelated histories were merged.
        const FirstCommit = 1 << 1;
        /// Traversal reached a Git shallow boundary, as is created with the shallow clone feature.
        const ShallowBoundary = 1 << 2;
    }
}

impl StopCondition {
    /// Return a concise symbolic representation of this stop condition for debug output.
    pub fn debug_string(&self, hard_limit: bool) -> String {
        let mut out = String::new();
        if self.contains(StopCondition::Limit) {
            out.push_str(if hard_limit { "❌" } else { "✂" });
        }
        if self.contains(StopCondition::FirstCommit) {
            out.push('🏁');
        }
        if self.contains(StopCondition::ShallowBoundary) {
            out.push('⛰');
        }
        out
    }

    /// Return `true` if traversal stopped because the configured traversal limit was reached.
    pub fn at_limit(&self) -> bool {
        self.contains(StopCondition::Limit)
    }

    /// Return `true` if traversal stopped due to an artificial boundary, not because history naturally ended.
    ///
    /// This also means that the traversal would have continued otherwise.
    pub fn is_unnatural(&self) -> bool {
        self.intersects(StopCondition::Limit | StopCondition::ShallowBoundary)
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
        /// The commit is listed in the repository's shallow boundary file.
        const ShallowBoundary = 1 << 3;
    }
}

impl CommitFlags {
    /// The amount of goals that were tracked, i.e. 0 if there is no goal, or N if there are N goal.
    pub fn num_goals(&self) -> usize {
        let goals = self.bits() & !Self::all().bits();
        if goals == 0 {
            0
        } else {
            (Self::all().bits().leading_zeros() - goals.leading_zeros()) as usize
        }
    }
    /// Return a less verbose debug string, with `max_goals` marking the highest amount of goals we have to display.
    pub fn debug_string(&self, max_goals: Option<usize>) -> String {
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
                .replace("ShallowBoundary", "⛰")
                .replace(" ", "");
            if extra != 0 {
                out.push_str(&format!(
                    "|{extra:>0width$b}",
                    width = max_goals.unwrap_or(0)
                ));
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

/// A run of commits owned exclusively by this segment, named by the ref at its tip (when named).
///
/// Metadata for segments, which are either dedicated branches or represent workspaces.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SegmentMetadata {
    /// Segments with this data are considered a branch in the workspace.
    Branch(but_core::ref_metadata::Branch),
    /// Segments with this data are considered the tip of the workspace.
    Workspace(but_core::ref_metadata::Workspace),
}
