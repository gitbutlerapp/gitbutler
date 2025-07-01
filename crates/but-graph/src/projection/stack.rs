use crate::{CommitFlags, Graph, SegmentIndex, SegmentMetadata};
use anyhow::{Context, bail};
use bitflags::bitflags;
use but_core::ref_metadata;
use std::fmt::Formatter;

/// A list of segments that together represent a list of dependent branches, stacked on top of each other.
#[derive(Clone)]
pub struct Stack {
    /// If there is an integration branch, we know a base commit shared with the integration branch from
    /// which we branched off.
    /// It is `None` if this is a stack derived from a branch without relation to any other branch.
    // TODO: figure out what this is used for, I have a feeling that we'd not want it anymore.
    pub base: Option<gix::ObjectId>,
    /// The branch-name denoted segments of the stack from its tip to the point of reference, typically a merge-base.
    /// This array is never empty.
    pub segments: Vec<StackSegment>,
}

impl Stack {
    /// A one-line string representing the stack itself, without its contents.
    pub fn debug_string(&self) -> String {
        let mut dbg = self
            .segments
            .first()
            .map_or_else(|| "<anon>".into(), |s| s.debug_string());
        if let Some(base) = self.base {
            dbg.push_str(" on ");
            dbg.push_str(&base.to_hex_with_len(7).to_string());
        }
        dbg.insert(0, '≡');
        dbg
    }
}

impl std::fmt::Debug for Stack {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Stack({})", self.debug_string()))
            .field("segments", &self.segments)
            .finish()
    }
}

impl From<Vec<StackSegment>> for Stack {
    fn from(segments: Vec<StackSegment>) -> Self {
        Stack {
            base: None,
            segments,
        }
    }
}

/// A typically named set of linearized commits, obtained by first-parent-only traversal.
///
/// Note that this maybe an aggregation of multiple [graph segments](crate::Segment).
#[derive(Clone)]
pub struct StackSegment {
    /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    /// Alternatively, the naming could have been ambiguous while this is the first segment in the stack.
    /// named segment.
    pub ref_name: Option<gix::refs::FullName>,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means [`commits_unique_in_remote_tracking_branch`] are possibly available.
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// An ID which uniquely identifies the [first graph segment](crate::Segment) that is contained
    /// in this instance.
    /// This is always the first id in the `commits_by_segment`.
    /// Note that it's not suitable to permanently identify the segment, so should not be persisted,
    /// and is only stable within this graph as it exists right now. Traversing the graph again will yield
    /// different IDs in an unpredictable way as the underlying commit-graph may have changed.
    /// Also, one cannot assume that one of its commits belongs to a graph segment of this ID directly,
    /// there is no 1:1 mapping.
    pub id: SegmentIndex,
    /// The portion of commits that can be reached from the tip of the *branch* downwards to the next [StackSegment],
    /// so that they are unique for this stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
    pub commits: Vec<StackCommit>,
    /// A mapping of `(segment_idx, offset)` to know which segment contributed the commits of the
    /// given range.
    /// This is useful to be able to retain the ability to associate a commit to a segment in the graph.
    pub commits_by_segment: Vec<(SegmentIndex, usize)>,
    /// Commits that are *only* reachable from the tip of the remote-tracking branch that is associated with this branch,
    /// down to the first (and possibly unrelated) non-remote commit.
    /// Note that these commits may not have an actual commit-graph connection to the local
    /// `commits` available above.
    /// Further, despite being in a simple list, their order is based on a simple topological walk, so
    /// this form doesn't imply a linear history.
    pub commits_on_remote: Vec<StackCommit>,
    /// Read-only branch metadata with additional information, or `None` if nothing was present.
    pub metadata: Option<ref_metadata::Branch>,
    /// This is `true` for exactly one segment in a workspace if the entrypoint of [the traversal](Graph::from_commit_traversal())
    /// is this segment, and the surrounding workspace is provided for context.
    /// This means one will see the entire workspace, while knowing the focus is on one specific segment.
    pub is_entrypoint: bool,
}

impl std::fmt::Debug for StackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("StackSegment({})", self.debug_string()))
            .field("commits", &self.commits)
            .field("commits_on_remote", &self.commits_on_remote)
            .finish()
    }
}

impl StackSegment {
    /// Given a list of *graph* `segments` to aggregate, produce a stack segment that is like the combination
    /// of a remote segment and a local ones, along with more detailed commits and (if possible) without
    /// anonymous portions.
    ///
    /// It's like reconstructing a first-parent traversal from the segmented graph, which splits each time there
    /// is an unambiguous ref pointing to a commit, or when it splits a segment by incoming connection.
    ///
    /// `graph` is used to look up the remote segment and find its commits.
    pub fn from_graph_segments(
        segments: &[&crate::Segment],
        graph: &Graph,
    ) -> anyhow::Result<Self> {
        let mut segments_iter = segments.iter();
        let crate::Segment {
            id,
            ref_name,
            remote_tracking_ref_name,
            commits: _,
            metadata,
        } = segments_iter
            .next()
            .context("BUG: need one or more segments")?;

        let mut commits_by_segment = Vec::new();
        for s in segments {
            let mut stack_commits = Vec::new();
            for commit in &s.commits {
                stack_commits.push(StackCommit::from_graph_commit(commit)?);
            }
            commits_by_segment.push((s.id, stack_commits));
        }
        // The last (actual) segment could be partial.
        if let Some(commits) = commits_by_segment
            .last_mut()
            .and_then(|(sidx, commits)| graph.is_early_end_of_traversal(*sidx).then_some(commits))
        {
            if let Some(commit) = commits.last_mut() {
                commit.flags |= StackCommitFlags::EarlyEnd;
            }
        }

        Ok(StackSegment {
            ref_name: ref_name.clone(),
            id: *id,
            remote_tracking_ref_name: remote_tracking_ref_name.clone(),
            commits_by_segment: {
                let mut ofs = 0;
                commits_by_segment
                    .iter()
                    .map(|(sidx, commits)| {
                        let res = (*sidx, ofs);
                        ofs += commits.len();
                        res
                    })
                    .collect()
            },
            commits: commits_by_segment
                .into_iter()
                .flat_map(|(_sid, commits)| commits)
                .collect(),
            // Will be set later once all stacks are known.
            commits_on_remote: Vec::new(),
            metadata: metadata
                .as_ref()
                .map(|md| match md {
                    SegmentMetadata::Branch(md) => Ok(md.clone()),
                    SegmentMetadata::Workspace(_) => {
                        bail!("BUG: Should always stop stacks at workspaces")
                    }
                })
                .transpose()?,
            is_entrypoint: false, /* to be set later */
        })
    }

    /// Digest as much as possible into a single line.
    pub fn debug_string(&self) -> String {
        let num_local_commits = if self.remote_tracking_ref_name.is_some() {
            self.commits
                .iter()
                .filter(|c| !c.flags.contains(StackCommitFlags::ReachableByRemote))
                .count()
        } else {
            0
        };
        format!(
            "{ep}{meta}:{id}:{name}{remote}{local_commits}{remote_commits}",
            ep = if self.is_entrypoint { "👉" } else { "" },
            meta = if self.metadata.is_some() { "📙" } else { "" },
            id = self.id.index(),
            name = self
                .ref_name
                .as_ref()
                .map(Graph::ref_debug_string)
                .unwrap_or_else(|| "<anon>".into()),
            remote = if let Some(remote_ref_name) = self.remote_tracking_ref_name.as_ref() {
                format!(
                    " <> {remote_name}",
                    remote_name = Graph::ref_debug_string(remote_ref_name)
                )
            } else {
                "".into()
            },
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

/// A combination of [Commits](crate::Commit).
#[derive(Clone, Eq, PartialEq)]
pub struct StackCommit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// Additional properties to help classify this commit.
    pub flags: StackCommitFlags,
    /// The references pointing to this commit, even after dereferencing tag objects.
    /// These can be names of tags and branches.
    pub refs: Vec<gix::refs::FullName>,
}

impl StackCommit {
    /// Collect additional information on `commit` using `repo`.
    pub fn from_graph_commit(commit: &crate::Commit) -> anyhow::Result<Self> {
        Ok(StackCommit {
            id: commit.id,
            parent_ids: commit.parent_ids.clone(),
            flags: StackCommitFlags::from(commit.flags),
            refs: commit.refs.clone(),
        })
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
                        .map(|rn| format!("►{}", { Graph::ref_debug_string(rn) }))
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
        /// from a workspace.
        const InWorkspace = 1 << 1;
        /// The commit is reachable from either the target branch (usually `refs/remotes/origin/main`).
        /// Note that when multiple workspaces are included in the traversal, this flag is set by
        /// any of many target branches.
        const Integrated = 1 << 2;
        // --- END OVERLAP ---

        /// This commit was pushed to *our* remote and thus could have been observed by others.
        /// This definitely means manipulation will require a force-push afterward.
        /// Implies `ReachableByRemote`, which is then also set for convenience.
        const ReachableByMatchingRemote = 1 << 3;
        /// Whether the commit is in a conflicted state, a GitButler concept.
        /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
        /// Conflicts are resolved via the Edit Mode mechanism.
        ///
        /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
        const HasConflicts = 1 << 4;
        /// The commit will appear 'snipped off' as it has parents, but the traversal stopped there due to hitting limits
        /// or because the commits weren't interesting.
        const EarlyEnd = 1 << 5;
    }
}

impl StackCommitFlags {
    /// Return a less verbose debug string.
    ///
    /// Note that this only displays flags that are not used when displaying [the whole commit](StackCommit::debug_string()).
    pub fn debug_string(&self) -> String {
        let flags = *self & (Self::InWorkspace | Self::Integrated);
        if flags.is_empty() {
            "".into()
        } else {
            let string = format!("{:?}", flags);
            let out = &string["StackCommitFlags(".len()..];
            out[..out.len() - 1]
                .to_string()
                .replace("InWorkspace", "🏘️")
                .replace("Integrated", "✓")
                .replace(" ", "")
        }
    }
}

/// Convert only matching bits
impl From<CommitFlags> for StackCommitFlags {
    fn from(value: CommitFlags) -> Self {
        StackCommitFlags::from_bits_retain(
            (value & (CommitFlags::Integrated | CommitFlags::InWorkspace)).bits() as u8,
        )
    }
}
