use std::{
    cell::RefCell,
    collections::{BTreeSet, VecDeque},
    fmt::Formatter,
};

use crate::projection::workspace;
use crate::{
    CommitFlags, CommitIndex, Graph, Segment, SegmentIndex,
    projection::{Stack, StackCommit, StackCommitFlags, StackSegment},
    segment,
};
use anyhow::Context;
use bstr::{BStr, ByteSlice};
use but_core::ref_metadata;
use but_core::ref_metadata::StackId;
use gix::reference::Category;
use itertools::Itertools;
use petgraph::{Direction, prelude::EdgeRef, visit::NodeRef};
use tracing::instrument;

/// A workspace is a list of [Stacks](Stack).
#[derive(Clone)]
pub struct Workspace<'graph> {
    /// The underlying graph for providing simplified access to data.
    pub graph: &'graph Graph,
    /// An ID which uniquely identifies the [graph segment](Segment) that represents the tip of the workspace.
    pub id: SegmentIndex,
    /// Specify what kind of workspace this is.
    pub kind: WorkspaceKind,
    /// One or more stacks that live in the workspace.
    pub stacks: Vec<Stack>,
    /// The bound can be imagined as the commit from which all other commits in the workspace originate.
    /// It can also be imagined to be the delimiter at the bottom beyond which nothing belongs to the workspace,
    /// as antagonist to the first commit in tip of the segment with `id`, serving as first commit that is
    /// inside the workspace.
    ///
    /// As such, it's always the longest path to the first shared commit with the target among
    /// all of our stacks, or it is the first commit that is shared among all of our stacks in absence of a target.
    /// One can also think of it as the starting point from which all workspace commits can be reached when
    /// following all incoming connections and stopping at the tip of the workspace.
    ///
    /// It is `None` there is only a single stack and no target, so nothing was integrated.
    pub lower_bound: Option<gix::ObjectId>,
    /// If `lower_bound` is set, this is the segment owning the commit.
    pub lower_bound_segment_id: Option<SegmentIndex>,
    /// The target to integrate workspace stacks into.
    ///
    /// If `None`, this is a local workspace that doesn't know when possibly pushed branches are considered integrated.
    /// This happens when there is a local branch checked out without a remote tracking branch.
    pub target: Option<Target>,
    /// The segment index of the extra target as provided for traversal,
    /// useful for AdHoc workspaces, but generally applicable to all workspaces to keep the lower bound lower than it
    /// otherwise would be.
    pub extra_target: Option<SegmentIndex>,
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    /// If this is `Some()` the `kind` is always [`WorkspaceKind::Managed`]
    pub metadata: Option<ref_metadata::Workspace>,
}

pub type CommitOwnerIndexes = (usize, usize, CommitIndex);

/// Utilities
impl Workspace<'_> {
    /// Return `true` if the workspace itself is where `HEAD` is pointing to.
    /// If `false`, one of the stack-segments is checked out instead.
    pub fn is_entrypoint(&self) -> bool {
        self.stacks
            .iter()
            .all(|s| s.segments.iter().all(|s| !s.is_entrypoint))
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
        self.stacks
            .iter()
            .enumerate()
            .find_map(|(stack_idx, stack)| {
                stack
                    .segments
                    .iter()
                    .enumerate()
                    .find_map(|(seg_idx, seg)| {
                        seg.ref_name
                            .as_ref()
                            .is_some_and(|rn| rn.as_ref() == ref_name)
                            .then_some((stack_idx, seg_idx))
                    })
            })
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
                        .any(|s| s.ref_name.as_ref().is_some_and(|rn| rn.as_ref() == name))
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
                seg.ref_name
                    .as_ref()
                    .is_some_and(|rn| rn.as_ref() == name)
                    .then_some((stack, seg))
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

/// A classifier for the workspace.
#[derive(Debug, Clone)]
pub enum WorkspaceKind {
    /// The `HEAD` is pointing to a dedicated workspace reference, like `refs/heads/gitbutler/workspace`.
    /// This also means that we have a workspace commit that `ref_name` points to directly, which is also owned
    /// exclusively by the underlying segment.
    Managed {
        /// The name of the reference pointing to the workspace commit. Useful for deriving the workspace name.
        ref_name: gix::refs::FullName,
    },
    /// Information for when a workspace reference was *possibly* advanced by hand and does not point to a
    /// managed workspace commit (anymore).
    /// That workspace commit, may be reachable by following the first parent from the workspace reference.
    ///
    /// Note that the stacks that follow *will* be in unusable if the workspace commit is in a segment below,
    /// but typically is usable if there is just a single real stack, or any amount of virtual stacks below
    /// (i.e. those that have no commits and are just marked by references).
    ManagedMissingWorkspaceCommit {
        /// The name of the reference pointing to the workspace commit. Useful for deriving the workspace name.
        ref_name: gix::refs::FullName,
    },
    /// A segment is checked out directly.
    ///
    /// It can be inside or outside a workspace.
    /// If the respective segment is [not named](Workspace::ref_name), this means the `HEAD` id detached.
    /// The commit that the working tree is at is always implied to be the first commit of the [`StackSegment`]
    /// at [`Workspace::id`].
    AdHoc,
}

impl WorkspaceKind {
    fn managed(ref_name: &Option<gix::refs::FullName>) -> anyhow::Result<Self> {
        Ok(WorkspaceKind::Managed {
            ref_name: ref_name
                .as_ref()
                .cloned()
                .context("BUG: managed workspaces must always be on a named segment")?,
        })
    }
}

/// Information about the target reference.
#[derive(Debug, Clone)]
pub struct Target {
    /// The name of the target branch, i.e. the branch that all [Stacks](Stack) want to get merged into.
    /// Typically, this is `origin/main`.
    pub ref_name: gix::refs::FullName,
    /// The index to the respective segment in the graph.
    pub segment_index: SegmentIndex,
    /// The amount of commits that aren't reachable by any segment in the workspace, they are in its future.
    pub commits_ahead: usize,
}

impl Target {
    /// Return `None` if `ref_name` wasn't found as segment in `graph`.
    /// This can happen if a reference is configured, but not actually present as reference.
    /// Note that `commits_ahead` isn't set yet, see [`Self::compute_and_set_commits_ahead()`].
    fn from_ref_name_without_commits_ahead(
        ref_name: &gix::refs::FullName,
        graph: &Graph,
    ) -> Option<Self> {
        let target_segment = graph.inner.node_indices().find_map(|n| {
            let s = &graph[n];
            (s.ref_name.as_ref() == Some(ref_name)).then_some(s)
        })?;
        Some(Target {
            ref_name: ref_name.to_owned(),
            segment_index: target_segment.id,
            commits_ahead: 0,
        })
    }

    fn compute_and_set_commits_ahead(
        &mut self,
        graph: &Graph,
        lower_bound_segment: Option<SegmentIndex>,
    ) {
        let lower_bound = lower_bound_segment.map(|sidx| (sidx, graph[sidx].generation));
        self.commits_ahead = 0;
        Self::visit_upstream_commits(graph, self.segment_index, lower_bound, |s| {
            self.commits_ahead += s.commits.len();
        })
    }
}

/// Utilities
impl Target {
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

pub(crate) enum Downgrade {
    /// Allows to turn a workspace above a selection to be downgraded back to the selection if it turns
    /// out to be outside the workspace.
    /// This is typically what you want when producing a workspace for display, as the workspace then isn't relevant.
    Allow,
    /// Use this if the closest workspace is what you want, even if the reference in question is below the workspace lower bound.
    Disallow,
}

impl Graph {
    /// Analyse the current graph starting at its [entrypoint](Self::lookup_entrypoint()).
    ///
    /// No matter what, each location of `HEAD`, which corresponds to the entrypoint, can be represented as workspace.
    /// Further, the most expensive operations we perform to query additional commit information by reading it, but we
    /// only do so on the ones that the user can interact with.
    ///
    /// The [`extra_target`](crate::init::Options::extra_target) options extends the workspace to include that target as base.
    /// This affects what we consider to be the part of the workspace.
    /// Typically, that's a previous location of the target segment.
    #[instrument(skip(self), err(Debug))]
    pub fn to_workspace(&self) -> anyhow::Result<Workspace<'_>> {
        self.to_workspace_inner(workspace::Downgrade::Allow)
    }

    pub(crate) fn to_workspace_inner(&self, downgrade: Downgrade) -> anyhow::Result<Workspace<'_>> {
        let (kind, metadata, mut ws_tip_segment, entrypoint_sidx, entrypoint_first_commit_flags) = {
            let ep = self.lookup_entrypoint()?;
            match ep.segment.workspace_metadata() {
                None => {
                    // Skip over empty segments.
                    if let Some((maybe_integrated_flags, sidx_of_flags)) = self
                        .first_commit_or_find_along_first_parent(ep.segment_index)
                        .map(|(c, sidx)| (c.flags, sidx))
                        .filter(|(f, _sidx)| f.contains(CommitFlags::InWorkspace))
                    {
                        // search the (for now just one) workspace upstream and use it instead,
                        // mark this segment as entrypoint.
                        // Note that at this time the entrypoint could still be below the fork-point of the workspace.
                        let ws_segment = self
                            .find_segment_upwards(sidx_of_flags, |s| {
                                s.workspace_metadata().is_some()
                            })
                            .with_context(|| {
                                format!(
                                    "BUG: should have found upstream workspace segment from {sidx_of_flags:?} as commit is marked as such"
                                )
                            })?;

                        (
                            WorkspaceKind::managed(&ws_segment.ref_name)?,
                            ws_segment.workspace_metadata().cloned(),
                            ws_segment,
                            Some(ep.segment_index),
                            maybe_integrated_flags,
                        )
                    } else {
                        (
                            WorkspaceKind::AdHoc,
                            None,
                            ep.segment,
                            None,
                            CommitFlags::empty(),
                        )
                    }
                }
                Some(meta) => (
                    WorkspaceKind::managed(&ep.segment.ref_name)?,
                    Some(meta.clone()),
                    ep.segment,
                    None,
                    CommitFlags::empty(),
                ),
            }
        };

        let mut ws = Workspace {
            graph: self,
            id: ws_tip_segment.id,
            kind,
            stacks: vec![],
            target: metadata.as_ref().and_then(|md| {
                Target::from_ref_name_without_commits_ahead(md.target_ref.as_ref()?, self)
            }),
            extra_target: self.extra_target,
            metadata,
            lower_bound_segment_id: None,
            lower_bound: None,
        };

        let ws_lower_bound = if ws.has_managed_ref() {
            self.compute_lowest_base(ws.id, ws.target.as_ref(), self.extra_target)
                .or_else(|| {
                    // target not available? Try the base of the workspace itself
                    if self
                        .inner
                        .neighbors_directed(ws_tip_segment.id, Direction::Outgoing)
                        .count()
                        == 1
                    {
                        None
                    } else {
                        self.inner
                            .neighbors_directed(ws_tip_segment.id, Direction::Outgoing)
                            .reduce(|a, b| self.first_merge_base(a, b).unwrap_or(a))
                            .and_then(|base| self[base].commits.first().map(|c| (c.id, base)))
                    }
                })
        } else {
            None
        };

        (ws.lower_bound, ws.lower_bound_segment_id) = ws_lower_bound
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or_default();

        // The entrypoint is integrated and has a workspace above it.
        // Right now we would be using it, but will discard it the entrypoint is *at* or *below* the merge-base.
        if let Some(((_lowest_base, lowest_base_sidx), ep_sidx)) = ws_lower_bound
            .filter(|_| {
                matches!(downgrade, Downgrade::Allow)
                    && entrypoint_first_commit_flags.contains(CommitFlags::Integrated)
            })
            .zip(entrypoint_sidx)
            && (ep_sidx == lowest_base_sidx
                || self
                    .find_map_downwards_along_first_parent(ep_sidx, |s| {
                        (s.id == lowest_base_sidx).then_some(())
                    })
                    .is_none())
        {
            // We cannot reach the lowest workspace base, by definition reachable through any path downward,
            // so we are outside the workspace limits which is above us. Turn the data back into entrypoint-only.
            let Workspace {
                graph: _,
                id,
                kind: head,
                stacks: _,
                target,
                metadata,
                extra_target: _,
                lower_bound,
                lower_bound_segment_id,
            } = &mut ws;
            *id = ep_sidx;
            *head = WorkspaceKind::AdHoc;
            *target = None;
            *metadata = None;
            ws_tip_segment = &self[ep_sidx];
            *lower_bound = None;
            *lower_bound_segment_id = None;
        }

        if ws.has_managed_ref() && self[ws.id].commits.is_empty() {
            ws.kind = WorkspaceKind::ManagedMissingWorkspaceCommit {
                ref_name: ws_tip_segment
                    .ref_name
                    .clone()
                    .expect("BUG: must be set or we wouldn't be here"),
            };
        }

        fn segment_name_is_special(s: &Segment) -> bool {
            s.ref_name
                .as_ref()
                .is_some_and(|rn| rn.as_bstr().starts_with_str("refs/heads/gitbutler/"))
        }

        if ws.has_managed_ref() {
            let (lowest_base, lowest_base_sidx) =
                ws_lower_bound.map_or((None, None), |(base, sidx)| (Some(base), Some(sidx)));
            for stack_top_sidx in self
                .inner
                .neighbors_directed(ws_tip_segment.id, Direction::Outgoing)
            {
                let stack_segment = &self[stack_top_sidx];
                let has_seen_base = RefCell::new(false);
                ws.stacks.extend(
                    self.collect_stack_segments(
                        stack_top_sidx,
                        entrypoint_sidx,
                        |s| {
                            let stop = true;
                            // The lowest base is a segment that all stacks will run into.
                            // If we meet it, we are done. Note how we ignored the integration state
                            // as pruning of fully integrated stacks happens later.
                            if Some(s.id) == lowest_base_sidx {
                                has_seen_base.replace(true);
                                return stop;
                            }
                            // Assure entrypoints get their own segments
                            if s.id != stack_top_sidx && Some(s.id) == entrypoint_sidx {
                                return stop;
                            }
                            // Check for anonymous segments with sibling ID - these know their
                            // named counterparts and we want to set the name, but they must
                            // be in their own stack-segment.
                            if s.ref_name.is_none() && s.sibling_segment_id.is_some() {
                                return stop;
                            }
                            if segment_name_is_special(s) {
                                return !stop;
                            }
                            match (
                                &stack_segment.ref_name,
                                s.ref_name
                                    .as_ref()
                                    .filter(|rn| rn.category() == Some(Category::LocalBranch)),
                            ) {
                                (Some(_), Some(_)) | (None, Some(_)) => stop,
                                (Some(_), None) | (None, None) => !stop,
                            }
                        },
                        |s| {
                            !*has_seen_base.borrow()
                                && self
                                    .inner
                                    .neighbors_directed(s.id, Direction::Incoming)
                                    .all(|n| n.id() != ws_tip_segment.id)
                        },
                        |s| Some(s.id) == ws.lower_bound_segment_id && s.metadata.is_none(),
                    )?
                    .and_then(|segments| {
                        let stack_id = find_matching_stack_id(ws.metadata.as_ref(), &segments);
                        // If we find no stack ID, then the segment is not included in the workspace metadata,
                        // indicating it's ignored. Just to be even more certain, if it starts with a commit
                        // that is the workspace base, then we definitely don't want to show it - it's unapplied.
                        if stack_id.is_none()
                            && segments
                                .first()
                                .is_some_and(|s| s.commits.first().map(|c| c.id) == lowest_base)
                        {
                            None
                        } else {
                            Some(Stack::from_base_and_segments(
                                &self.inner,
                                segments,
                                stack_id,
                            ))
                        }
                    }),
                );
            }
        } else {
            let start = ws_tip_segment;
            ws.stacks.extend(
                // TODO: This probably depends on more factors, could have relationship with remote tracking branch.
                self.collect_stack_segments(
                    start.id,
                    None,
                    |s| {
                        let stop = true;
                        if segment_name_is_special(s) {
                            return !stop;
                        }
                        match (&start.ref_name, &s.ref_name) {
                            (Some(_), Some(_)) | (None, Some(_)) => stop,
                            (Some(_), None) | (None, None) => !stop,
                        }
                    },
                    // We keep going until depletion
                    |_s| true,
                    // Never discard stacks
                    |_s| false,
                )?
                .map(|segments| Stack::from_base_and_segments(&self.inner, segments, None)),
            );
        }

        if let Some(target) = ws.target.as_mut() {
            target.compute_and_set_commits_ahead(self, ws.lower_bound_segment_id);
        }

        ws.prune_archived_segments();
        ws.mark_remote_reachability()?;
        Ok(ws)
    }

    /// Compute the lowest base (i.e. the highest generation) between the `ws_tip` of a top-most segment of the workspace,
    /// another `target` segment, and any amount of `additional` segments which could be *past targets* to keep
    /// an artificial lower base for consistency.
    ///
    /// Returns `Some((lowest_base, segment_idx_with_lowest_base))`.
    ///
    /// ## Note
    ///
    /// This is a **merge-base octopus** effectively, and works without generation numbers.
    // TODO: actually compute the lowest base, see `first_merge_base()` which should be `lowest_merge_base()` by itself,
    //       accounting for finding the lowest of all merge-bases which would be assumed to be reachable by all segments
    //       searching downward, a necessary trait for many search problems.
    fn compute_lowest_base(
        &self,
        ws_tip: SegmentIndex,
        target: Option<&Target>,
        additional: impl IntoIterator<Item = SegmentIndex>,
    ) -> Option<(gix::ObjectId, SegmentIndex)> {
        // It's important to not start from the tip, but instead find paths to the merge-base from each stack individually.
        // Otherwise, we may end up with a short path to a segment that isn't actually reachable by all stacks.
        let stacks = self.inner.neighbors_directed(ws_tip, Direction::Outgoing);
        let mut count = 0;
        let base = stacks
            .chain(target.map(|t| t.segment_index))
            .chain(additional)
            .inspect(|_| count += 1)
            .reduce(|a, b| self.first_merge_base(a, b).unwrap_or(a))?;

        if count < 2 || base == ws_tip {
            None
        } else {
            self.first_commit_or_find_along_first_parent(base)
                .map(|(c, sidx)| (c.id, sidx))
        }
    }

    /// Compute the loweset merge-base between two segments.
    /// Such a merge-base is reachable from all possible paths from `a` and `b`.
    ///
    /// We know this works as all branching and merging is represented by a segment.
    /// Thus, the merge-base is always the first commit of the returned segment
    // TODO: should be multi, with extra segments as third parameter
    // TODO: actually find the lowest merge-base, right now it just finds the first merge-base, but that's not
    //       the lowest.
    fn first_merge_base(&self, a: SegmentIndex, b: SegmentIndex) -> Option<SegmentIndex> {
        // TODO(perf): improve this by allowing to set bitflags on the segments themselves, to allow
        //       marking them accordingly, just like Git does.
        //       Right now we 'emulate' bitflags on pre-allocated data with two data sets, expensive
        //       in comparison.
        //       And yes, let's avoid `gix::Repository::merge_base` as we have free
        //       generation numbers here and can avoid work duplication.
        let mut segments_reachable_by_b = BTreeSet::new();
        self.visit_all_segments_including_start_until(b, Direction::Outgoing, |s| {
            segments_reachable_by_b.insert(s.id);
            // Collect everything, keep it simple.
            // This is fast* as completely in memory.
            // *means slow compared to an array traversal with memory locality.
            false
        });

        let mut candidate = None;
        self.visit_all_segments_including_start_until(a, Direction::Outgoing, |s| {
            if candidate.is_some() {
                return true;
            }
            let prune = segments_reachable_by_b.contains(&s.id);
            if prune {
                candidate = Some(s.id);
            }
            prune
        });
        if candidate.is_none() {
            // TODO: improve this - workspaces shouldn't be like this but if they are, do we deal with it well?
            tracing::warn!(
                "Couldn't find merge-base between segments {a:?} and {b:?} - this might lead to unexpected results"
            )
        }
        candidate
    }
}

/// This works as named segments have been created in a prior step. Thus, we are able to find best matches by
/// the amount of matching names, probably.
fn find_matching_stack_id(
    metadata: Option<&ref_metadata::Workspace>,
    segments: &[StackSegment],
) -> Option<StackId> {
    let metadata = metadata?;
    metadata
        .stacks
        .iter()
        .filter_map(|s| {
            let num_matching_refs = s
                .branches
                .iter()
                .filter(|b| {
                    segments
                        .iter()
                        .any(|s| s.ref_names().any(|rn| rn == b.ref_name.as_ref()))
                })
                .count();
            (num_matching_refs != 0).then_some((s.id, num_matching_refs))
        })
        .sorted_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs).reverse())
        .next()
        .map(|(stack_id, _)| stack_id)
}

/// Traversals
impl Graph {
    /// Return the ancestry of `start` along the first parents, itself included, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    ///
    /// Note that the traversal assumes as well-segmented graph without cycles.
    fn collect_first_parent_segments_until<'a>(
        &'a self,
        start: &'a Segment,
        mut stop: impl FnMut(&Segment) -> bool,
    ) -> (Vec<&'a Segment>, Option<&'a Segment>) {
        let mut out = vec![start];
        let mut edge = self
            .inner
            .edges_directed(start.id, Direction::Outgoing)
            .last();
        let mut stopped_at = None;
        let mut seen = BTreeSet::new();
        while let Some(first_edge) = edge {
            let next = &self[first_edge.target()];
            if stop(next) {
                stopped_at = Some(next);
                break;
            }
            out.push(next);
            if seen.insert(next.id) {
                edge = self
                    .inner
                    .edges_directed(next.id, Direction::Outgoing)
                    .last();
            }
        }
        (out, stopped_at)
    }

    /// Visit all segments from `start`, excluding, and return once `find` returns something mapped from the
    /// first suitable segment it encountered.
    fn find_map_downwards_along_first_parent<T>(
        &self,
        start: SegmentIndex,
        mut find: impl FnMut(&Segment) -> Option<T>,
    ) -> Option<T> {
        let mut out = None;
        self.visit_segments_downward_along_first_parent_exclude_start(start, |s| {
            if let Some(res) = find(s) {
                out = Some(res);
                true
            } else {
                false
            }
        });
        out
    }

    /// Return `(commit, start)` if `start` has a commit, or find the first commit downstream along the first parent.
    pub(crate) fn first_commit_or_find_along_first_parent(
        &self,
        start: SegmentIndex,
    ) -> Option<(&crate::Commit, SegmentIndex)> {
        self[start].commits.first().map(|c| (c, start)).or_else(|| {
            self.find_map_downwards_along_first_parent(start, |s| s.commits.first().map(|_c| s.id))
                // workaround borrowchk
                .map(|sidx| (self[sidx].commits.first().expect("present"), sidx))
        })
    }

    /// Return `OK(None)` if the post-process discarded this segment after collecting it in full as it was not
    /// local a local branch.
    ///
    /// `entrypoint_sidx` is passed to set the collected segment as entrypoint automatically.
    ///
    /// `is_one_past_end_of_stack_segment(s)` returns `true` if the graph segment `s` should be considered past the
    /// currently collected stack segment. If `false` is returned, it will become part of the current stack segment.
    /// It's not called for the first segment, so you can use it to compare the first with other segments.
    ///
    /// `starts_next_stack_segment(s)` returns `true` if a new stack segment should be started with `s` as first member,
    /// or `false` if the stack segments are complete and with it all stack segments.
    ///
    /// `discard_stack(stack_segment)` returns `true` if after collecting everything, we'd still want to discard the
    /// whole stack due to custom rules, after assuring the stack segment is no entrypoint.
    /// It's also called to determine if a stack-segment (from the bottom of the stack upwards) should be discarded.
    /// If the stack is empty at the end, it will be discarded in full.
    fn collect_stack_segments(
        &self,
        from: SegmentIndex,
        entrypoint_sidx: Option<SegmentIndex>,
        mut is_one_past_end_of_stack_segment: impl FnMut(&Segment) -> bool,
        mut starts_next_stack_segment: impl FnMut(&Segment) -> bool,
        mut discard_stack: impl FnMut(&StackSegment) -> bool,
    ) -> anyhow::Result<Option<Vec<StackSegment>>> {
        // TODO: Test what happens if a workspace commit is pointed at by a different ref (which is the entrypoint).
        let mut out = Vec::new();
        let mut next = Some(from);
        while let Some(from) = next.take() {
            let start = &self[from];
            let (segments, stopped_at) = self
                .collect_first_parent_segments_until(start, &mut is_one_past_end_of_stack_segment);
            let mut segment = StackSegment::from_graph_segments(&segments, self)?;
            if entrypoint_sidx.is_some_and(|id| segment.id == id) {
                segment.is_entrypoint = true;
            }
            out.push(segment);
            next = stopped_at
                .filter(|s| starts_next_stack_segment(s))
                .map(|s| s.id);
        }

        fn is_entrypoint_or_local(s: &StackSegment) -> bool {
            if s.is_entrypoint {
                return true;
            }
            s.ref_name
                .as_ref()
                .and_then(|rn| rn.category())
                .is_none_or(|c| c == Category::LocalBranch)
        }

        // Prune empty invalid ones from the front as cleanup.
        // This isn't an issue for algorithms as they always see the full version.
        // TODO: remove this once we don't have remotes in a workspace because traversal logic can do it better.
        if let Some(end) = out
            .iter()
            .enumerate()
            .take_while(|(_idx, s)| s.commits.is_empty() && !is_entrypoint_or_local(s))
            .map(|(idx, _s)| idx + 1)
            .last()
        {
            out.drain(..end);
        }

        // Definitely remove non-local empties from behind.
        // TODO: revise this
        if let Some(new_len) = out
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_idx, s)| s.commits.is_empty() && !is_entrypoint_or_local(s))
            .last()
            .map(|(idx, _s)| idx)
        {
            out.truncate(new_len);
        }

        // TODO: remove the hack of avoiding empty segments as special case, remove .is_empty() condition
        let is_pruned = |s: &StackSegment| !s.commits.is_empty() && !is_entrypoint_or_local(s);
        // Prune the whole stack if we start with unwanted segments.
        if out
            .first()
            .is_some_and(|s| is_pruned(s) || discard_stack(s))
        {
            tracing::warn!(
                "Ignoring stack {:?} ({:?}) as it is pruned",
                out.first().and_then(|s| s.ref_name.as_ref()),
                from,
            );
            return Ok(None);
        }

        // We may have picked up unwanted segments, if the graph isn't perfectly clean
        // TODO: remove this to rather assure that non-local branches aren't linked up that way.
        if let Some(new_len) = out
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_idx, s)| is_pruned(s))
            .last()
            .map(|(idx, _s)| idx)
        {
            out.truncate(new_len);
        }
        Ok((!out.is_empty()).then_some(out))
    }

    /// Visit all segments across all connections, including `start` and return the segment for which `f(segment)` returns `true`.
    /// There is no traversal pruning.
    pub(crate) fn find_segment_upwards(
        &self,
        start: SegmentIndex,
        mut f: impl FnMut(&Segment) -> bool,
    ) -> Option<&Segment> {
        let mut next = VecDeque::new();
        next.push_back(start);
        let mut seen = BTreeSet::new();
        while let Some(next_sidx) = next.pop_front() {
            let s = &self[next_sidx];
            if f(s) {
                return Some(s);
            }
            next.extend(
                self.inner
                    .neighbors_directed(next_sidx, Direction::Incoming)
                    .filter(|n| seen.insert(*n)),
            );
        }
        None
    }
}

/// More processing
impl Workspace<'_> {
    /// Match the archived flag from our workspace metadata by name with actual segments and prune them,
    /// top to bottom, but only if they are empty all the way down for safety.
    /// Doing so naturally shows segments that we have to show, independently of the archived flag.
    ///
    /// Match the archived flag by name, that's all we have.
    /// Note that we chose to not make `archived` intrusive and a member of the respective segment data
    /// despite other portions of the code possibly being in a good position to do that. Ultimately, they
    /// all match by name, and we just keep the 'archived' handling localised
    /// (possibly allowing it to be turned off, etc).
    fn prune_archived_segments(&mut self) {
        let Some(md) = &self.metadata else { return };
        let archived_stack_branches = md.stacks.iter().flat_map(|s| {
            s.branches
                .iter()
                .filter_map(|s| s.archived.then_some(s.ref_name.as_ref()))
        });
        for archived_ref_name in archived_stack_branches {
            let Some((stack_idx, segment_idx)) =
                self.find_segment_owner_indexes_by_refname(archived_ref_name)
            else {
                continue;
            };
            let stack = &mut self.stacks[stack_idx];
            let all_downwards_are_empty = stack.segments[segment_idx..]
                .iter()
                .all(|s| s.commits.is_empty());
            if !all_downwards_are_empty {
                continue;
            }
            stack.segments.truncate(segment_idx);
        }
    }

    /// Trace the remotes of each segments down to their segment or other segments and set the commit flags accordingly
    /// to indicate if a commit in the workspace is reachable, and how.
    fn mark_remote_reachability(&mut self) -> anyhow::Result<()> {
        let remote_refs: Vec<_> = self
            .stacks
            .iter()
            .flat_map(|s| {
                s.segments.iter().filter_map(|s| {
                    s.remote_tracking_ref_name
                        .as_ref()
                        .cloned()
                        .zip(s.sibling_segment_id)
                })
            })
            .collect();
        let graph = self.graph;
        for (remote_tracking_ref_name, remote_sidx) in remote_refs {
            let mut remote_commits = Vec::new();
            let mut may_take_commits_from_first_remote = graph[remote_sidx].commits.is_empty();
            graph.visit_all_segments_including_start_until(remote_sidx, Direction::Outgoing, |s| {
                let prune = !s.commits.iter().all(|c| c.flags.is_remote())
                    // Do not 'steal' commits from other known remote segments while they are officially connected,
                    // unless we started out empty. That means ambiguous ownership, as multiple remotes point
                    // to the same commit.
                    || {
                    let mut prune = s.id != remote_sidx
                        && s.ref_name
                        .as_ref()
                        .is_some_and(|orn| orn.category() == Some(Category::RemoteBranch));
                    if prune && may_take_commits_from_first_remote {
                        prune = false;
                        may_take_commits_from_first_remote = false;
                    }
                    prune
                };
                if prune {
                    // See if this segment links to a commit we know as local, and mark it accordingly,
                    // along with all segments in that stack.
                    for stack in &mut self.stacks {
                        let Some((first_segment, first_commit_index)) =
                            stack.segments.iter().enumerate().find_map(|(os_idx, os)| {
                                os.commits_by_segment
                                    .iter()
                                    .find_map(|(sidx, commit_ofs)| {
                                        (*sidx == s.id).then_some(commit_ofs)
                                    })
                                    .map(|commit_ofs| (os_idx, *commit_ofs))
                            })
                        else {
                            continue;
                        };

                        let mut first_commit_index = Some(first_commit_index);
                        for segment in &mut stack.segments[first_segment..] {
                            let remote_reachable_flags =
                                if segment.remote_tracking_ref_name.as_ref()
                                    == Some(&remote_tracking_ref_name)
                                {
                                    StackCommitFlags::ReachableByMatchingRemote
                                } else {
                                    StackCommitFlags::empty()
                                } | StackCommitFlags::ReachableByRemote;
                            for commit in &mut segment.commits
                                [first_commit_index.take().unwrap_or_default()..]
                            {
                                commit.flags |= remote_reachable_flags;
                            }
                        }
                        // keep looking - other stacks can repeat the segment!
                        continue;
                    }
                } else {
                    for commit in &s.commits {
                        remote_commits.push(StackCommit::from_graph_commit(commit));
                    }
                }
                prune
            });

            // Have to keep looking for matching segments, they can be mentioned multiple times.
            let mut found_segment = false;
            for local_segment_with_this_remote in self.stacks.iter_mut().flat_map(|stack| {
                stack.segments.iter_mut().filter_map(|s| {
                    (s.remote_tracking_ref_name.as_ref() == Some(&remote_tracking_ref_name))
                        .then_some(s)
                })
            }) {
                found_segment = true;
                local_segment_with_this_remote.commits_on_remote = remote_commits.clone();
            }
            if !found_segment {
                tracing::error!(
                    "BUG: Couldn't find local segment with remote tracking ref '{remote_tracking_ref_name}' - remote commits for it seem to be missing",
                );
            }
        }
        Ok(())
    }
}

/// Query
impl<'graph> Workspace<'graph> {
    /// Return `true` if this workspace is managed, meaning we control certain aspects of it.
    /// If `false`, we are more conservative and may not support all features.
    pub fn has_managed_ref(&self) -> bool {
        matches!(
            self.kind,
            WorkspaceKind::Managed { .. } | WorkspaceKind::ManagedMissingWorkspaceCommit { .. }
        )
    }

    /// Return `true` if the workspace has workspace metadata associated with it.
    /// This is relevant when creating references for example.
    pub fn has_metadata(&self) -> bool {
        self.metadata.is_some()
    }

    /// Return the name of the workspace reference by looking our segment up in `graph`.
    /// Note that for managed workspaces, this can be retrieved via [`WorkspaceKind::Managed`].
    /// Note that it can be expected to be set on any workspace, but the data would allow it to not be set.
    pub fn ref_name(&self) -> Option<&'graph gix::refs::FullNameRef> {
        self.graph[self.id].ref_name.as_ref().map(|rn| rn.as_ref())
    }

    /// Like [`Self::ref_name()`], but return a generic `<anonymous>` name for unnamed workspaces.
    pub fn ref_name_display(&self) -> &BStr {
        self.ref_name()
            .map_or("<anonymous>".into(), |rn| rn.as_bstr())
    }
}

/// Debugging
impl Workspace<'_> {
    /// Produce a distinct and compressed debug string to show at a glance what the workspace is about.
    pub fn debug_string(&self) -> String {
        let graph = self.graph;
        let (name, sign) = match &self.kind {
            WorkspaceKind::Managed { ref_name } => (Graph::ref_debug_string(ref_name), "🏘️"),
            WorkspaceKind::ManagedMissingWorkspaceCommit { ref_name } => {
                (Graph::ref_debug_string(ref_name), "🏘️⚠️")
            }
            WorkspaceKind::AdHoc => (
                graph[self.id]
                    .ref_name
                    .as_ref()
                    .map_or("DETACHED".into(), Graph::ref_debug_string),
                "⌂",
            ),
        };
        let target = self.target.as_ref().map_or_else(
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

impl std::fmt::Debug for Workspace<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Workspace({})", self.debug_string()))
            .field("id", &self.id.index())
            .field("kind", &self.kind)
            .field("stacks", &self.stacks)
            .field("metadata", &self.metadata)
            .field("target", &self.target)
            .field("extra_target", &self.extra_target)
            .finish()
    }
}
