use std::{
    cell::RefCell,
    collections::{BTreeSet, VecDeque},
};

use anyhow::Context;
use bstr::ByteSlice;
use but_core::ref_metadata::StackKind::{Applied, AppliedAndUnapplied};
use but_core::ref_metadata::{self, StackId};

use gix::refs::Category;
use itertools::Itertools;
use petgraph::{
    Direction,
    visit::{EdgeRef, NodeRef},
};
use tracing::instrument;

use crate::{
    CommitFlags, Graph, Segment, SegmentIndex,
    projection::{
        Stack, StackCommit, StackCommitFlags, StackSegment, TargetCommit, TargetRef, Workspace, WorkspaceKind,
        workspace::{WorkspaceState, find_segment_owner_indexes_by_refname},
    },
};

pub(crate) enum Downgrade {
    /// Allows to turn a workspace above a selection to be downgraded back to the selection if it turns
    /// out to be outside the workspace.
    /// This is typically what you want when producing a workspace for display, as the workspace then isn't relevant.
    Allow,
    /// Use this if the closest workspace is what you want, even if the reference in question is below the workspace lower bound.
    Disallow,
}

impl Graph {
    /// Analyze the current graph starting at its [entrypoint](Self::lookup_entrypoint()).
    ///
    /// No matter what, each location of `HEAD`, which corresponds to the entrypoint, can be represented as workspace.
    /// Further, the most expensive operations we perform to query additional commit information by reading it, but we
    /// only do so on the ones that the user can interact with.
    ///
    /// The [extra-target](crate::init::Options::with_extra_target_commit_id) option extends the workspace to include
    /// that target as base. The same is true for [target commit ids](but_core::ref_metadata::Workspace::target_commit_id).
    /// This affects what we consider to be the part of the workspace.
    /// Typically, that's a previous location of the target segment.
    #[instrument(name = "Graph::into_workspace", level = "debug", skip(self), err(Debug))]
    pub fn into_workspace(self) -> anyhow::Result<Workspace> {
        let state = self.to_workspace_state(Downgrade::Allow)?;
        Ok(Workspace::from_state(self, state))
    }

    pub(crate) fn to_workspace_state(&self, downgrade: Downgrade) -> anyhow::Result<WorkspaceState> {
        let (mut kind, mut metadata, mut ws_tip_segment, entrypoint_sidx, entrypoint_first_commit_flags) = {
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
                            WorkspaceKind::managed(&ws_segment.ref_info)?,
                            ws_segment.workspace_metadata().cloned(),
                            ws_segment,
                            Some(ep.segment_index),
                            maybe_integrated_flags,
                        )
                    } else {
                        (WorkspaceKind::AdHoc, None, ep.segment, None, CommitFlags::empty())
                    }
                }
                Some(meta) => (
                    WorkspaceKind::managed(&ep.segment.ref_info)?,
                    Some(meta.clone()),
                    ep.segment,
                    None,
                    CommitFlags::empty(),
                ),
            }
        };

        let mut target_ref = metadata
            .as_ref()
            .and_then(|md| TargetRef::from_ref_name_without_commits_ahead(md.target_ref.as_ref()?, self));
        let mut target_commit = metadata
            .as_ref()
            .and_then(|md| md.target_commit_id)
            .and_then(|target_commit_id| TargetCommit::from_commit(target_commit_id, self));
        let extra_target = self.extra_target;
        let mut id = ws_tip_segment.id;
        let mut stacks = vec![];

        let ws_lower_bound = if kind.has_managed_ref() {
            self.compute_lowest_base(
                ComputeBaseTip::WorkspaceCommit(id),
                target_ref.as_ref(),
                target_commit.as_ref(),
                self.extra_target,
            )
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
                        .reduce(|a, b| self.find_git_merge_base(a, b).unwrap_or(a))
                        .and_then(|base| self[base].commits.first().map(|c| (c.id, base)))
                }
            })
        } else {
            // Auto-set the target by its remote.
            if target_ref.is_none() {
                let ws_head_segment = &self[id];
                target_ref = ws_head_segment
                    .remote_tracking_ref_name
                    .as_ref()
                    .zip(ws_head_segment.remote_tracking_branch_segment_id)
                    .map(|(target_ref, target_sidx)| TargetRef {
                        ref_name: target_ref.to_owned(),
                        segment_index: target_sidx,
                        commits_ahead: 0,
                    });
            }
            if target_ref.is_some() || target_commit.is_some() || extra_target.is_some() {
                self.compute_lowest_base(
                    ComputeBaseTip::SingleBranch(id),
                    target_ref.as_ref(),
                    target_commit.as_ref(),
                    self.extra_target,
                )
            } else {
                None
            }
        };

        let (mut lower_bound, mut lower_bound_segment_id) =
            ws_lower_bound.map(|(a, b)| (Some(a), Some(b))).unwrap_or_default();

        // The entrypoint is integrated and has a workspace above it.
        // Right now we would be using it, but will discard it if the entrypoint is *at* or *below* the merge-base.
        if let Some(((_lowest_base, lowest_base_sidx), ep_sidx)) = ws_lower_bound
            .filter(|_| {
                matches!(downgrade, Downgrade::Allow) && entrypoint_first_commit_flags.contains(CommitFlags::Integrated)
            })
            .zip(entrypoint_sidx)
            && (ep_sidx == lowest_base_sidx
                || self
                    .find_map_downwards_along_first_parent(ep_sidx, |s| (s.id == lowest_base_sidx).then_some(()))
                    .is_none())
        {
            // We cannot reach the lowest workspace base, by definition reachable through any path downward,
            // so we are outside the workspace limits which is above us. Turn the data back into entrypoint-only.
            id = ep_sidx;
            kind = WorkspaceKind::AdHoc;
            target_ref = None;
            target_commit = None;
            metadata = None;
            ws_tip_segment = &self[ep_sidx];
            lower_bound = None;
            lower_bound_segment_id = None;
        }

        if kind.has_managed_ref() && self[id].commits.is_empty() {
            let ref_info = ws_tip_segment
                .ref_info
                .as_ref()
                .expect("BUG: must be set or we wouldn't be here");
            kind = WorkspaceKind::ManagedMissingWorkspaceCommit {
                ref_info: ref_info.clone(),
            };
        }

        fn segment_name_is_special(s: &Segment) -> bool {
            s.ref_name()
                .is_some_and(|rn| rn.as_bstr().starts_with_str("refs/heads/gitbutler/"))
        }

        let (lowest_base, lowest_base_sidx) =
            ws_lower_bound.map_or((None, None), |(base, sidx)| (Some(base), Some(sidx)));
        if kind.has_managed_ref() {
            let mut used_stack_ids = BTreeSet::default();
            for stack_top_sidx in self.inner.neighbors_directed(ws_tip_segment.id, Direction::Outgoing) {
                let stack_segment = &self[stack_top_sidx];
                let has_seen_base = RefCell::new(false);
                stacks.extend(
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
                            // named counterparts, and we want to set the name, but they must
                            // be in their own stack-segment.
                            if s.ref_info.is_none() && s.sibling_segment_id.is_some() {
                                return stop;
                            }
                            if segment_name_is_special(s) {
                                return !stop;
                            }
                            match (
                                &stack_segment.ref_info,
                                s.ref_name().filter(|rn| rn.category() == Some(Category::LocalBranch)),
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
                        |s| Some(s.id) == lower_bound_segment_id && s.metadata.is_none(),
                    )?
                    .and_then(|segments| {
                        let stack_id = find_matching_stack_id(metadata.as_ref(), &segments, &mut used_stack_ids);
                        // If we find no stack ID, then the segment is not included in the workspace metadata,
                        // indicating it's ignored. Just to be even more certain, if it starts with a commit
                        // that is the workspace base, then we definitely don't want to show it - it's unapplied.
                        if stack_id.is_none_or(|(_id, in_workspace)| !in_workspace)
                            && segments
                                .first()
                                .is_some_and(|s| s.commits.first().map(|c| c.id) == lowest_base)
                        {
                            None
                        } else {
                            Some(Stack::from_base_and_segments(
                                &self.inner,
                                segments,
                                stack_id.map(|(id, _in_workspace)| id),
                            ))
                        }
                    }),
                );
            }
        } else {
            let start = ws_tip_segment;
            let has_seen_base = RefCell::new(false);
            let maybe_stack = self
                .collect_stack_segments(
                    start.id,
                    None,
                    |s| {
                        let stop = true;
                        if segment_name_is_special(s) {
                            return !stop;
                        }
                        // Cut the stack off at the lower base if we have one. This is only
                        // the case if we have a remote.
                        if Some(s.id) == lowest_base_sidx {
                            has_seen_base.replace(true);
                            return stop;
                        }
                        match (&start.ref_info, &s.ref_info) {
                            (Some(_), Some(_)) | (None, Some(_)) => stop,
                            (Some(_), None) | (None, None) => !stop,
                        }
                    },
                    |_s| !*has_seen_base.borrow(),
                    // Never discard stacks
                    |_s| false,
                )?
                .map(|segments| {
                    Stack::from_base_and_segments(&self.inner, segments, Some(StackId::single_branch_id()))
                });
            if let Some(stack) = maybe_stack {
                stacks.push(stack);
            } else {
                tracing::warn!("Didn't get a single stack for AdHoc workspace - this is unexpected");
            }
        }

        if let Some(target) = target_ref.as_mut() {
            target.compute_and_set_commits_ahead(self, lower_bound_segment_id);
        }

        let mut ws = WorkspaceState {
            id,
            kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
            extra_target,
            metadata,
        };

        ws.prune_archived_segments();
        ws.mark_remote_reachability(self)?;
        ws.truncate_single_stack_to_match_base();
        Ok(ws)
    }

    /// Compute the lowest base (i.e. the highest generation) between the `tip` of a top-most segment of the workspace,
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
        tip: ComputeBaseTip,
        target_ref: Option<&TargetRef>,
        target_commit: Option<&TargetCommit>,
        additional: impl IntoIterator<Item = SegmentIndex>,
    ) -> Option<(gix::ObjectId, SegmentIndex)> {
        // It's important to not start from the tip, but instead find paths to the merge-base from each stack individually.
        // Otherwise, we may end up with a short path to a segment that isn't actually reachable by all stacks.
        let (tips, actual_tip) = match tip {
            ComputeBaseTip::WorkspaceCommit(ws_tip) => (
                self.inner.neighbors_directed(ws_tip, Direction::Outgoing).collect(),
                ws_tip,
            ),
            ComputeBaseTip::SingleBranch(tip) => (vec![tip], tip),
        };
        let mut count = 0;
        let all_segments = tips
            .into_iter()
            .chain(target_ref.map(|t| t.segment_index))
            .chain(target_commit.map(|t| t.segment_index))
            .chain(additional);

        let base = all_segments
            .inspect(|_| count += 1)
            .reduce(|a, b| self.find_git_merge_base(a, b).unwrap_or(a))?;

        if count < 2 || base == actual_tip {
            match tip {
                ComputeBaseTip::WorkspaceCommit(_) => {
                    // In workspace mode, we get natural results if we don't accept tips == base situations,
                    // which would mean the workspace tip is included in the target.
                    None
                }
                ComputeBaseTip::SingleBranch(_) => {
                    // In single-branch mode, and if the checkout branch is directly reachable from the target
                    // which typically is its remote, it should just be empty. Allow this for now, and see what happens.
                    self.first_commit_or_find_along_first_parent(base)
                        .map(|(c, sidx)| (c.id, sidx))
                }
            }
        } else {
            self.first_commit_or_find_along_first_parent(base)
                .map(|(c, sidx)| (c.id, sidx))
        }
    }
}

enum ComputeBaseTip {
    /// The tip is a workspace commit, and we should consider all of its stacks.
    WorkspaceCommit(SegmentIndex),
    /// Use the tip directly.
    SingleBranch(SegmentIndex),
}

/// This works as named segments have been created in a prior step. Thus, we are able to find best matches by
/// the amount of matching names, probably.
/// Note that we find applied stack-ids first, then try again with unapplied ones, and indicate if it was applied or not.
/// Update `seen` with the stack_id we find and avoid reusing seen stack ids.
fn find_matching_stack_id(
    metadata: Option<&ref_metadata::Workspace>,
    segments: &[StackSegment],
    seen: &mut BTreeSet<StackId>,
) -> Option<(StackId, bool)> {
    let metadata = metadata?;

    fn ref_names_with_weight(s: &StackSegment) -> impl Iterator<Item = (u64, &gix::refs::FullNameRef)> {
        s.ref_info
            .as_ref()
            .map(|ri| (100_000, ri.ref_name.as_ref()))
            .into_iter()
            .chain(
                s.commits
                    .iter()
                    .flat_map(|c| c.refs.iter().map(|ri| (1, ri.ref_name.as_ref()))),
            )
    }

    segments
        .iter()
        .flat_map(|s| {
            ref_names_with_weight(s).filter_map(|(weight, rn)| {
                metadata.stacks(AppliedAndUnapplied).find_map(|meta_stack| {
                    if let Some(bidx) = meta_stack
                        .branches
                        .iter()
                        .enumerate()
                        .find_map(|(bidx, b)| (rn == b.ref_name.as_ref()).then_some(bidx))
                    {
                        let priority = if bidx == 0 { 3 } else { 1 };
                        Some((
                            if meta_stack.is_in_workspace() {
                                weight * 2
                            } else {
                                weight
                            } * priority,
                            meta_stack.id,
                            meta_stack.is_in_workspace(),
                        ))
                    } else {
                        None
                    }
                })
            })
        })
        .sorted_by(|l, r| l.0.cmp(&r.0).reverse())
        .map(|(_weight, stack_id, in_workspace)| (stack_id, in_workspace))
        .find(|(stack_id, _)| seen.insert(*stack_id))
}

/// Traversals
impl Graph {
    /// Return the ancestry of `start` along the first parents, itself included, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    ///
    /// The `stop` signal is ignored for unnamed segments (no `ref_info`) whose `sibling_segment_id`
    /// points to a segment already collected in the output. This prevents the traversal from stopping
    /// at an ancestor-link segment that merely reconnects to a workspace branch we are already traversing.
    ///
    /// Note that the traversal assumes as well-segmented graph without cycles.
    fn collect_first_parent_segments_until<'a>(
        &'a self,
        start: &'a Segment,
        mut stop: impl FnMut(&Segment) -> bool,
    ) -> (Vec<&'a Segment>, Option<&'a Segment>) {
        let mut out = vec![start];
        let mut edge = self.inner.edges_directed(start.id, Direction::Outgoing).last();
        let mut stopped_at = None;
        let mut seen = BTreeSet::new();
        while let Some(first_edge) = edge {
            let next = &self[first_edge.target()];
            if stop(next)
                && !(next.ref_info.is_none()
                    && next
                        .sibling_segment_id
                        .is_some_and(|sid| out.iter().any(|s| s.id == sid)))
            {
                stopped_at = Some(next);
                break;
            }
            out.push(next);
            if seen.insert(next.id) {
                edge = self.inner.edges_directed(next.id, Direction::Outgoing).last();
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

    /// Return `(commit, owner_sidx_of_commit)` if `start` has a commit, or find the first commit downstream along the first parent.
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
        mut entrypoint_sidx: Option<SegmentIndex>,
        mut is_one_past_end_of_stack_segment: impl FnMut(&Segment) -> bool,
        mut starts_next_stack_segment: impl FnMut(&Segment) -> bool,
        mut discard_stack: impl FnMut(&StackSegment) -> bool,
    ) -> anyhow::Result<Option<Vec<StackSegment>>> {
        // TODO: Test what happens if a workspace commit is pointed at by a different ref (which is the entrypoint).
        let mut out = Vec::new();
        let mut next = Some(from);
        while let Some(from) = next.take() {
            let start = &self[from];
            let (segments, stopped_at) =
                self.collect_first_parent_segments_until(start, &mut is_one_past_end_of_stack_segment);
            let mut segment = StackSegment::from_graph_segments(&segments, self)?;
            if entrypoint_sidx.is_some_and(|id| segment.id == id) {
                segment.is_entrypoint = true;
                entrypoint_sidx = None;
            }
            out.push(segment);
            next = stopped_at.filter(|s| starts_next_stack_segment(s)).map(|s| s.id);
        }

        fn is_entrypoint_or_local(s: &StackSegment) -> bool {
            if s.is_entrypoint {
                return true;
            }
            s.ref_name()
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
        if out.first().is_some_and(|s| is_pruned(s) || discard_stack(s)) {
            tracing::warn!(
                "Ignoring stack {:?} ({:?}) as it is pruned",
                out.first().and_then(|s| s.ref_info.as_ref()),
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
impl WorkspaceState {
    /// Match the archived flag from our workspace metadata by name with actual segments and prune them,
    /// top to bottom, but only if they are empty all the way down for safety.
    /// Doing so naturally shows segments that we have to show, independently of the archived flag.
    ///
    /// Match the archived flag by name, that's all we have.
    /// Note that we chose to not make `archived` intrusive and a member of the respective segment data
    /// despite other portions of the code possibly being in a good position to do that. Ultimately, they
    /// all match by name, and we just keep the 'archived' handling localised
    /// (possibly allowing it to be turned off, etc).
    ///
    /// Remove the whole stack if everything is archived.
    fn prune_archived_segments(&mut self) {
        let Some(md) = &self.metadata else {
            return;
        };
        let archived_stack_branches = md.stacks(Applied).flat_map(|s| {
            s.branches
                .iter()
                .filter_map(|s| s.archived.then_some(s.ref_name.as_ref()))
        });
        let mut empty_stacks_to_remove = Vec::new();
        for archived_ref_name in archived_stack_branches {
            let Some((stack_idx, segment_idx)) = find_segment_owner_indexes_by_refname(&self.stacks, archived_ref_name)
            else {
                continue;
            };
            let stack = &mut self.stacks[stack_idx];
            let all_downwards_are_empty = stack.segments[segment_idx..].iter().all(|s| s.commits.is_empty());
            if !all_downwards_are_empty {
                continue;
            }
            stack.segments.truncate(segment_idx);
            if stack.segments.is_empty() {
                empty_stacks_to_remove.push(stack_idx);
            }
        }

        empty_stacks_to_remove.sort();
        for stack_idx_to_remove in empty_stacks_to_remove.into_iter().rev() {
            let stack = self.stacks.remove(stack_idx_to_remove);
            tracing::warn!(
                "Pruned stack {stack_id:?} from workspace as all its segments were archived",
                stack_id = stack.id
            )
        }
    }

    /// Trace the remotes of each segments down to their segment or other segments and set the commit flags accordingly
    /// to indicate if a commit in the workspace is reachable, and how.
    fn mark_remote_reachability(&mut self, graph: &Graph) -> anyhow::Result<()> {
        let remote_refs: Vec<_> = self
            .stacks
            .iter()
            .flat_map(|s| {
                s.segments.iter().filter_map(|s| {
                    s.remote_tracking_ref_name
                        .as_ref()
                        .cloned()
                        .zip(s.remote_tracking_branch_segment_id)
                })
            })
            .collect();
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
                        && s.ref_name()
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
                                    .find_map(|(sidx, commit_ofs)| (*sidx == s.id).then_some(commit_ofs))
                                    .map(|commit_ofs| (os_idx, *commit_ofs))
                            })
                        else {
                            continue;
                        };

                        let mut first_commit_index = Some(first_commit_index);
                        for segment in &mut stack.segments[first_segment..] {
                            let remote_reachable_flags =
                                if segment.remote_tracking_ref_name.as_ref() == Some(&remote_tracking_ref_name) {
                                    StackCommitFlags::ReachableByMatchingRemote
                                } else {
                                    StackCommitFlags::empty()
                                } | StackCommitFlags::ReachableByRemote;
                            for commit in &mut segment.commits[first_commit_index.take().unwrap_or_default()..] {
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
                    (s.remote_tracking_ref_name.as_ref() == Some(&remote_tracking_ref_name)).then_some(s)
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

    /// If there is a single stack and the base happens to be itself (which happens if the stack is directly integrated/inline with the target),
    /// then empty all commits and segment-related metadata.
    fn truncate_single_stack_to_match_base(&mut self) {
        if self.stacks.len() != 1 {
            return;
        }
        let Some(stack) = self.stacks.first_mut() else {
            return;
        };
        let stack_is_base = stack
            .segments
            .first()
            .zip(self.lower_bound_segment_id)
            .is_some_and(|(segment, base)|
                // We can go by branch ID as this also means the first commit is the one that is the base.
                // This should be fine, as these kinds of stacks/segments should have at least one commit.
                // There is no hard guarantee though, so let's see.
                segment.id == base);
        if !stack_is_base {
            return;
        }

        stack.segments.drain(1..);
        let first_segment = stack.segments.first_mut().expect("non-empty");
        first_segment.commits.clear();
        first_segment.commits_by_segment.clear();
    }
}
