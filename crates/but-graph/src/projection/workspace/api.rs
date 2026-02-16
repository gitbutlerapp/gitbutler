use std::borrow::Cow;

use anyhow::Context;
use bstr::BStr;
use but_core::{RefMetadata, extract_remote_name_and_short_name, ref_metadata::StackId};
use petgraph::Direction;
use tracing::instrument;

use crate::{
    CommitFlags, CommitIndex, Graph, Segment, SegmentIndex,
    projection::{
        Stack, StackCommit, StackSegment, TargetRef, Workspace, WorkspaceKind,
        workspace::find_segment_owner_indexes_by_refname,
    },
    segment,
};

/// A utility type to represent `(stack_idx, segment_idx, commit_idx)`.
pub type CommitOwnerIndexes = (usize, usize, CommitIndex);

/// Lifecycle
impl Workspace {
    /// Redo the graph traversal with the same settings as before, but use the latest
    /// data from `repo` and `meta` to do it.
    /// This is useful to make this instance represent changes to `repo` or `meta`.
    #[instrument(name = "Workspace::refresh_from_head", level = "debug", skip_all, err(Debug))]
    pub fn refresh_from_head(&mut self, repo: &gix::Repository, meta: &impl RefMetadata) -> anyhow::Result<()> {
        let graph = Graph::from_head(repo, meta, self.graph.options.clone())?;
        *self = graph.into_workspace()?;
        Ok(())
    }
}

/// Query
impl Workspace {
    /// Return `true` if the workspace has workspace metadata associated with it.
    /// This is relevant when creating references for example.
    pub fn has_metadata(&self) -> bool {
        self.metadata.is_some()
    }

    /// Return the name of the workspace reference by looking our segment up in `graph`.
    /// Note that for managed workspaces, this can be retrieved via [`WorkspaceKind::Managed`].
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.graph[self.id].ref_name()
    }

    /// Like [Self::ref_name()], but returns reference and worktree information instead.
    pub fn ref_info(&self) -> Option<&crate::RefInfo> {
        self.graph[self.id].ref_info.as_ref()
    }

    /// Like [`Self::ref_name()`], but return a generic `<anonymous>` name for unnamed workspaces.
    pub fn ref_name_display(&self) -> &BStr {
        self.ref_name().map_or("<anonymous>".into(), |rn| rn.as_bstr())
    }
}

/// Utilities
impl Workspace {
    /// Return the name of the remote most closely associated with this workspace.
    /// In order, we try:
    /// - The remote name of the [Self::target_ref].
    /// - The remote name configured in [workspace metadata](Self::metadata).
    ///
    /// The caller *may* consider falling back to [`gix::Repository::remote_default_name()`],
    /// but beware that one should handle ambiguity if there are more than one remotes.
    pub fn remote_name(&self) -> Option<String> {
        if let Some(tr) = self.target_ref.as_ref() {
            // TODO: should we rather get remote configuration from the repository?
            let remote_names = self
                .graph
                .symbolic_remote_names
                .iter()
                .map(|name| Cow::Borrowed(name.as_str().into()))
                .collect();
            extract_remote_name_and_short_name(tr.ref_name.as_ref(), &remote_names).map(|(remote_name, _)| remote_name)
        } else if let Some(md) = self.metadata.as_ref() {
            md.push_remote.clone()
        } else {
            None
        }
    }

    /// Return the `(merge-base, target-commit-id)` of the merge-base between the `commit_to_merge`
    /// and either the [target-branch](Self::target_ref), the [extra-target](Self::extra_target)
    /// or the [target-commit](Self::target_commit), depending on which is set and encountered
    /// in this order.
    /// Return `None` when none of these is set, or if there was no merge-base.
    ///
    /// Use this to get the merge-base for test-merges between `commit_to_merge` and the target,
    /// whose commit is also returned as `target-commit-id`.
    pub fn merge_base_with_target_branch(
        &self,
        commit_to_merge: impl Into<gix::ObjectId>,
    ) -> Option<(gix::ObjectId, gix::ObjectId)> {
        let commit_to_merge = commit_to_merge.into();
        let commit_segment_index = self.graph.node_weights().find_map(|s| {
            s.commits
                .first()
                .is_some_and(|c| c.id == commit_to_merge)
                .then_some(s.id)
        })?;

        let target_segment_index = self
            .target_ref
            .as_ref()
            .map(|t| t.segment_index)
            .or(self.target_commit.as_ref().map(|t| t.segment_index))
            .or(self.extra_target)?;

        let merge_base_segment_index = self
            .graph
            .find_git_merge_base(commit_segment_index, target_segment_index)?;

        self.graph
            .tip_skip_empty(merge_base_segment_index)
            .map(|c| c.id)
            .zip(self.graph.tip_skip_empty(target_segment_index).map(|c| c.id))
    }

    /// Return `true` if the workspace itself is where `HEAD` is pointing to.
    /// If `false`, one of the stack-segments is checked out instead.
    pub fn is_entrypoint(&self) -> bool {
        self.stacks.iter().all(|s| s.segments.iter().all(|s| !s.is_entrypoint))
    }

    /// Return an iterator over all commits in the workspace,
    /// i.e. all commits in all segments in all stacks.
    ///
    /// This doesn't include the workspace commit.
    pub fn commits(&self) -> impl Iterator<Item = &StackCommit> + '_ {
        self.stacks
            .iter()
            .flat_map(|s| s.segments.iter())
            .flat_map(|s| s.commits.iter())
    }

    /// Return `true` if the branch with `name` is the workspace target or the targets local tracking branch.
    pub fn is_branch_the_target_or_its_local_tracking_branch(&self, name: &gix::refs::FullNameRef) -> bool {
        let Some(t) = self.target_ref.as_ref() else {
            return false;
        };

        t.ref_name.as_ref() == name
            || self
                .graph
                .lookup_sibling_segment(t.segment_index)
                .and_then(|local_tracking_segment| local_tracking_segment.ref_name())
                .is_some_and(|local_tracking_ref| local_tracking_ref == name)
    }

    /// Return the `commit` at the tip of the workspace itself, and do so by following empty segments along the
    /// first parent until the first commit is found.
    /// This importantly is different from the [`Graph::lookup_entrypoint()`] `commit`, as the entrypoint could be anywhere
    /// inside the workspace as well.
    ///
    /// Note that this commit could also be the base of the workspace, particularly if there is no commits in the workspace.
    pub fn tip_commit(&self) -> Option<&segment::Commit> {
        self.graph.tip_skip_empty(self.id)
    }

    /// Lookup a triple obtained by [`Self::find_owner_indexes_by_commit_id()`] or panic.
    pub fn lookup_commit(&self, (stack_idx, seg_idx, cidx): CommitOwnerIndexes) -> &StackCommit {
        &self.stacks[stack_idx].segments[seg_idx].commits[cidx]
    }

    /// Find a stack with the given `id` or error.
    pub fn try_find_stack_by_id(&self, id: impl Into<Option<StackId>>) -> anyhow::Result<&Stack> {
        let id = id.into();
        self.find_stack_by_id(id)
            .with_context(|| format!("Couldn't find stack with id {id:?} in workspace"))
    }

    /// Find a stack with the given `id`.
    pub fn find_stack_by_id(&self, id: impl Into<Option<StackId>>) -> Option<&Stack> {
        let id = id.into();
        self.stacks.iter().find(|s| s.id == id)
    }

    /// Try to find the `(stack_idx, segment_idx, commit_idx)` to be able to access the commit with `oid` in this workspace
    /// as `ws.stacks[stack_idx].segments[segment_idx].commits[commit_idx]`.
    pub fn find_owner_indexes_by_commit_id(&self, oid: impl Into<gix::ObjectId>) -> Option<CommitOwnerIndexes> {
        let oid = oid.into();
        self.stacks.iter().enumerate().find_map(|(stack_idx, stack)| {
            stack.segments.iter().enumerate().find_map(|(seg_idx, seg)| {
                seg.commits
                    .iter()
                    .enumerate()
                    .find_map(|(cidx, c)| (c.id == oid).then_some((stack_idx, seg_idx, cidx)))
            })
        })
    }

    /// Like [`Self::find_owner_indexes_by_commit_id()`], but returns an error if the commit can't be found.
    pub fn try_find_owner_indexes_by_commit_id(
        &self,
        oid: impl Into<gix::ObjectId>,
    ) -> anyhow::Result<CommitOwnerIndexes> {
        let oid = oid.into();
        self.find_owner_indexes_by_commit_id(oid)
            .with_context(|| format!("Commit {oid} isn't part of the workspace"))
    }

    /// Try to find the `(stack_idx, segment_idx)` to be able to access the named segment going by `name`.
    /// Access the segment as `ws.stacks[stack_idx].segments[segment_idx]`
    pub fn find_segment_owner_indexes_by_refname(&self, ref_name: &gix::refs::FullNameRef) -> Option<(usize, usize)> {
        find_segment_owner_indexes_by_refname(&self.stacks, ref_name)
    }

    /// Like [`Self::find_segment_owner_indexes_by_refname`], but fails with an error.
    pub fn try_find_segment_owner_indexes_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<(usize, usize)> {
        self.find_segment_owner_indexes_by_refname(name).with_context(|| {
            format!(
                "Couldn't find any stack that contained the branch named '{}'",
                name.shorten()
            )
        })
    }

    /// Return `true` if `name` is contained in the workspace as segment.
    pub fn refname_is_segment(&self, name: &gix::refs::FullNameRef) -> bool {
        self.find_segment_and_stack_by_refname(name).is_some()
    }

    /// Return `true` if `name` is in the ancestry of the workspace entrypoint, and is IN the workspace as well.
    pub fn is_reachable_from_entrypoint(&self, name: &gix::refs::FullNameRef) -> bool {
        if self.ref_name().filter(|_| self.is_entrypoint()) == Some(name) {
            return true;
        }
        if self.is_entrypoint() {
            self.refname_is_segment(name)
        } else {
            let Some((entrypoint_stack, entrypoint_segment_idx)) = self.stacks.iter().find_map(|stack| {
                stack
                    .segments
                    .iter()
                    .enumerate()
                    .find_map(|(idx, segment)| segment.is_entrypoint.then_some((stack, idx)))
            }) else {
                return false;
            };
            entrypoint_stack
                .segments
                .get(entrypoint_segment_idx..)
                .into_iter()
                .any(|segments| segments.iter().any(|s| s.ref_name().is_some_and(|rn| rn == name)))
        }
    }

    /// Try to find `name` in any named [`StackSegment`] and return it along with the stack containing it.
    pub fn find_segment_and_stack_by_refname(&self, name: &gix::refs::FullNameRef) -> Option<(&Stack, &StackSegment)> {
        self.stacks.iter().find_map(|stack| {
            stack
                .segments
                .iter()
                .find_map(|seg| seg.ref_name().is_some_and(|rn| rn == name).then_some((stack, seg)))
        })
    }

    /// Like [`Self::find_segment_and_stack_by_refname`], but fails with an error.
    pub fn try_find_segment_and_stack_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<(&Stack, &StackSegment)> {
        self.find_segment_and_stack_by_refname(name).with_context(|| {
            format!(
                "Couldn't find any stack that contained the branch named '{}'",
                name.shorten()
            )
        })
    }
}

/// Debugging
impl Workspace {
    /// Produce a distinct and compressed debug string to show at a glance what the workspace is about.
    pub fn debug_string(&self) -> String {
        let graph = &self.graph;
        let (name, sign) = match &self.kind {
            WorkspaceKind::Managed { ref_info } => (
                Graph::ref_debug_string(ref_info.ref_name.as_ref(), ref_info.worktree.as_ref()),
                "🏘️",
            ),
            WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => (
                Graph::ref_debug_string(ref_info.ref_name.as_ref(), ref_info.worktree.as_ref()),
                "🏘️⚠️",
            ),
            WorkspaceKind::AdHoc => (
                graph[self.id].ref_info.as_ref().map_or("DETACHED".into(), |ri| {
                    Graph::ref_debug_string(ri.ref_name.as_ref(), ri.worktree.as_ref())
                }),
                "⌂",
            ),
        };
        let target = self.target_ref.as_ref().map_or_else(
            || "!".to_string(),
            |t| {
                format!(
                    "{target}{ahead}",
                    target = t.ref_name,
                    ahead = if t.commits_ahead == 0 {
                        "".to_string()
                    } else {
                        format!("⇣{}", t.commits_ahead)
                    }
                )
            },
        );
        format!(
            "{meta}{sign}:{id}:{name} <> ✓{target}{bound}",
            meta = if self.metadata.is_some() { "📕" } else { "" },
            id = self.id.index(),
            bound = self
                .lower_bound
                .map(|base| format!(" on {}", base.to_hex_with_len(7)))
                .unwrap_or_default()
        )
    }
}

/// Utilities
impl TargetRef {
    /// Visit all segments whose commits would be considered 'upstream', or part of the target branch
    /// whose tip is identified with `target_segment`. The `lower_bound_segment_and_generation` is another way
    /// to stop the traversal.
    pub fn visit_upstream_commits(
        graph: &Graph,
        target_segment: SegmentIndex,
        lower_bound_segment_and_generation: Option<(SegmentIndex, usize)>,
        mut visit: impl FnMut(&Segment),
    ) {
        graph.visit_all_segments_including_start_until(target_segment, Direction::Outgoing, |s| {
            let prune = true;
            if lower_bound_segment_and_generation.is_some_and(|(lower_bound, lower_bound_generation)| {
                s.id == lower_bound || s.generation > lower_bound_generation
            }) || s.commits.iter().any(|c| c.flags.contains(CommitFlags::InWorkspace))
            {
                return prune;
            }
            visit(s);
            !prune
        });
    }
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Workspace({})", self.debug_string()))
            .field("id", &self.id.index())
            .field("kind", &self.kind)
            .field("stacks", &self.stacks)
            .field("metadata", &self.metadata)
            .field("target_ref", &self.target_ref)
            .field("extra_target", &self.extra_target)
            .finish()
    }
}
