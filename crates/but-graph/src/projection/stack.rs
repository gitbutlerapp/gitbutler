//! The shapes the projection emits: [`Stack`]s of [`StackSegment`]s holding
//! [`StackCommit`]s — a deliberately simplified, display-ready view that degenerates
//! graph detail while keeping ids that link back to it.

use std::fmt::Formatter;

use bitflags::bitflags;
use but_core::{ref_metadata, ref_metadata::StackId};

use crate::CommitFlags;

/// A list of segments that together represent a list of dependent branches, stacked on top of each other.
#[derive(Clone)]
pub struct Stack {
    /// If the stack belongs to a managed workspace, the `id` will be set and persist.
    /// Otherwise, it is `None`.
    pub id: Option<StackId>,
    /// The branch-name denoted segments of the stack from its tip to the point of reference, typically a merge-base.
    /// This array is never empty.
    pub segments: Vec<StackSegment>,
}

/// Query
impl Stack {
    /// Return the name of the first segment of the stack.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.segments.first().and_then(|s| s.ref_name())
    }

    /// Return the first commit of the first non-empty segment, or `None` this stack is completely empty, or has only empty segments.
    pub fn tip_skip_empty(&self) -> Option<gix::ObjectId> {
        self.segments.iter().find_map(|s| {
            if s.commits.is_empty() {
                return None;
            }
            s.commits.first().map(|c| c.id)
        })
    }

    /// The [base](StackSegment::base) of the last of our segments.
    pub fn base(&self) -> Option<gix::ObjectId> {
        self.segments.last().and_then(|s| s.base)
    }
}

impl Stack {
    /// Like [`Self::from_base_and_segments`], but without a graph to recompute the
    /// last base from — the arena-derived collection threads bases purely from the
    /// segments themselves.
    pub(crate) fn from_base_and_segments_raw(
        mut segments: Vec<StackSegment>,
        id: Option<StackId>,
    ) -> Self {
        let mut iter = segments.iter_mut();
        let mut cur = iter.next();
        while let Some((a, b)) = cur.zip(iter.next()) {
            a.base = b.commits.first().map(|c| c.id);
            cur = Some(b);
        }
        Stack { id, segments }
    }

    /// Recompute the last segment's base from the stack's bottom commit's first parent.
    ///
    /// Needed after the stack's bottom is truncated (e.g. by integrated-trunk pruning),
    /// which leaves the collection-time base pointing below territory no longer in the
    /// stack.
    pub(crate) fn recompute_last_segment_base_from(&mut self, cg: &crate::CommitGraph) {
        let bottom = self
            .segments
            .iter()
            .rev()
            .find_map(|s| s.commits.last().map(|c| c.id));
        let Some(last_segment) = self.segments.last_mut() else {
            return;
        };
        let Some(bottom) = bottom else {
            return;
        };
        last_segment.base = cg.all_parent_ids(bottom).first().copied();
    }
}

impl Stack {
    /// A one-line string representing the stack itself, without its contents.
    ///
    /// Use `id_override` to have it use this (usually controlled) id instead of what otherwise
    /// would be a generated one.
    pub fn debug_string(&self, id_override: Option<StackId>) -> String {
        let mut dbg = self
            .segments
            .first()
            .map_or_else(|| "<anon>".into(), |s| s.debug_string());
        self.push_debug_suffix(&mut dbg, id_override);
        dbg
    }

    /// Like [`Self::debug_string()`], but includes graph-contextual worktree ownership markers.
    pub fn debug_string_with_graph_context(
        &self,
        ws: &crate::Workspace,
        id_override: Option<StackId>,
    ) -> String {
        let mut dbg = self.segments.first().map_or_else(
            || "<anon>".into(),
            |s| s.debug_string_with_graph_context(ws),
        );
        self.push_debug_suffix(&mut dbg, id_override);
        dbg
    }

    fn push_debug_suffix(&self, dbg: &mut String, id_override: Option<StackId>) {
        if let Some(base) = self.base() {
            dbg.push_str(" on ");
            dbg.push_str(&base.to_hex_with_len(7).to_string());
        }
        dbg.insert(0, '≡');
        if let Some(id) = id_override.or(self.id) {
            let id_string = id.to_string().replace("0", "").replace("-", "");
            dbg.push_str(&format!(
                " {{{}}}",
                if id_string.is_empty() {
                    "0".into()
                } else {
                    id_string
                }
            ));
        }
    }
}

impl std::fmt::Debug for Stack {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct(&format!("Stack({})", self.debug_string(None)));
        s.field("segments", &self.segments);
        if let Some(stack_id) = self.id {
            s.field("id", &stack_id);
        }
        s.finish()
    }
}

/// A typically named set of linearized commits, obtained by first-parent-only traversal.
///
/// Note that this maybe an aggregation of multiple `graph segments`.
///
/// ### WARNING
///
/// As it stands, we may 'doctor' the `ref_name`, `remote_tracking_ref_name` and `metadata` *if* `commits_outside` is not
/// `None`. This is to help with visualisation, but makes this data much less usable in algorithms, at least if
/// these fields are significant.
#[derive(Clone, Default)]
pub struct StackSegment {
    /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit,
    /// along with its worktree information.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    /// Alternatively, the naming could have been ambiguous while this is the first segment in the stack.
    /// named segment.
    pub ref_info: Option<crate::RefInfo>,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means [`commits_outside`](Self::commits_outside) are possibly available.
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// The segment is anonymous in the graph; the name, metadata, and remote shown here were
    /// projected from its out-of-workspace sibling (an advanced branch whose tip left the
    /// workspace), to allow reconstructing the originally desired workspace.
    pub name_projected_from_outside: bool,
    /// The portion of commits that can be reached from the tip of the *branch* downwards to the next [StackSegment],
    /// so that they are unique for this stack segment and not included in any other stack or stack segment.
    /// The walk is performed **along the first parent only**.
    ///
    /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
    pub commits: Vec<StackCommit>,
    /// All commits *that are not workspace commits* reachable by (and including commits in) this segment.
    /// The list was created by walking all parents, not only the first parent.
    /// When set, the `ref_name`, `metadata` and `remote_tracking_ref_name` were copied from
    /// the out-of-workspace sibling segment that owns these commits.
    pub commits_outside: Option<Vec<StackCommit>>,
    /// This is always the `first()` commit in `commits` of the next stacksegment, or the first commit of
    /// the first ancestor segment.
    /// It can be imagined as the base upon which the segment is resting, or the connection point to the rest
    /// of the commit-graph along the first parent.
    /// It is `None` if the stack segment contains the first commit in the history, an orphan without ancestry,
    /// or if the history traversal was stopped early.
    pub base: Option<gix::ObjectId>,
    /// Commits that are *only* reachable from the tip of the remote-tracking branch that is associated with this branch,
    /// down to the first (and possibly unrelated) non-remote commit.
    /// Note that these commits may not have an actual commit-graph connection to the local
    /// `commits` available above.
    /// Further, despite being in a simple list, their order is based on a simple topological walk, so
    /// this form doesn't imply a linear history.
    pub commits_on_remote: Vec<StackCommit>,
    /// Read-only branch metadata with additional information, or `None` if nothing was present.
    pub metadata: Option<ref_metadata::Branch>,
    /// This is `true` for exactly one segment in a workspace if the entrypoint of `the traversal`)
    /// is this segment, and the surrounding workspace is provided for context.
    /// This means one will see the entire workspace, while knowing the focus is on one specific segment.
    pub is_entrypoint: bool,
}

/// Access
impl StackSegment {
    /// An empty segment for tests outside this crate that assemble projections by hand.
    #[doc(hidden)]
    pub fn default_for_testing() -> Self {
        Self::default()
    }
}

impl StackSegment {
    /// Return the top-most commit id, or `None` if this segment is empty.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits.first().map(|c| c.id)
    }

    /// Return the name of the stack segment, if present.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_info.as_ref().map(|ri| ri.ref_name.as_ref())
    }
}

impl std::fmt::Debug for StackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("StackSegment({})", self.debug_string()))
            .field("commits", &self.commits)
            .field("commits_on_remote", &self.commits_on_remote)
            .field("commits_outside", &self.commits_outside)
            .finish()
    }
}

impl StackSegment {
    /// Digest as much as possible into a single line.
    pub fn debug_string(&self) -> String {
        // Segment indices are ephemeral; keep them out of snapshot output.
        self.debug_string_with_ref_name_remote(crate::debug::ref_and_remote_debug_string(
            self.ref_info.as_ref(),
            self.remote_tracking_ref_name.as_ref(),
            None,
            None,
        ))
    }

    /// Like [`Self::debug_string()`], but includes graph-contextual worktree ownership markers.
    pub fn debug_string_with_graph_context(&self, ws: &crate::Workspace) -> String {
        self.debug_string_with_ref_name_remote(crate::debug::ref_and_remote_debug_string_inner(
            self.ref_info.as_ref(),
            self.remote_tracking_ref_name.as_ref(),
            None,
            None,
            ws.has_multiple_worktrees,
        ))
    }

    fn debug_string_with_ref_name_remote(&self, ref_name_remote: String) -> String {
        let num_local_commits = if self.remote_tracking_ref_name.is_some() {
            self.commits
                .iter()
                .filter(|c| {
                    !c.flags.intersects(
                        StackCommitFlags::ReachableByRemote | StackCommitFlags::Integrated,
                    )
                })
                .count()
        } else {
            0
        };
        format!(
            "{ep}{meta}:{ref_name_remote}{local_commits}{remote_commits}",
            ep = if self.is_entrypoint { "👉" } else { "" },
            meta = if self.metadata.is_some() { "📙" } else { "" },
            local_commits = if num_local_commits == 0 {
                "".into()
            } else {
                format!("⇡{num_local_commits}")
            },
            remote_commits = if self.commits_on_remote.is_empty() {
                "".into()
            } else {
                format!("⇣{}", self.commits_on_remote.len())
            }
        )
    }
}

/// The same as [Commits](crate::Commit), but with [StackCommitFlags].
#[derive(Clone, Eq, PartialEq)]
pub struct StackCommit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// Additional properties to help classify this commit.
    pub flags: StackCommitFlags,
    /// The references pointing to this commit, even after dereferencing tag objects, along with their worktree information.
    /// These can be names of tags and branches.
    pub refs: Vec<crate::RefInfo>,
}

/// Utilities
impl StackCommit {
    /// Return an iterator over all reference names that point to this commit.
    pub fn ref_iter(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> + Clone {
        self.refs.iter().map(|ri| ri.ref_name.as_ref())
    }

    /// Collect additional information on `commit` using `repo`.
    pub(crate) fn from_graph_commit(commit: &crate::Commit) -> Self {
        StackCommit {
            id: commit.id,
            parent_ids: commit.parent_ids.clone(),
            flags: StackCommitFlags::from(commit.flags),
            refs: commit.refs.clone(),
        }
    }

    /// Digest this commits down into a single-line debug string.
    pub fn debug_string(&self, flags: StackCommitDebugFlags) -> String {
        use StackCommitDebugFlags as F;
        format!(
            "{end}{kind}{hex}{flags}{refs}",
            end = if self.flags.contains(StackCommitFlags::EarlyEnd) {
                if flags.contains(F::HardLimitReached) {
                    "❌"
                } else {
                    "✂️"
                }
            } else {
                ""
            },
            kind = if flags.contains(F::RemoteOnly) {
                "🟣"
            } else if self
                .flags
                .contains(StackCommitFlags::ReachableByMatchingRemote)
            {
                "❄️"
            } else if self.flags.contains(StackCommitFlags::ReachableByRemote) {
                "❄"
            } else {
                "·"
            },
            flags = {
                let flags = self.flags.debug_string();
                if !flags.is_empty() {
                    format!(" ({flags})")
                } else {
                    "".to_string()
                }
            },
            hex = self.id.to_hex_with_len(7),
            refs = if self.refs.is_empty() {
                "".to_string()
            } else {
                format!(
                    " {}",
                    self.refs
                        .iter()
                        .map(|ri| format!("►{}", {
                            crate::debug::ref_debug_string(
                                ri.ref_name.as_ref(),
                                ri.worktree.as_ref(),
                            )
                        }))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    }
}

impl std::fmt::Debug for StackCommit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_string(Default::default()).fmt(f)
    }
}

bitflags! {
    /// Define how to debug-print the commit.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StackCommitDebugFlags: u8 {
        /// Is a designated remote commit, i.e. available only from a remote tracking branch.
        const RemoteOnly = 1 << 0;
        /// The hard limit was reached at which the traversal stopped unconditionally. Needs `EarlyEnd`
        /// to be effective.
        const HardLimitReached = 1 << 1;
    }
}

bitflags! {
    /// Provide more information about a commit, as gathered during traversal and as member of the stack.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StackCommitFlags: u8 {
        /// This commit was pushed to a remote and thus could have been observed by others.
        /// Note that this remote isn't directly related to the owning segment, it may be another remote.
        ///
        /// These commits should be considered frozen and not be manipulated casually.
        const ReachableByRemote = 1 << 0;

        // --- MUST OVERLAP WITH CommitFlags and keep order ---
        /// Following the graph upward will lead to at least one tip that is a workspace.
        ///
        /// Note that if this flag isn't present, this means the commit isn't reachable
        /// from a workspace, and thus was outside the workspace due to an advanced workspace head.
        /// This happens if a reference that is in the workspace is checked out directly and committed into.
        const InWorkspace = 1 << 1;
        /// The commit is reachable from either the target branch (usually `refs/remotes/origin/main`).
        /// Note that when multiple workspaces are included in the traversal, this flag is set by
        /// any of many target branches.
        const Integrated = 1 << 2;
        /// The commit is listed in the repository's shallow boundary file, it's 'grafted'.
        const ShallowBoundary = 1 << 3;
        // --- END OVERLAP ---

        /// This commit was pushed to *our* remote and thus could have been observed by others.
        /// This definitely means manipulation will require a force-push afterward.
        /// Implies `ReachableByRemote`, which is then also set for convenience.
        const ReachableByMatchingRemote = 1 << 4;
        /// Whether the commit is in a conflicted state, a GitButler concept.
        /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
        /// Conflicts are resolved via the Edit Mode mechanism.
        ///
        /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
        const HasConflicts = 1 << 5;
        /// The commit will appear 'snipped off' as it has parents, but the traversal stopped there due to hitting limits
        /// or because the commits weren't interesting.
        const EarlyEnd = 1 << 6;
    }
}

impl StackCommitFlags {
    /// Return a less verbose debug string.
    ///
    /// Note that this only displays flags that are not used when displaying [the whole commit](StackCommit::debug_string()).
    pub fn debug_string(&self) -> String {
        let flags = *self & (Self::InWorkspace | Self::Integrated | Self::ShallowBoundary);
        if flags.is_empty() {
            "".into()
        } else {
            let string = format!("{flags:?}");
            let out = &string["StackCommitFlags(".len()..];
            out[..out.len() - 1]
                .to_string()
                .replace("InWorkspace", "🏘️")
                .replace("Integrated", "✓")
                .replace("ShallowBoundary", "⛰")
                .replace(" ", "")
        }
    }
}

/// Convert only matching bits
impl From<CommitFlags> for StackCommitFlags {
    fn from(value: CommitFlags) -> Self {
        StackCommitFlags::from_bits_retain(
            (value
                & (CommitFlags::Integrated
                    | CommitFlags::InWorkspace
                    | CommitFlags::ShallowBoundary))
                .bits() as u8,
        )
    }
}
