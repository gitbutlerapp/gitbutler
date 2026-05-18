use std::{
    cell::RefCell,
    collections::{BTreeSet, HashSet},
};

use anyhow::Context;
use bstr::ByteSlice;
use but_core::ref_metadata::{
    self, StackId,
    StackKind::{Applied, AppliedAndUnapplied},
};
use gix::{ObjectId, refs::Category};
use itertools::Itertools;
use petgraph::{Direction, visit::NodeRef};
use tracing::instrument;

use crate::{
    CommitFlags, Graph, Segment, SegmentIndex, Workspace,
    workspace::{
        Stack, StackCommit, StackCommitFlags, StackSegment, TargetCommit, TargetRef, WorkspaceKind,
        workspace::{
            WorkspaceReconciliationInput, WorkspaceState, find_segment_owner_indexes_by_refname,
        },
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

/// Shared graph-level workspace analysis before projection-only cleanup.
///
/// `WorkspaceFrame` identifies the workspace tip, entrypoint relationship,
/// target-side traversal context, and lower bound. Final projection turns it
/// into [`WorkspaceState`] by collecting stacks and then applying display-only
/// pruning/enrichment. Reconciliation turns it into
/// [`WorkspaceReconciliationInput`] by collecting the same raw stack paths but
/// keeping only the fields needed to reshape graph segments before projection.
struct WorkspaceFrame {
    /// Workspace classifier derived from the entrypoint or containing workspace segment.
    kind: WorkspaceKind,
    /// Managed workspace metadata, if the frame is backed by a GitButler workspace ref.
    metadata: Option<ref_metadata::Workspace>,
    /// Segment that acts as the workspace tip for stack collection.
    ws_tip_segment_id: SegmentIndex,
    /// Original entrypoint segment when it is inside or below a containing workspace.
    entrypoint_sidx: Option<SegmentIndex>,
    /// Commit id of the computed workspace lower bound.
    lower_bound: Option<ObjectId>,
    /// Segment that owns the computed workspace lower-bound commit.
    lower_bound_segment_id: Option<SegmentIndex>,
    /// Resolved target ref used as the workspace integration frame.
    target_ref: Option<TargetRef>,
    /// Resolved target commit used as an additional lower-bound anchor.
    target_commit: Option<TargetCommit>,
}

/// Return whether `s` is named by an internal GitButler ref.
///
/// Stack collection normally treats local branch names as stack boundaries:
/// another local branch means another user-visible stack segment starts there.
/// Refs below `refs/heads/gitbutler/` are implementation refs, especially
/// workspace refs, and should not shape user-visible stacks. When collection
/// encounters such a segment it continues through it instead of stopping or
/// splitting the stack at that internal name.
fn segment_name_is_special(s: &Segment) -> bool {
    s.ref_name()
        .is_some_and(|rn| rn.as_bstr().starts_with_str("refs/heads/gitbutler/"))
}

impl Graph {
    /// Analyze the current graph starting at its [entrypoint](Self::entrypoint()).
    ///
    /// No matter what, each location of `HEAD`, which corresponds to the entrypoint, can be represented as workspace.
    /// Further, the most expensive operations we perform to query additional commit information by reading it, but we
    /// only do so on the ones that the user can interact with.
    ///
    /// Target commit ids and integrated traversal tips can extend the
    /// workspace to include these commits to define its lowest base.
    #[instrument(
        name = "Graph::into_workspace",
        level = "trace",
        skip(self),
        err(Debug)
    )]
    pub fn into_workspace(self) -> anyhow::Result<Workspace> {
        let state = self.to_workspace_state(Downgrade::Allow)?;
        Ok(Workspace::from_state(self, state))
    }

    pub(crate) fn to_workspace_state(
        &self,
        downgrade: Downgrade,
    ) -> anyhow::Result<WorkspaceState> {
        let frame = self.workspace_frame(downgrade)?;
        let stacks = self.workspace_stacks(&frame)?;
        let mut target_ref = frame.target_ref;

        if let Some(target) = target_ref.as_mut() {
            target.compute_and_set_commits_ahead(self, frame.lower_bound_segment_id);
        }

        let mut ws = WorkspaceState {
            id: frame.ws_tip_segment_id,
            kind: frame.kind,
            stacks,
            lower_bound: frame.lower_bound,
            lower_bound_segment_id: frame.lower_bound_segment_id,
            target_ref,
            target_commit: frame.target_commit,
            metadata: frame.metadata,
        };

        ws.prune_archived_segments();
        ws.prune_integrated_segments(self);
        ws.mark_remote_reachability(self)?;
        ws.add_commits_on_remote(self);
        ws.truncate_single_stack_to_match_base();
        Ok(ws)
    }

    pub(crate) fn workspace_reconciliation_input(
        &self,
    ) -> anyhow::Result<Option<WorkspaceReconciliationInput>> {
        let frame = self.workspace_frame(Downgrade::Disallow)?;
        let Some(metadata) = frame.metadata.clone() else {
            return Ok(None);
        };
        let stacks = self.workspace_stacks(&frame)?;
        Ok(Some(WorkspaceReconciliationInput {
            id: frame.ws_tip_segment_id,
            stacks,
            lower_bound_segment_id: frame.lower_bound_segment_id,
            target_ref: frame.target_ref,
            target_commit: frame.target_commit,
            metadata,
        }))
    }

    fn workspace_frame(&self, downgrade: Downgrade) -> anyhow::Result<WorkspaceFrame> {
        let (
            mut kind,
            mut metadata,
            mut ws_tip_segment_id,
            entrypoint_sidx,
            entrypoint_first_commit_flags,
        ) = {
            let ep = self.entrypoint()?;
            match ep.segment.workspace_metadata() {
                None => {
                    // Skip over empty segments.
                    if let Some((maybe_integrated_flags, sidx_of_flags)) = self
                        .resolve_to_unambiguously_pointed_to_commit(ep.segment.id)
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
                            ws_segment.id,
                            Some(ep.segment.id),
                            maybe_integrated_flags,
                        )
                    } else {
                        (
                            WorkspaceKind::AdHoc,
                            None,
                            ep.segment.id,
                            None,
                            CommitFlags::empty(),
                        )
                    }
                }
                Some(meta) => (
                    WorkspaceKind::managed(&ep.segment.ref_info)?,
                    Some(meta.clone()),
                    ep.segment.id,
                    None,
                    CommitFlags::empty(),
                ),
            }
        };

        let mut target_ref = metadata
            .as_ref()
            .and_then(|md| {
                TargetRef::from_ref_name_without_commits_ahead(md.target_ref.as_ref()?, self)
            })
            .or_else(|| self.integrated_tip_target_ref());
        let mut target_commit = metadata
            .as_ref()
            .and_then(|md| md.target_commit_id)
            .and_then(|target_commit_id| TargetCommit::from_commit(target_commit_id, self))
            .or_else(|| self.integrated_tip_target_commit(target_ref.as_ref()));
        let integrated_tip_segments =
            self.integrated_tip_segments_excluding_target_ref_tip(target_ref.as_ref());

        let ws_lower_bound = if kind.has_managed_ref() {
            self.compute_lowest_base(
                ComputeBaseTip::WorkspaceCommit(ws_tip_segment_id),
                target_ref.as_ref(),
                target_commit.as_ref(),
                &integrated_tip_segments,
            )
            .or_else(|| {
                // target not available? Try the base of the workspace itself
                if self
                    .inner
                    .neighbors_directed(ws_tip_segment_id, Direction::Outgoing)
                    .count()
                    == 1
                {
                    None
                } else {
                    self.find_best_effort_workspace_base(
                        self.inner
                            .neighbors_directed(ws_tip_segment_id, Direction::Outgoing),
                    )
                    .and_then(|base| self[base].commits.first().map(|c| (c.id, base)))
                }
            })
        } else {
            // Auto-set the target by its remote.
            if target_ref.is_none() {
                let ws_head_segment = &self[ws_tip_segment_id];
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
            if target_ref.is_some()
                || target_commit.is_some()
                || !integrated_tip_segments.is_empty()
            {
                self.compute_lowest_base(
                    ComputeBaseTip::SingleBranch(ws_tip_segment_id),
                    target_ref.as_ref(),
                    target_commit.as_ref(),
                    &integrated_tip_segments,
                )
            } else {
                None
            }
        };

        let (mut lower_bound, mut lower_bound_segment_id) = ws_lower_bound
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or_default();

        // The entrypoint is integrated and has a workspace above it.
        // Right now we would be using it, but will discard it if the entrypoint is *at* or *below* the merge-base.
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
            ws_tip_segment_id = ep_sidx;
            kind = WorkspaceKind::AdHoc;
            target_ref = None;
            target_commit = None;
            metadata = None;
            lower_bound = None;
            lower_bound_segment_id = None;
        }

        if kind.has_managed_ref() && self[ws_tip_segment_id].commits.is_empty() {
            let ref_info = self[ws_tip_segment_id]
                .ref_info
                .as_ref()
                .expect("BUG: must be set or we wouldn't be here");
            kind = WorkspaceKind::ManagedMissingWorkspaceCommit {
                ref_info: ref_info.clone(),
            };
        }

        Ok(WorkspaceFrame {
            kind,
            metadata,
            ws_tip_segment_id,
            entrypoint_sidx,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
        })
    }

    fn workspace_stacks(&self, frame: &WorkspaceFrame) -> anyhow::Result<Vec<Stack>> {
        let mut stacks = vec![];
        let (lowest_base, lowest_base_sidx) = (frame.lower_bound, frame.lower_bound_segment_id);
        if frame.kind.has_managed_ref() {
            let mut used_stack_ids = BTreeSet::default();
            for stack_top_sidx in self
                .inner
                .neighbors_directed(frame.ws_tip_segment_id, Direction::Outgoing)
            {
                let stack_segment = &self[stack_top_sidx];
                let has_seen_base = RefCell::new(false);
                stacks.extend(
                    self.collect_stack_segments(
                        stack_top_sidx,
                        frame.entrypoint_sidx,
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
                            if s.id != stack_top_sidx && Some(s.id) == frame.entrypoint_sidx {
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
                                s.ref_name()
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
                                    .all(|n| n.id() != frame.ws_tip_segment_id)
                        },
                        |s| Some(s.id) == frame.lower_bound_segment_id && s.metadata.is_none(),
                    )?
                    .and_then(|segments| {
                        let stack_id = find_matching_stack_id(
                            frame.metadata.as_ref(),
                            &segments,
                            &mut used_stack_ids,
                        );
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
            let start = &self[frame.ws_tip_segment_id];
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
                    Stack::from_base_and_segments(
                        &self.inner,
                        segments,
                        Some(StackId::single_branch_id()),
                    )
                });
            if let Some(stack) = maybe_stack {
                stacks.push(stack);
            } else {
                tracing::warn!(
                    "Didn't get a single stack for AdHoc workspace - this is unexpected"
                );
            }
        }
        Ok(stacks)
    }

    /// Compute the lowest base (i.e. the highest generation) for the
    /// workspace projection.
    ///
    /// `tip` identifies the workspace side. For a workspace commit, its direct
    /// outgoing segments are used as stack tips; for a single-branch workspace,
    /// the branch segment itself is used. `target_ref` and `target_commit`
    /// identify the ordinary target side and are folded with the workspace
    /// stack tips using the legacy pairwise merge-base behavior: candidates
    /// are folded in order, and if a candidate pair has no merge-base, the
    /// previous base candidate is kept instead of clearing the workspace base.
    /// `integrated_tip_segments` are tips of interest that represent integrated
    /// or past target positions. They are considered only after the ordinary
    /// base is found, and can lower that base so the workspace does not appear
    /// to lose stacks merely because they are now reachable from target tips.
    ///
    /// Returns `Some((lowest_base, segment_idx_with_lowest_base))`.
    ///
    /// ## Note
    ///
    /// This is a best-effort merge-base fold for workspace lower-bound compatibility.
    ///
    /// Target refs and target commits preserve the legacy pairwise fold
    /// behavior. Integrated tips then lower that base if they are farther down
    /// the common history.
    // TODO: actually compute the lowest base, see `first_merge_base()` which should be `lowest_merge_base()` by itself,
    //       accounting for finding the lowest of all merge-bases which would be assumed to be reachable by all segments
    //       searching downward, a necessary trait for many search problems.
    fn compute_lowest_base(
        &self,
        tip: ComputeBaseTip,
        target_ref: Option<&TargetRef>,
        target_commit: Option<&TargetCommit>,
        integrated_tip_segments: &[SegmentIndex],
    ) -> Option<(ObjectId, SegmentIndex)> {
        // It's important to not start from the tip, but instead find paths to the merge-base from each stack individually.
        // Otherwise, we may end up with a short path to a segment that isn't actually reachable by all stacks.
        let (tips, actual_tip) = match tip {
            ComputeBaseTip::WorkspaceCommit(ws_tip) => (
                self.inner
                    .neighbors_directed(ws_tip, Direction::Outgoing)
                    .collect(),
                ws_tip,
            ),
            ComputeBaseTip::SingleBranch(tip) => (vec![tip], tip),
        };
        let mut count = 0;
        let base_segments = tips
            .iter()
            .copied()
            .chain(target_ref.map(|t| t.segment_index))
            .chain(target_commit.map(|t| t.segment_index))
            .chain(integrated_tip_segments.iter().copied());

        let base = self.find_best_effort_workspace_base(base_segments.inspect(|_| count += 1))?;

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
                    self.resolve_to_unambiguously_pointed_to_commit(base)
                        .map(|(c, sidx)| (c.id, sidx))
                }
            }
        } else {
            self.resolve_to_unambiguously_pointed_to_commit(base)
                .map(|(c, sidx)| (c.id, sidx))
        }
    }

    /// Fold pairwise merge-bases for workspace lower-bound projection.
    ///
    /// A workspace lower-bound is used to frame legacy presentation and mutation compatibility.
    /// Historically, disjoint inputs kept the previous candidate instead of clearing the lower
    /// bound, so keep that behavior local to workspace projection rather than weakening
    ///[`Self::find_merge_base_octopus()`].
    fn find_best_effort_workspace_base(
        &self,
        segments: impl IntoIterator<Item = SegmentIndex>,
    ) -> Option<SegmentIndex> {
        segments
            .into_iter()
            .reduce(|base, segment| self.find_merge_base(base, segment).unwrap_or(base))
    }

    pub(super) fn integrated_tip_segments(&self) -> Vec<SegmentIndex> {
        self.integrated_tip_segments_excluding_target_ref_tip(None)
    }

    /// Return target-remote tip segments that provide extra target context.
    ///
    /// These are resolved from effective traversal tips with
    /// [`crate::init::TipRole::TargetRemote`]. They are used as additional
    /// lower-bound candidates and as a signal that integrated commits should
    /// not be pruned from the workspace projection yet.
    ///
    /// If `target_ref` is provided, its own tip commit is excluded. The target
    /// ref is already represented separately as [`TargetRef`], so including
    /// the same commit again would make an ordinary configured target look like
    /// additional target context. Distinct target-remote tips, including lower
    /// extra targets, remain in the returned list.
    fn integrated_tip_segments_excluding_target_ref_tip(
        &self,
        target_ref: Option<&TargetRef>,
    ) -> Vec<SegmentIndex> {
        self.workspace_projection_target_remote_tips()
            .filter_map(|tip| {
                TargetCommit::from_commit(tip.id, self)
                    .filter(|target| {
                        !self.target_ref_points_to_commit(target_ref, target.commit_id)
                    })
                    .map(|target| target.segment_index)
            })
            .unique()
            .collect()
    }

    /// Return the first named integrated tip that can act as the workspace's target ref.
    ///
    /// This is needed for graphs built with [`Graph::from_commit_traversal_tips()`], where callers
    /// can provide a named [`crate::init::TipRole::TargetRemote`] target without workspace metadata. In that mode
    /// there is no configured `target_ref` to resolve, but workspace projection can still expose the
    /// named integrated tip as the target ref for presentation and target-related queries.
    fn integrated_tip_target_ref(&self) -> Option<TargetRef> {
        if self.has_workspace_metadata_tip() {
            return None;
        }
        self.workspace_projection_target_remote_tips()
            .filter_map(|tip| tip.ref_name.as_ref())
            .find_map(|ref_name| TargetRef::from_ref_name_without_commits_ahead(ref_name, self))
    }

    /// Return the *lowest* target-remote tip that can act as the workspace's effective target commit.
    ///
    /// This is needed for graphs built with [`Graph::from_commit_traversal_tips()`], where callers
    /// can provide an explicit [`crate::init::TipRole::TargetRemote`] target without workspace metadata. In that
    /// mode there is no stored `target_commit_id` to resolve, but workspace projection still needs a
    /// target commit to frame the lower bound and workspace view. When multiple target remotes are
    /// available, choose the lowest one, i.e. the one with the highest segment generation.
    fn integrated_tip_target_commit(&self, target_ref: Option<&TargetRef>) -> Option<TargetCommit> {
        self.workspace_projection_target_remote_tips()
            .filter_map(|tip| TargetCommit::from_commit(tip.id, self))
            .filter(|target| !self.target_ref_points_to_commit(target_ref, target.commit_id))
            .max_by_key(|target| self[target.segment_index].generation)
    }

    /// Target-remote traversal tips that workspace projection can use as target context.
    ///
    /// `Graph::traversal_tips` stores every effective traversal tip. Workspace
    /// projection treats all target-remote tips the same here, named or
    /// anonymous, and lets graph position decide which one is useful for lower
    /// bound computation. Workspace metadata still wins for configured
    /// `target_ref` and stored `target_commit_id`; these tips are fallback
    /// target context derived from traversal.
    // TDOO: `traversal_tips` include the tips discovered in workspace metadata already, so the project code
    //       doesn't have to access them specifically.
    fn workspace_projection_target_remote_tips(&self) -> impl Iterator<Item = &crate::init::Tip> {
        self.traversal_tips
            .iter()
            .filter(|tip| tip.role.is_integrated())
    }

    /// Return `true` if there is a traversal tip with workspace metadata attached.
    fn has_workspace_metadata_tip(&self) -> bool {
        self.traversal_tips
            .iter()
            .any(|tip| matches!(tip.metadata, Some(crate::SegmentMetadata::Workspace(_))))
    }

    /// Return whether `target_ref` resolves to `commit_id` in the final graph.
    ///
    /// `target_ref` is the configured or inferred target branch, while
    /// `commit_id` comes from a target-remote traversal tip. These can differ:
    /// the tip may be an extra target/lower-bound commit, a persisted target
    /// commit, or another target-remote root that is not the branch tip. Only
    /// when both resolve to the same commit is the target-remote tip redundant
    /// with the target ref and safe to ignore as additional target context.
    fn target_ref_points_to_commit(
        &self,
        target_ref: Option<&TargetRef>,
        commit_id: gix::ObjectId,
    ) -> bool {
        target_ref
            .and_then(|target| self.tip_skip_empty(target.segment_index))
            .is_some_and(|commit| commit.id == commit_id)
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

    fn ref_names_with_weight(
        s: &StackSegment,
    ) -> impl Iterator<Item = (u64, &gix::refs::FullNameRef)> {
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
        let mut out = vec![start.id];
        let mut stopped_at = None;
        self.visit_segments_downward_along_first_parent_exclude_start(start.id, |next| {
            if stop(next)
                && !(next.ref_info.is_none()
                    && next
                        .sibling_segment_id
                        .is_some_and(|sid| out.contains(&sid)))
            {
                stopped_at = Some(next.id);
                return true;
            }
            out.push(next.id);
            false
        });
        (
            out.into_iter().map(|sidx| &self[sidx]).collect(),
            stopped_at.map(|sidx| &self[sidx]),
        )
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
        let mut out = Vec::new();
        let mut next = Some(from);
        while let Some(from) = next.take() {
            let start = &self[from];
            let (segments, stopped_at) = self
                .collect_first_parent_segments_until(start, &mut is_one_past_end_of_stack_segment);
            let mut segment = StackSegment::from_graph_segments(&segments, self)?;
            if entrypoint_sidx.is_some_and(|id| segment.id == id) {
                segment.is_entrypoint = true;
                entrypoint_sidx = None;
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
            s.ref_name()
                .and_then(|rn| rn.category())
                .is_none_or(|c| c == Category::LocalBranch)
        }

        let is_pruned = |s: &StackSegment| !is_entrypoint_or_local(s);
        // Prune the whole stack if we start with unwanted segments.
        if out
            .first()
            .is_some_and(|s| is_pruned(s) || discard_stack(s))
        {
            tracing::warn!(
                "Ignoring stack {:?} ({:?}) as it is pruned",
                out.first().and_then(|s| s.ref_info.as_ref()),
                from,
            );
            return Ok(None);
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
        let mut out = None;
        self.visit_all_segments_including_start_until(start, Direction::Incoming, |s| {
            if f(s) {
                out = Some(s.id);
                true
            } else {
                false
            }
        });
        out.map(|sidx| &self[sidx])
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
            let Some((stack_idx, segment_idx)) =
                find_segment_owner_indexes_by_refname(&self.stacks, archived_ref_name)
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

    /// Remove integrated commits and empty branches at the bottom of each
    /// stack, but only those at or below the workspace's target commit.
    /// Integrated commits above the target commit are kept until the user advances
    /// the target via upstream integration.
    // TODO: we need per-stack target commits/stored-forkpoint to be able to apply
    //       branches from the future onto workspaces from the past without showing
    //       too many integrated commits.
    //       Also, this implementation doesn't adjust the base yet, which leaves the
    //       workspace inconcistent. This can have repercussions for reference creation
    //       which currently uses the lowest bound, but this may change.
    fn prune_integrated_segments(&mut self, graph: &Graph) {
        if !graph
            .integrated_tip_segments_excluding_target_ref_tip(self.target_ref.as_ref())
            .is_empty()
            || self.target_ref.is_none()
        {
            return;
        }
        // TODO: it seems like we assume this is the lowest commit,
        //       but don't chose by generation.
        let target_segment_index = if let Some(tc) = self.target_commit.as_ref() {
            tc.segment_index
        } else if let Some(tr) = self.target_ref.as_ref() {
            tr.segment_index
        } else {
            return;
        };

        // Collect all segments that are the target segment itself or ancestors
        // of it by walking the graph toward its ancestors (Direction::Outgoing).
        let mut target_or_below_segments = HashSet::new();
        graph.visit_all_segments_excluding_start_until(
            target_segment_index,
            Direction::Outgoing,
            |s| {
                target_or_below_segments.insert(s.id);
                false
            },
        );
        target_or_below_segments.insert(target_segment_index);

        let metadata = self.metadata.as_ref();
        for stack in &mut self.stacks {
            prune_integrated_stack_segments(stack, &target_or_below_segments);
            remove_empty_branches(stack, metadata);
        }
        self.stacks.retain(|stack| !stack.segments.is_empty());
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
                }
                prune
            });
        }
        Ok(())
    }

    /// For each local segment that has a remote tracking branch, walk the remote
    /// side and collect commits that exist on the remote but not locally:
    /// - commits that are purely remote (never existed locally or pre-rebase versions), and
    /// - non-integrated commits from upper stack segments that are still on the
    ///   remote (the "branch split" case — a previously combined push left the
    ///   remote pointing at commits that now belong to branch above it).
    fn add_commits_on_remote(&mut self, graph: &Graph) {
        for stack in &mut self.stacks {
            let mut above_commit_ids = HashSet::new();
            for seg_idx in 0..stack.segments.len() {
                let Some(rsidx) = stack.segments[seg_idx].remote_tracking_branch_segment_id else {
                    // Still accumulate this segment's commits for lower segments.
                    above_commit_ids.extend(stack.segments[seg_idx].commits.iter().map(|c| c.id));
                    continue;
                };

                // All-parents walk: collect commits from *fully*-remote segments.
                // Stop at segments that contain non-remote commits or that belong
                // to another remote-branch, unless this segment is empty and
                // the first reachable remote commits can't be uniquely attributed.
                // This happens if multiple remote tracking branches point to the same commit,
                // which is when ours might be a virtual segment because it was traversed after
                // the segment that was prioritized to own the commit.
                // So `may_take_from_first_remote` allows us to pretend that these commits
                // belong to our remote (which they do as well from a pure graph perspective).
                let mut may_take_from_first_remote = graph[rsidx].commits.is_empty();
                let mut remote_commits = Vec::new();
                graph.visit_all_segments_including_start_until(
                    rsidx,
                    Direction::Outgoing,
                    |segment| {
                        if !segment.commits.iter().all(|c| c.flags.is_remote()) {
                            return true;
                        }
                        if segment.id != rsidx
                            && segment
                                .ref_name()
                                .is_some_and(|rn| rn.category() == Some(Category::RemoteBranch))
                        {
                            if may_take_from_first_remote {
                                may_take_from_first_remote = false;
                            } else {
                                return true;
                            }
                        }
                        for commit in &segment.commits {
                            remote_commits.push(StackCommit::from_graph_commit(commit));
                        }
                        false
                    },
                );

                // First-parent walk: detect non-integrated commits from upper
                // stack segments that are still reachable by the remote tracking branch.
                if !above_commit_ids.is_empty() {
                    let mut seen: HashSet<_> = remote_commits.iter().map(|c| c.id).collect();
                    let mut extra = Vec::new();
                    graph.visit_segments_downward_along_first_parent_exclude_start(rsidx, |s| {
                        if s.ref_name()
                            .is_some_and(|rn| rn.category() == Some(Category::RemoteBranch))
                        {
                            return true;
                        }
                        for commit in &s.commits {
                            if above_commit_ids.contains(&commit.id)
                                && !commit.flags.contains(CommitFlags::Integrated)
                                && seen.insert(commit.id)
                            {
                                extra.push(StackCommit::from_graph_commit(commit));
                            }
                        }
                        false
                    });
                    remote_commits.extend(extra);
                }

                stack.segments[seg_idx].commits_on_remote = remote_commits;

                // Accumulate this segment's commits for lower segments.
                above_commit_ids.extend(stack.segments[seg_idx].commits.iter().map(|c| c.id));
            }
        }
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

/// Prune whole integrated graph segments at the bottom, but only those that are
/// at or below the target segment. Integrated segments above the target are kept
/// until the user advances the target via upstream integration.
fn prune_integrated_stack_segments(
    stack: &mut Stack,
    target_or_below_segments: &HashSet<SegmentIndex>,
) {
    // Walk stack segments bottom-up, then graph-segment blocks bottom-up within
    // each stack segment. Stop at the first graph segment block that is either
    // not fully integrated or not at or below the target.
    let mut cut: Option<(usize, usize)> = None;
    'outer: for seg_idx in (0..stack.segments.len()).rev() {
        let seg = &stack.segments[seg_idx];

        if seg.commits.is_empty() {
            continue;
        }

        if seg.commits_by_segment.is_empty() {
            if commits_are_integrated(&seg.commits) && target_or_below_segments.contains(&seg.id) {
                cut = Some((seg_idx, 0));
                continue;
            }
            break 'outer;
        }

        for block_idx in (0..seg.commits_by_segment.len()).rev() {
            let (segment_id, start_offset) = seg.commits_by_segment[block_idx];
            let end_offset = seg
                .commits_by_segment
                .get(block_idx + 1)
                .map_or(seg.commits.len(), |(_, offset)| *offset);
            let commits = &seg.commits[start_offset..end_offset];

            if target_or_below_segments.contains(&segment_id) && commits_are_integrated(commits) {
                cut = Some((seg_idx, start_offset));
            } else {
                break 'outer;
            }
        }
    }

    let Some((cut_seg_idx, cut_offset)) = cut else {
        return;
    };

    stack.segments[cut_seg_idx].commits.truncate(cut_offset);
    stack.segments[cut_seg_idx]
        .commits_by_segment
        .retain(|(_, offset)| *offset < cut_offset);

    // Remove all stack segments below the cut. If the cut emptied the topmost
    // stack segment, keep it so `remove_empty_branches` can decide whether its
    // branch ref should be preserved, e.g. a metadata-tracked branch at the fork point.
    let keep = if stack.segments[cut_seg_idx].commits.is_empty() && cut_seg_idx > 0 {
        cut_seg_idx
    } else {
        cut_seg_idx + 1
    };
    stack.segments.truncate(keep);
}

fn commits_are_integrated(commits: &[StackCommit]) -> bool {
    commits
        .iter()
        .all(|commit| commit.flags.contains(StackCommitFlags::Integrated))
}

/// Remove empty segments unless they are mentioned in workspace metadata
/// (e.g. a branch the user just added at the fork point with no commits yet).
fn remove_empty_branches(stack: &mut Stack, metadata: Option<&but_core::ref_metadata::Workspace>) {
    let own_metadata_stack = stack.id.and_then(|stack_id| {
        metadata.and_then(|meta| meta.stacks(Applied).find(|ms| ms.id == stack_id))
    });
    stack.segments.retain(|seg| {
        !seg.commits.is_empty()
            || own_metadata_stack.is_some_and(|ms| {
                seg.ref_info
                    .as_ref()
                    // NOTE: `!b.archived` compensates for `prune_archived_segments`
                    // running *before* integrated-commit pruning — archived segments
                    // that still had commits are skipped there, then emptied here.
                    // Once metadata is kept trimmed and up-to-date we can drop this.
                    .is_some_and(|ri| {
                        ms.branches
                            .iter()
                            .any(|b| b.ref_name == ri.ref_name && !b.archived)
                    })
            })
    });
}
