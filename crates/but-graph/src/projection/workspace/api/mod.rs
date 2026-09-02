use anyhow::Context;
use bstr::BStr;
use but_core::{
    RefMetadata, extract_remote_name_and_short_name,
    ref_metadata::{ProjectedWorkspaceStack, StackId},
};
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
    /// data from `repo`, `meta`, `project_meta` and `db` to do it.
    /// This is useful to make this instance represent changes to `repo` or `meta`.
    /// Worktree tips are [discovered](crate::init::Options::worktrees) afresh from
    /// `db` rather than reusing the previous traversal's, as they may have changed.
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
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<()> {
        *self = Workspace::from_head(repo, meta, project_meta, db, self.options.clone())?;
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

/// Validation
impl Workspace {
    /// Validate the projection for consistency and fail loudly when an issue was found.
    /// Use this before using the workspace for anything serious, but particularly in testing.
    /// Invariants that a partial traversal cannot uphold are skipped if the hard limit was hit.
    pub fn validated(self) -> anyhow::Result<Self> {
        use anyhow::{Context as _, ensure};
        let branches = self.branches().unwrap_or_default();
        let commit_graph = self.commit_graph_ref();
        if !branches.is_empty() {
            let entrypoints = branches.iter().filter(|b| b.is_entrypoint).count();
            ensure!(
                entrypoints == 1,
                "expected exactly one entrypoint branch record, found {entrypoints} among {:?} (entrypoint ref: {:?}, commit: {:?})",
                branches
                    .iter()
                    .map(|b| (
                        b.ref_name.as_ref().map(|n| n.to_string()),
                        b.commits.first().map(|c| c.id.to_string()),
                        b.is_entrypoint
                    ))
                    .collect::<Vec<_>>(),
                self.entrypoint_ref.as_ref().map(|n| n.to_string()),
                self.entrypoint_commit_id
            );
        }
        if let Some(ep_ref) = self.entrypoint_ref.as_ref()
            && !branches.is_empty()
            && !self.hard_limit_hit
        {
            let named = branches.iter().any(|b| b.ref_name.as_ref() == Some(ep_ref))
                || branches
                    .iter()
                    .flat_map(|b| b.commits.iter())
                    .any(|c| c.refs.iter().any(|ri| ri.ref_name == *ep_ref));
            ensure!(
                named,
                "entrypoint ref {ep_ref} is not represented in the branch records"
            );
        }
        let mut owners = std::collections::HashMap::new();
        for (idx, branch) in branches.iter().enumerate() {
            for &(target, _) in &branch.outgoing {
                ensure!(
                    target < branches.len(),
                    "branch {idx} connects to non-existing branch {target}"
                );
                ensure!(target != idx, "branch {idx} connects to itself");
            }
            for commit in &branch.commits {
                if let Some(prev) = owners.insert(commit.id, idx) {
                    anyhow::bail!("commit {} is owned by branches {prev} and {idx}", commit.id);
                }
                if let Some(commit_graph) = commit_graph {
                    commit_graph.commit(commit.id).with_context(|| {
                        format!("commit {} is not in the commit graph", commit.id)
                    })?;
                }
            }
        }
        for stack in &self.stacks {
            for segment in &stack.segments {
                for commit in &segment.commits {
                    ensure!(
                        owners.contains_key(&commit.id),
                        "stack commit {} is not owned by any branch record",
                        commit.id
                    );
                }
            }
        }
        Ok(self)
    }

    /// The managed workspace commit the entrypoint sits on, if the workspace has one: the commit
    /// itself when this is a managed workspace, and otherwise the entrypoint commit if its message
    /// marks it as GitButler-created. `repo` reads the message.
    pub fn managed_entrypoint_commit(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<Option<crate::Commit>> {
        let Some(commit_graph) = self.commit_graph_ref() else {
            return Ok(None);
        };
        let Some(id) = self.branch_graph(repo).workspace_commit else {
            return Ok(None);
        };
        Ok(commit_graph.commit(id).cloned())
    }
}

/// Utilities
impl Workspace {
    /// The commits the target branch has that the workspace does not: everything reachable from
    /// the target tip but not from the workspace's lower bound, excluding commits already in the
    /// workspace. Empty when the target tip is unknown to the traversal.
    pub fn incoming_target_commit_ids(&self) -> anyhow::Result<Vec<gix::ObjectId>> {
        let target_ref = self
            .target_ref
            .as_ref()
            .context("incoming target commits require a workspace with a target ref")?;
        let (Some(target_tip), Some(commit_graph)) =
            (target_ref.tip_commit_id, self.commit_graph_ref())
        else {
            return Ok(Vec::new());
        };
        if commit_graph.commit(target_tip).is_none() {
            return Ok(Vec::new());
        }
        let candidates: Vec<gix::ObjectId> = match self.lower_bound {
            Some(lower_bound) => commit_graph
                .commits_reachable_from_a_not_b(target_tip, lower_bound, false)
                .into_iter()
                .collect(),
            None => commit_graph.ancestor_ids(target_tip).into_iter().collect(),
        };
        let generations = commit_graph.generation_by_commit_id();
        let mut commit_ids: Vec<_> = candidates
            .into_iter()
            .filter(|id| {
                !commit_graph
                    .commit(*id)
                    .is_some_and(|c| c.flags.contains(crate::CommitFlags::InWorkspace))
            })
            .collect();
        // Newest first, like a walk down from the target tip.
        commit_ids.sort_by_key(|id| generations.get(id).copied().unwrap_or_default());
        Ok(commit_ids)
    }

    /// Reconcile workspace metadata with the stacks in this projection using
    /// [`metadata.reconcile_projected_stacks()`](but_core::ref_metadata::Workspace::reconcile_projected_stacks).
    pub fn reconcile_metadata(
        &self,
        metadata: &mut but_core::ref_metadata::Workspace,
    ) -> anyhow::Result<()> {
        metadata.reconcile_projected_stacks(
            self.stacks.iter().map(|stack| ProjectedWorkspaceStack {
                id: stack.id,
                branches: stack
                    .segments
                    .iter()
                    .filter_map(|segment| segment.ref_name().map(ToOwned::to_owned))
                    .collect(),
            }),
            |_| StackId::generate(),
        )
    }

    /// Return workspace metadata normalized against this projection.
    ///
    /// Unlike [`Self::metadata`], applied stacks absent from the projection are
    /// treated as outside the workspace, and branches absent from a projected
    /// stack are excluded.
    ///
    /// Branches checked out in linked worktrees are deliberately absent from
    /// projected stacks, but they remain part of their recorded stack - being
    /// checked out elsewhere is transient state, not a workspace change - so
    /// they count as present here.
    pub fn metadata_from_projection(
        &self,
    ) -> anyhow::Result<Option<but_core::ref_metadata::Workspace>> {
        let Some(mut metadata) = self.metadata.clone() else {
            return Ok(None);
        };
        let worktree_refs: std::collections::BTreeSet<&gix::refs::FullName> = self
            .worktree_tips
            .iter()
            .filter_map(|tip| tip.ref_name.as_ref())
            .collect();
        for stack in &mut metadata.stacks {
            if !stack.workspacecommit_relation.is_in_workspace() {
                continue;
            }
            let Some(projected_stack) = self.stacks.iter().find(|projected| {
                projected.id == Some(stack.id)
                    || projected.segments.iter().any(|segment| {
                        segment.ref_name().is_some_and(|projected_ref| {
                            stack
                                .branches
                                .iter()
                                .any(|branch| branch.ref_name == projected_ref)
                        })
                    })
            }) else {
                if stack
                    .branches
                    .iter()
                    .any(|branch| worktree_refs.contains(&branch.ref_name))
                {
                    continue;
                }
                stack.workspacecommit_relation =
                    but_core::ref_metadata::WorkspaceCommitRelation::Outside;
                continue;
            };
            stack.branches.retain(|branch| {
                worktree_refs.contains(&branch.ref_name)
                    || projected_stack
                        .segments
                        .iter()
                        .any(|segment| segment.ref_name() == Some(branch.ref_name.as_ref()))
            });
        }
        self.reconcile_metadata(&mut metadata)?;
        Ok(Some(metadata))
    }

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
                .map(|name| name.as_str().into())
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
            "{meta}{sign}:{name} <> ✓{target}{bound}",
            meta = if self.metadata.is_some() { "📕" } else { "" },
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
