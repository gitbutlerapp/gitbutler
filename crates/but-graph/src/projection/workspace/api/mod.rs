use std::borrow::Cow;

use anyhow::Context;
use bstr::BStr;
use but_core::{RefMetadata, extract_remote_name_and_short_name, ref_metadata::StackId};
use tracing::instrument;

use crate::{
    Workspace,
    workspace::{
        Stack, StackCommit, StackSegment, WorkspaceKind,
        workspace::find_segment_owner_indexes_by_refname,
    },
};

/// A utility type to represent `(stack_idx, segment_idx, commit_idx)`.
pub type CommitOwnerIndexes = (usize, usize, usize);

mod queries;
#[cfg(feature = "legacy")]
pub use queries::legacy::HeadStatus;

/// Lifecycle
impl Workspace {
    /// Redo the graph traversal with the same settings as before, but use the latest
    /// data from `repo`, `meta` and `project_meta` to do it.
    /// This is useful to make this instance represent changes to `repo` or `meta`.
    ///
    /// Pass a freshly read `project_meta` to pick up target changes as well, or
    /// `self.project_meta.clone()` to deliberately keep the current one,
    /// e.g. in the middle of an operation.
    #[instrument(
        name = "Workspace::refresh_from_head",
        level = "debug",
        skip_all,
        err(Debug)
    )]
    pub fn refresh_from_head(
        &mut self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
    ) -> anyhow::Result<()> {
        *self = Workspace::from_head(repo, meta, project_meta, self.options.clone())?;
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
        self.ref_info.as_ref().map(|ri| ri.ref_name.as_ref())
    }

    /// Like [Self::ref_name()], but returns reference and worktree information instead.
    pub fn ref_info(&self) -> Option<&crate::RefInfo> {
        self.ref_info.as_ref()
    }

    /// Like [`Self::ref_name()`], but return a generic `<anonymous>` name for unnamed workspaces.
    pub fn ref_name_display(&self) -> &BStr {
        self.ref_name()
            .map_or("<anonymous>".into(), |rn| rn.as_bstr())
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
                .symbolic_remote_names
                .iter()
                .map(|name| Cow::Borrowed(name.as_str().into()))
                .collect();
            extract_remote_name_and_short_name(tr.ref_name.as_ref(), &remote_names)
                .map(|(remote_name, _)| remote_name)
        } else {
            self.project_meta.push_remote.clone()
        }
    }

    /// Return the resolved target commit ID for use as a base for new branches.
    ///
    /// Prefers the stored [`Self::target_commit`] (the last-synced target SHA),
    /// falling back to the tip of [`Self::target_ref`] (the remote tracking branch).
    /// Does not consider additional traversal tips.
    ///
    /// Use [`Self::stored_target_commit_id()`] instead when callers need only the explicit
    /// stored target commit without falling back to the target ref tip.
    ///
    /// Returns `None` if neither `target_commit` nor `target_ref` is configured.
    pub fn resolved_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.stored_target_commit_id()
            .or_else(|| self.target_ref.as_ref().and_then(|t| t.tip_commit_id))
    }

    /// The commit graph underlying this workspace: one node per commit, edges child → parent.
    /// Merge-base and reachability queries are built on it.
    pub fn commit_graph(&self) -> crate::commit_graph::CommitGraph {
        self.commit_graph.clone().unwrap_or_default()
    }

    /// The commit graph underlying this workspace, or `None` for default/unborn workspaces that
    /// have no commits. Merge-base and reachability queries are built on it.
    pub fn commit_graph_ref(&self) -> Option<&crate::commit_graph::CommitGraph> {
        self.commit_graph.as_ref()
    }

    /// Return the `(merge-base, target-commit-id)` of the merge-base between `commit_to_merge`
    /// and the effective target side (see [`Self::effective_target_commit_id`]).
    /// Return `None` when no target is set, there is no merge-base, or `commit_to_merge` is not
    /// in the graph.
    ///
    /// Use this to get the merge-base for test-merges between `commit_to_merge` and the target,
    /// whose commit is also returned as `target-commit-id`.
    pub fn merge_base_with_target_branch(
        &self,
        commit_to_merge: impl Into<gix::ObjectId>,
    ) -> Option<(gix::ObjectId, gix::ObjectId)> {
        let commit_to_merge = commit_to_merge.into();
        let target = self.effective_target_commit_id()?;
        let merge_base = self.commit_graph().merge_base(commit_to_merge, target)?;
        Some((merge_base, target))
    }

    /// Return `true` if the workspace itself is where `HEAD` is pointing to.
    /// If `false`, one of the stack-segments is checked out instead.
    pub fn is_entrypoint(&self) -> bool {
        self.stacks
            .iter()
            .all(|s| s.segments.iter().all(|s| !s.is_entrypoint))
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
    pub fn is_branch_the_target_or_its_local_tracking_branch(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> bool {
        let Some(t) = self.target_ref.as_ref() else {
            return false;
        };

        t.ref_name.as_ref() == name
            || t.local_tracking
                .as_ref()
                .is_some_and(|local_tracking| local_tracking.ref_name.as_ref() == name)
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
    pub fn find_owner_indexes_by_commit_id(
        &self,
        oid: impl Into<gix::ObjectId>,
    ) -> Option<CommitOwnerIndexes> {
        let oid = oid.into();
        self.stacks
            .iter()
            .enumerate()
            .find_map(|(stack_idx, stack)| {
                stack
                    .segments
                    .iter()
                    .enumerate()
                    .find_map(|(seg_idx, seg)| {
                        seg.commits.iter().enumerate().find_map(|(cidx, c)| {
                            (c.id == oid).then_some((stack_idx, seg_idx, cidx))
                        })
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
    pub fn find_segment_owner_indexes_by_refname(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> Option<(usize, usize)> {
        find_segment_owner_indexes_by_refname(&self.stacks, ref_name)
    }

    /// Like [`Self::find_segment_owner_indexes_by_refname`], but fails with an error.
    pub fn try_find_segment_owner_indexes_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<(usize, usize)> {
        self.find_segment_owner_indexes_by_refname(name)
            .with_context(|| {
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
            let Some((entrypoint_stack, entrypoint_segment_idx)) =
                self.stacks.iter().find_map(|stack| {
                    stack
                        .segments
                        .iter()
                        .enumerate()
                        .find_map(|(idx, segment)| segment.is_entrypoint.then_some((stack, idx)))
                })
            else {
                return false;
            };
            entrypoint_stack
                .segments
                .get(entrypoint_segment_idx..)
                .into_iter()
                .any(|segments| {
                    segments
                        .iter()
                        .any(|s| s.ref_name().is_some_and(|rn| rn == name))
                })
        }
    }

    /// Try to find `name` in any named [`StackSegment`] and return it along with the stack containing it.
    pub fn find_segment_and_stack_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<(&Stack, &StackSegment)> {
        self.stacks.iter().find_map(|stack| {
            stack.segments.iter().find_map(|seg| {
                seg.ref_name()
                    .is_some_and(|rn| rn == name)
                    .then_some((stack, seg))
            })
        })
    }

    /// Try to find a commit in the workspace and return it along with the segment and stack containing it.
    pub fn find_commit_and_containers(
        &self,
        commit_id: gix::ObjectId,
    ) -> Option<(&Stack, &StackSegment, &StackCommit)> {
        self.stacks.iter().find_map(|stack| {
            stack.segments.iter().find_map(|seg| {
                seg.commits
                    .iter()
                    .find(|commit| commit.id == commit_id)
                    .map(|commit| (stack, seg, commit))
            })
        })
    }

    /// Like [`Self::find_segment_and_stack_by_refname`], but fails with an error.
    pub fn try_find_segment_and_stack_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<(&Stack, &StackSegment)> {
        self.find_segment_and_stack_by_refname(name)
            .with_context(|| {
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
        let ref_debug = |ri: &crate::RefInfo| {
            crate::debug::ref_debug_string_inner(
                ri.ref_name.as_ref(),
                ri.worktree.as_ref(),
                self.has_multiple_worktrees,
            )
        };
        let (name, sign) = match &self.kind {
            WorkspaceKind::Managed { ref_info } => (ref_debug(ref_info), "🏘️"),
            WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
                (ref_debug(ref_info), "🏘️⚠️")
            }
            WorkspaceKind::AdHoc => (
                self.ref_info.as_ref().map_or("DETACHED".into(), ref_debug),
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
            id = self.id,
            bound = self
                .lower_bound
                .map(|base| format!(" on {}", base.to_hex_with_len(7)))
                .unwrap_or_default()
        )
    }
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Workspace({})", self.debug_string()))
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("stacks", &self.stacks)
            .field("metadata", &self.metadata)
            .field("target_ref", &self.target_ref)
            .field("target_commit", &self.target_commit)
            .finish()
    }
}
