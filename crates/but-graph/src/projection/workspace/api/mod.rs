use anyhow::Context;
use bstr::BStr;
use but_core::{
    RefMetadata, extract_remote_name_and_short_name,
    ref_metadata::{ProjectedWorkspaceStack, StackId},
};
use petgraph::Direction;
use tracing::instrument;

use crate::{
    CommitFlags, CommitIndex, Graph, Segment, SegmentIndex, Workspace,
    workspace::{
        Lane, Stack, StackCommit, StackSegment, TargetRef, WorkspaceKind,
        workspace::find_segment_owner_indexes_by_refname,
    },
};

/// A utility type to represent `(stack_idx, segment_idx, commit_idx)`.
pub type CommitOwnerIndexes = (usize, usize, CommitIndex);

mod queries;

/// Lifecycle
impl Workspace {
    /// Redo the graph traversal with the same settings as before, but use the latest
    /// data from `repo`, `meta`, `project_meta` and `db` to do it.
    /// This is useful to make this instance represent changes to `repo` or `meta`.
    /// Worktree tips are [discovered](crate::init::Options::worktrees) afresh from
    /// `db` rather than reusing the previous traversal's, as they may have changed.
    ///
    /// Pass a freshly read `project_meta` to pick up target changes as well, or
    /// `self.graph.project_meta.clone()` to deliberately keep the current one,
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
        let graph = Graph::from_head(repo, meta, project_meta, db, self.graph.options.clone())?;
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

    /// Like [`Self::ref_name()`], but return a generic `<anonymous>` name for unnamed workspaces.
    pub fn ref_name_display(&self) -> &BStr {
        self.ref_name()
            .map_or("<anonymous>".into(), |rn| rn.as_bstr())
    }
}

/// Utilities
impl Workspace {
    /// Reconcile workspace metadata with the stacks in this projection using
    /// [`metadata.reconcile_projected_stacks()`](but_core::ref_metadata::Workspace::reconcile_projected_stacks).
    pub fn reconcile_metadata(
        &self,
        metadata: &mut but_core::ref_metadata::Workspace,
    ) -> anyhow::Result<()> {
        // Lanes share the history they converge on, so a branch can show in several stacks.
        // It belongs to the stack it heads, or else to the first stack showing it.
        let mut owner = std::collections::HashMap::new();
        for (idx, stack) in self.stacks.iter().enumerate() {
            for (pos, segment) in stack.segments.iter().enumerate() {
                let Some(name) = segment.ref_name() else {
                    continue;
                };
                let entry = owner.entry(name).or_insert((pos != 0, idx));
                if entry.0 && pos == 0 {
                    *entry = (false, idx);
                }
            }
        }
        metadata.reconcile_projected_stacks(
            self.stacks
                .iter()
                .enumerate()
                .map(|(idx, stack)| ProjectedWorkspaceStack {
                    id: stack.id,
                    branches: stack
                        .segments
                        .iter()
                        .filter_map(|segment| segment.ref_name())
                        .filter(|name| owner[name].1 == idx)
                        .map(ToOwned::to_owned)
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
            .graph
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

    /// Return the configured push remote, falling back to the local target's fetch remote.
    /// Without a configured target, use the projected upstream's remote instead.
    ///
    /// Return `None` when no remote is configured; repository configuration errors propagate.
    /// The caller may consider falling back to [`gix::Repository::remote_default_name()`],
    /// taking care to handle ambiguity.
    pub fn push_remote_name(&self, repo: &gix::Repository) -> anyhow::Result<Option<String>> {
        let project_meta = &self.graph.project_meta;
        if project_meta.push_remote.is_some() || project_meta.target_ref.is_some() {
            return project_meta.try_push_remote_name(repo);
        }
        Ok(self.target_ref.as_ref().and_then(|target| {
            let remote_names = self
                .graph
                .symbolic_remote_names
                .iter()
                .map(|name| name.as_str().into())
                .collect();
            extract_remote_name_and_short_name(target.ref_name.as_ref(), &remote_names)
                .map(|(remote_name, _)| remote_name)
        }))
    }

    /// Return the merge-base with the selected workspace cutoff, together with that cutoff's ID.
    ///
    /// Return `None` without a cutoff, if the input commit is absent from the graph,
    /// or if the commits have no merge-base.
    pub fn merge_base_with_target_commit(
        &self,
        commit_to_merge: impl Into<gix::ObjectId>,
    ) -> Option<(gix::ObjectId, gix::ObjectId)> {
        let target = self.target_commit.as_ref()?;
        let commit_to_merge = commit_to_merge.into();
        let commit_segment = self.graph.segment_by_commit_id(commit_to_merge).ok()?;
        let merge_base_segment_index = self
            .graph
            .find_merge_base(commit_segment.id, target.segment_index)?;

        // Segment ancestry alone does not locate the base when an input is inside a segment.
        let merge_base = if commit_segment.id == target.segment_index {
            let commit_index = commit_segment.commit_index_of(commit_to_merge)?;
            let target_index = commit_segment.commit_index_of(target.commit_id)?;
            commit_segment.commits[commit_index.max(target_index)].id
        } else if merge_base_segment_index == target.segment_index {
            target.commit_id
        } else if merge_base_segment_index == commit_segment.id {
            commit_to_merge
        } else {
            self.tip_commit_by_segment_id(merge_base_segment_index)?.id
        };
        Some((merge_base, target.commit_id))
    }

    /// Every lane, i.e. each stack followed by each worktree.
    pub fn lanes(&self) -> impl Iterator<Item = Lane<'_>> + '_ {
        self.stacks
            .iter()
            .map(Lane::Stack)
            .chain(self.worktrees.iter().map(Lane::Worktree))
    }

    /// Every segment of every lane.
    pub fn segments(&self) -> impl Iterator<Item = &StackSegment> + '_ {
        self.lanes().flat_map(|lane| lane.segments())
    }

    /// Every commit of every lane. This doesn't include the workspace commit.
    pub fn commits(&self) -> impl Iterator<Item = &StackCommit> + '_ {
        self.segments().flat_map(|segment| segment.commits.iter())
    }

    /// Find the commit with `oid` in any lane.
    pub fn find_commit(&self, oid: impl Into<gix::ObjectId>) -> Option<&StackCommit> {
        let oid = oid.into();
        self.commits().find(|commit| commit.id == oid)
    }

    /// Like [`Self::find_commit()`], but fails with an error.
    pub fn try_find_commit(&self, oid: impl Into<gix::ObjectId>) -> anyhow::Result<&StackCommit> {
        let oid = oid.into();
        self.find_commit(oid)
            .with_context(|| format!("Commit {oid} isn't part of the workspace"))
    }

    /// Return whether `name` is the configured local target or its derived upstream.
    /// Repository configuration errors propagate.
    pub fn is_target_or_upstream(
        &self,
        name: &gix::refs::FullNameRef,
        repo: &gix::Repository,
    ) -> anyhow::Result<bool> {
        let project_meta = &self.graph.project_meta;
        if project_meta
            .target_ref
            .as_ref()
            .is_some_and(|target| target.as_local_full_name_ref() == name)
        {
            return Ok(true);
        }
        Ok(project_meta
            .remote_target_ref(repo)?
            .is_some_and(|upstream| upstream.as_ref() == name))
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

    /// Try to find the `(stack_idx, segment_idx)` to be able to access the named segment going by `name`.
    /// Access the segment as `ws.stacks[stack_idx].segments[segment_idx]`
    pub fn find_segment_owner_indexes_by_refname(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> Option<(usize, usize)> {
        find_segment_owner_indexes_by_refname(&self.stacks, ref_name)
    }

    /// Return `true` if `name` is contained in the workspace as segment of a stack or a worktree.
    pub fn refname_is_segment(&self, name: &gix::refs::FullNameRef) -> bool {
        self.find_segment_by_refname(name).is_some()
    }

    /// Try to find `name` in any named [`StackSegment`] of a stack, then of a worktree.
    pub fn find_segment_by_refname(&self, name: &gix::refs::FullNameRef) -> Option<&StackSegment> {
        self.segments()
            .find(|segment| segment.ref_name() == Some(name))
    }

    /// Like [`Self::find_segment_by_refname`], but fails with an error.
    pub fn try_find_segment_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<&StackSegment> {
        self.find_segment_by_refname(name).with_context(|| {
            format!(
                "Couldn't find any stack or worktree that contained the branch named '{}'",
                name.shorten()
            )
        })
    }

    /// Try to find `name` in any named [`StackSegment`] and return it along with the lane containing it.
    pub fn find_segment_and_lane_by_refname(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<(Lane<'_>, &StackSegment)> {
        self.lanes().find_map(|lane| {
            lane.segments()
                .iter()
                .find(|segment| segment.ref_name() == Some(name))
                .map(|segment| (lane, segment))
        })
    }

    /// Try to find the [`StackSegment`] owning the commit `oid` and return it along with the lane containing it.
    pub fn find_segment_and_lane_by_commit_id(
        &self,
        oid: impl Into<gix::ObjectId>,
    ) -> Option<(Lane<'_>, &StackSegment)> {
        let oid = oid.into();
        self.lanes().find_map(|lane| {
            lane.segments()
                .iter()
                .find(|segment| segment.commits.iter().any(|commit| commit.id == oid))
                .map(|segment| (lane, segment))
        })
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
}

/// Debugging
impl Workspace {
    /// Produce a distinct and compressed debug string to show at a glance what the workspace is about.
    pub fn debug_string(&self) -> String {
        let graph = &self.graph;
        let (name, sign) = match &self.kind {
            WorkspaceKind::Managed { ref_info } => (
                graph.ref_debug_string_with_graph_context(
                    ref_info.ref_name.as_ref(),
                    ref_info.worktree.as_ref(),
                ),
                "🏘️",
            ),
            WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => (
                graph.ref_debug_string_with_graph_context(
                    ref_info.ref_name.as_ref(),
                    ref_info.worktree.as_ref(),
                ),
                "🏘️⚠️",
            ),
            WorkspaceKind::AdHoc => (
                graph[self.id]
                    .ref_info
                    .as_ref()
                    .map_or("DETACHED".into(), |ri| {
                        graph.ref_debug_string_with_graph_context(
                            ri.ref_name.as_ref(),
                            ri.worktree.as_ref(),
                        )
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
            "{meta}{sign}:{name} <> ✓{target}{bound}",
            meta = if self.metadata.is_some() { "📕" } else { "" },
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
    pub(crate) fn visit_upstream_commits(
        graph: &Graph,
        target_segment: SegmentIndex,
        lower_bound_segment_and_generation: Option<(SegmentIndex, usize)>,
        mut visit: impl FnMut(&Segment),
    ) {
        graph.visit_all_segments_including_start_until(target_segment, Direction::Outgoing, |s| {
            let prune = true;
            if lower_bound_segment_and_generation.is_some_and(
                |(lower_bound, lower_bound_generation)| {
                    s.id == lower_bound || s.generation > lower_bound_generation
                },
            ) || s
                .commits
                .iter()
                .any(|c| c.flags.contains(CommitFlags::InWorkspace))
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
        let mut s = f.debug_struct(&format!("Workspace({})", self.debug_string()));
        s.field("id", &self.id.index())
            .field("kind", &self.kind)
            .field("stacks", &self.stacks)
            .field("metadata", &self.metadata)
            .field("target_ref", &self.target_ref)
            .field("target_commit", &self.target_commit);
        if !self.worktrees.is_empty() {
            s.field("worktrees", &self.worktrees);
        }
        s.finish()
    }
}
