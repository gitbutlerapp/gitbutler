use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, bail};
use but_core::{
    RefMetadata, ref_metadata,
    ref_metadata::StackKind::{Applied, AppliedAndUnapplied},
};
use gix::{ObjectId, prelude::ObjectIdExt, reference::Category};
use itertools::Itertools;
use petgraph::{Direction, graph::NodeIndex, prelude::EdgeRef, visit::NodeRef};
use tracing::instrument;

use crate::{
    Commit, CommitFlags, CommitIndex, Edge, Graph, SegmentIndex, SegmentMetadata,
    init::{
        PetGraph, branch_segment_from_name_and_meta,
        overlay::{OverlayMetadata, OverlayRepo},
        remotes,
        types::{EdgeOwned, TopoWalk},
        walk::{RefsById, WorktreeByBranch, disambiguate_refs_by_branch_metadata},
    },
    projection::workspace,
};

pub(super) struct Context<'a> {
    pub repo: &'a OverlayRepo<'a>,
    pub symbolic_remote_names: &'a [String],
    pub configured_remote_tracking_branches: &'a BTreeSet<gix::refs::FullName>,
    pub inserted_proxy_segments: Vec<SegmentIndex>,
    pub refs_by_id: RefsById,
    pub hard_limit: bool,
    pub dangerously_skip_postprocessing_for_debugging: bool,
    pub worktree_by_branch: WorktreeByBranch,
}

impl Context<'_> {
    pub(super) fn with_hard_limit(mut self) -> Self {
        self.hard_limit = true;
        self
    }
}

/// Processing
impl Graph {
    /// Now that the graph is complete, perform additional structural improvements with
    /// the requirement of them to be computationally cheap.
    #[instrument(level = "trace", skip(self, meta, repo, refs_by_id), err(Debug))]
    pub(super) fn post_processed<T: RefMetadata>(
        mut self,
        meta: &OverlayMetadata<'_, T>,
        tip: gix::ObjectId,
        Context {
            repo,
            symbolic_remote_names,
            configured_remote_tracking_branches,
            inserted_proxy_segments,
            refs_by_id,
            hard_limit,
            dangerously_skip_postprocessing_for_debugging,
            worktree_by_branch,
        }: Context<'_>,
    ) -> anyhow::Result<Self> {
        self.hard_limit_hit = hard_limit;

        // For the first id to be inserted into our entrypoint segment, set commit index.
        self.update_entrypoint_commit_index(tip);

        if dangerously_skip_postprocessing_for_debugging {
            return Ok(self);
        }

        // Before anything, cleanup the graph.
        self.fixup_remote_tracking_refs_and_maybe_split_segments(meta, &worktree_by_branch)?;

        // This should be first as what follows could help name these new segments that it creates.
        self.fixup_workspace_segments(repo, &refs_by_id, meta, &worktree_by_branch)?;
        // All non-workspace fixups must come first, otherwise the workspace handling might
        // differ as it relies on non-anonymous segments much more.
        self.fixup_segment_names(meta, &inserted_proxy_segments, &worktree_by_branch);
        // We perform view-related updates here for convenience, but also because the graph
        // traversal should have nothing to do with workspace details. It's just about laying
        // the foundation for figuring out our workspaces more easily.
        self.workspace_upgrades(meta, repo, &refs_by_id, &worktree_by_branch)?;

        // Point entrypoint to the right spot after all the virtual branches were added.
        self.set_entrypoint_to_ref_name(meta)?;
        // After the entrypoint changed, also update its commit-index again, for good measure.
        self.update_entrypoint_commit_index(tip);

        // However, when it comes to using remotes to disambiguate, it's better to
        // *not* do that before workspaces are sorted as it might incorrectly place
        // a segment on top of another one, setting a first-second relationship that isn't
        // what we have in the workspace metadata, which then also can't set it anymore
        // because it can't reorder existing empty segments (which are not natural).
        self.improve_remote_segments(
            repo,
            symbolic_remote_names,
            configured_remote_tracking_branches,
            &worktree_by_branch,
        )?;

        // Finally, once all segments were added, it's good to generations
        // have to figure out early abort conditions, or to know what's ahead of another.
        self.compute_generation_numbers();
        Ok(self)
    }

    /// Ensure the entrypoint commit-index is updated to match the actual tip commit.
    /// This may get out of sync when we do our graph manipulations, and it's easier
    /// to set it right in post than to let everything deal with this.
    fn update_entrypoint_commit_index(&mut self, tip: ObjectId) {
        if let Some((segment, ep_commit)) = self.entrypoint.as_mut() {
            *ep_commit = self
                .inner
                .node_weight(*segment)
                .and_then(|s| s.commit_index_of(tip));
        }
    }

    /// After everything, assure the entrypoint still points to a segment with the correct ref-name,
    /// if one was given when starting the traversal.
    /// If not, try to find a segment with the right ref-name.
    ///
    /// *This is the brute-force way of doing it, instead of ensuring that the workspace upgrade functions
    /// that create independent and dependent branches keep everything up-to-date at all times.
    fn set_entrypoint_to_ref_name<T: RefMetadata>(
        &mut self,
        meta: &OverlayMetadata<'_, T>,
    ) -> anyhow::Result<()> {
        let Some(((ep_sidx, _commit_idx), desired_ref_name)) =
            self.entrypoint.zip(self.entrypoint_ref.clone())
        else {
            return Ok(());
        };

        let ep_segment_is_correctly_named = self[ep_sidx]
            .ref_name()
            .is_some_and(|rn| rn == desired_ref_name.as_ref());
        if ep_segment_is_correctly_named {
            return Ok(());
        }

        let (sidx_with_desired_name, sidx_with_first_commit_with_desired_name) = self
            .node_weights()
            .find_map(|s| {
                s.ref_name()
                    .is_some_and(|rn| rn == desired_ref_name.as_ref())
                    .then_some((Some(s.id), None))
                    .or_else(|| {
                        s.commits.first().and_then(|c| {
                            c.refs
                                .iter()
                                .position(|ri| ri.ref_name == desired_ref_name)
                                .map(|pos| (None, Some((s.id, pos))))
                        })
                    })
            })
            .unwrap_or_default();
        if let Some(new_ep_sidx) = sidx_with_desired_name {
            let assume_tip_is_not_available_in_segment_anymore = None;
            self.entrypoint = Some((new_ep_sidx, assume_tip_is_not_available_in_segment_anymore));
        } else if let Some((new_ep_sidx, ref_idx)) = sidx_with_first_commit_with_desired_name {
            let s = &mut self.inner[new_ep_sidx];
            let desired_ref_name = s
                .commits
                .first_mut()
                .context("BUG: we have ref_idx because the first commit was checked")?
                .refs
                .remove(ref_idx);
            if s.ref_info.is_some() {
                // ref-name is known to not be the desired one, and the first commit of this segment has the name
                // we seek. For that, we now create a new
                let new_ep_sidx_first_commit_idx = s.commits.first().map(|_| 0);
                let incoming_edges = collect_edges_at_commit_reverse_order(
                    &self.inner,
                    (new_ep_sidx, new_ep_sidx_first_commit_idx),
                    Direction::Incoming,
                );
                let mut entrypoint_segment = branch_segment_from_name_and_meta(
                    Some((desired_ref_name.ref_name, None)),
                    meta,
                    None,
                    &Default::default(),
                )?;
                if let Some(ri) = entrypoint_segment.ref_info.as_mut() {
                    ri.worktree = desired_ref_name.worktree;
                }
                let entrypoint_sidx = self.insert_segment_set_entrypoint(entrypoint_segment);
                self.connect_segments(
                    entrypoint_sidx,
                    None,
                    new_ep_sidx,
                    new_ep_sidx_first_commit_idx,
                );
                for edge in incoming_edges {
                    self.inner.add_edge(
                        edge.source,
                        entrypoint_sidx,
                        Edge {
                            src: edge.weight.src,
                            src_id: edge.weight.src_id,
                            dst: None,
                            dst_id: None,
                        },
                    );
                    self.inner.remove_edge(edge.id);
                }
                self.entrypoint = Some((entrypoint_sidx, None));
            } else {
                self.entrypoint = Some((new_ep_sidx, Some(0)));
                // It's really important to get the name, as the HEAD is pointing to this segment.
                // So find the segment with the ambiguous ref we desire, and rewrite it to be non-ambiguous.
                // The reason we wait till now is to not disturb the workspace upgrades, which act differently
                // if they already have a named segment.
                // Note that any ref type works here, we just do as we are told even though tags for instance aren't usually
                // pointed to by HEAD.
                s.ref_info = Some(desired_ref_name);
            }
        } else {
            tracing::warn!(
                "Couldn't find any segment that was named after the entrypoint ref name or contained its name '{desired_ref_name}'",
            );
        };
        Ok(())
    }

    /// This is a post-process as only in the end we are sure what is a remote commit.
    /// On remote commits, we want to further segment remote tracking segments to avoid picking
    /// up too many remote commits later.
    /// For everything else, we want to just remove the extra ref-names that we aren't interested in.
    fn fixup_remote_tracking_refs_and_maybe_split_segments<T: RefMetadata>(
        &mut self,
        meta: &OverlayMetadata<'_, T>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        let mut split_info = Vec::new();
        for node in self.node_weights_mut() {
            let node_has_commits = node.commits.len() > 1;
            for (cidx, commit_with_refs) in node
                .commits
                .iter_mut()
                .enumerate()
                .filter_map(|t| (!t.1.refs.is_empty()).then_some(t))
            {
                let is_splittable =
                    commit_with_refs.flags.is_remote() && node_has_commits && cidx > 0;
                commit_with_refs.refs.retain(|ri| {
                    ri.ref_name.category().is_none_or(|c| {
                        if matches!(c, Category::RemoteBranch) {
                            // Always remove the ref, but keep info to create a split if possible.
                            if is_splittable {
                                let info = (node.id, cidx, ri.ref_name.clone());
                                // This means we are more interested in the split than in representing every reference for now.
                                if split_info.iter().any(|(a_sidx, a_cidx, _)| *a_sidx == node.id && *a_cidx == cidx) {
                                    tracing::debug!(?node.id, ?commit_with_refs.id, ?ri, "Ignoring remote reference which *should* have no effect");
                                } else {
                                    split_info.push(info);
                                }
                            }
                            false
                        } else {
                            true
                        }
                    })
                })
            }
        }

        for (sidx, new_segment_start_idx, segment_name) in split_info.into_iter().rev() {
            self.split_segment(
                sidx,
                new_segment_start_idx,
                Some(segment_name),
                None,
                meta,
                worktree_by_branch,
            )?;
        }
        Ok(())
    }

    /// Assure that workspace segments with managed commits only have that commit, and move all others
    /// into a new segment.
    fn fixup_workspace_segments<T: RefMetadata>(
        &mut self,
        repo: &OverlayRepo<'_>,
        refs_by_id: &RefsById,
        meta: &OverlayMetadata<'_, T>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        let workspace_segments_with_multiple_commits: Vec<_> = self
            .inner
            .node_indices()
            .filter(|sidx| {
                let s = &self[*sidx];
                s.workspace_metadata().is_some() && s.commits.len() > 1
            })
            .collect();

        for ws_sidx in workspace_segments_with_multiple_commits {
            let s = &mut self[ws_sidx];
            let first_commit = &mut s.commits[0];
            if !crate::projection::commit::is_managed_workspace_by_message(
                repo.find_commit(first_commit.id)?.message_raw()?,
            ) {
                continue;
            }

            self.split_segment(ws_sidx, 1, None, Some(refs_by_id), meta, worktree_by_branch)?;
        }
        Ok(())
    }

    fn split_segment<T: RefMetadata>(
        &mut self,
        sidx: NodeIndex,
        cidx_for_new_segment: CommitIndex,
        segment_name: Option<gix::refs::FullName>,
        refs_by_id: Option<&RefsById>,
        meta: &OverlayMetadata<'_, T>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<SegmentIndex> {
        let s = &mut self[sidx];
        let tip_of_new_segment = s
            .commits
            .get(cidx_for_new_segment)
            .with_context(|| {
                format!(
                    "Segment {sidx:?} \
        has only {} commit(s), cannot split at {cidx_for_new_segment}",
                    s.commits.len()
                )
            })?
            .id;
        let new_segment_commits = s.commits.drain(cidx_for_new_segment..).collect();
        let last_cidx_in_top_segment = s.last_commit_index();
        let edges_to_reconnect: Vec<_> = self
            .edges_directed_in_order_of_creation(sidx, Direction::Outgoing)
            .into_iter()
            .map(EdgeOwned::from)
            .collect();
        let mut new_segment = branch_segment_from_name_and_meta(
            segment_name.map(|sn| (sn, None)),
            meta,
            refs_by_id.map(|lut| (lut, tip_of_new_segment)),
            worktree_by_branch,
        )?;
        new_segment.commits = new_segment_commits;
        let new_segment_sidx = self.connect_new_segment(
            sidx,
            last_cidx_in_top_segment,
            new_segment,
            0,
            tip_of_new_segment,
        );

        let (src, src_id) = {
            let s = &self[new_segment_sidx];
            let last = s.commits.len() - 1;
            (Some(last), Some(s.commits[last].id))
        };
        for edge in edges_to_reconnect {
            self.inner.add_edge(
                new_segment_sidx,
                edge.target,
                Edge {
                    src,
                    src_id,
                    dst: edge.weight.dst,
                    dst_id: edge.weight.dst_id,
                },
            );
            self.inner.remove_edge(edge.id);
        }

        if cidx_for_new_segment == 0 {
            // The top-segment is still connected with edges that think they link to a commit, so adjust them.
            let edges_to_adjust: Vec<_> = self
                .inner
                .edges_directed(sidx, Direction::Incoming)
                .map(|e| e.id())
                .collect();
            for edge_id in edges_to_adjust {
                let edge = self
                    .inner
                    .edge_weight_mut(edge_id)
                    .expect("still present as we just saw it");
                edge.dst = None;
                edge.dst_id = None;
            }
        }
        Ok(new_segment_sidx)
    }

    /// To keep it simple, the iteration will not always create perfect segment names right away so we
    /// fix it in post.
    ///
    /// * segments are anonymous even though there is an unambiguous name for its first parent.
    ///   These segments sometimes are inserted to assure workspace segments don't own non-workspace commits.
    /// * segments have a name, but the same name is still visible in the refs of the first commit.
    ///
    /// Only perform disambiguation on proxy segments (i.e. those inserted segments to prevent commit-ownership).
    fn fixup_segment_names<T: RefMetadata>(
        &mut self,
        meta: &OverlayMetadata<'_, T>,
        inserted_proxy_segments: &[SegmentIndex],
        worktree_by_branch: &WorktreeByBranch,
    ) {
        let segments_with_refs_on_first_commit: Vec<_> = self
            .inner
            .node_indices()
            .filter(|sidx| {
                self[*sidx]
                    .commits
                    .first()
                    .is_some_and(|c| !c.refs.is_empty())
            })
            .collect();
        for sidx in segments_with_refs_on_first_commit {
            let s = &mut self.inner[sidx];
            let first_commit = &mut s.commits[0];
            if let Some(srn) = &s.ref_info {
                if let Some(pos) = first_commit.refs.iter().position(|rn| rn == srn) {
                    first_commit.refs.remove(pos);
                }
            } else {
                match first_commit.refs.len() {
                    0 => unreachable!("prefiltered"),
                    1 => {
                        if first_commit
                            .refs
                            .first()
                            .is_some_and(|rn| rn.ref_name.category() == Some(Category::LocalBranch))
                        {
                            s.ref_info = first_commit.refs.pop();
                            s.metadata = meta
                                .branch_opt(s.ref_name().expect("just set"))
                                .ok()
                                .flatten()
                                .map(|md| SegmentMetadata::Branch(md.clone()));
                        }
                    }
                    _ => {
                        if !inserted_proxy_segments.contains(&sidx) {
                            continue;
                        }
                        let Some((rn, metadata)) = disambiguate_refs_by_branch_metadata(
                            first_commit.refs.iter().map(|ri| &ri.ref_name),
                            meta,
                        ) else {
                            continue;
                        };

                        s.metadata = metadata;
                        first_commit.refs.retain(|cri| cri.ref_name != rn);
                        s.ref_info = Some(crate::RefInfo::from_ref(rn, worktree_by_branch));
                    }
                }
            }
        }
    }

    /// Find all *unique* commits (along with their owning segments) where we can look for references that are to be
    /// spread out as independent branches.
    /// This means these commits must be in the workspace!
    fn candidates_for_independent_branches_in_workspace(
        &self,
        ws_sidx: SegmentIndex,
        target_ref: Option<SegmentIndex>,
        target_commit_sidx: Option<SegmentIndex>,
        ws_low_bound: Option<SegmentIndex>,
        ws_stacks: &[crate::projection::Stack],
        repo: &OverlayRepo<'_>,
    ) -> anyhow::Result<Vec<SegmentIndex>> {
        let mut out: Vec<_> = ws_stacks
            .iter()
            .filter_map(|s| {
                s.base_segment_id().and_then(|sidx| {
                    let base_segment = &self[sidx];
                    // These are naturally in the workspace.
                    base_segment
                        .commit_index_of(s.base().expect("must be set if sidx is set"))
                        .map(|cidx| (sidx, &base_segment.commits[cidx]))
                })
            })
            .chain(
                self.inner
                    .neighbors_directed(ws_sidx, Direction::Outgoing)
                    .filter_map(|s| {
                        // This rule means that if there is no target, we'd want to put new independent stacks
                        // onto segments which then are ambiguous so they get pulled out.
                        if target_ref.is_some() {
                            return None;
                        }
                        // It's a very specialised filter… will that lead to strange behaviour later?
                        let segment = &self[s];
                        if segment.ref_info.is_some() {
                            return None;
                        }
                        segment
                            .commits
                            .first()
                            .filter(|c| !c.refs.is_empty())
                            .map(|c| (s.id(), c))
                    }),
            )
            .collect();
        out.sort_by_key(|t| t.1.id);
        out.dedup_by_key(|t| t.1.id);

        let mut out: Vec<_> = out.into_iter().map(|t| t.0).collect();
        for extra_sidx in self
            .extra_target
            .into_iter()
            .chain(target_commit_sidx)
            .chain(ws_low_bound)
            .dedup()
        {
            out.extend(
                self.first_commit_or_find_along_first_parent(extra_sidx)
                    .and_then(|(c, sidx)| {
                        (!out.contains(&sidx) && c.flags.contains(CommitFlags::InWorkspace))
                            .then_some(sidx)
                    }),
            );
        }

        match target_ref {
            None => {
                let Some(commit_with_refs) =
                    self[ws_sidx].commits.first().filter(|c| !c.refs.is_empty())
                else {
                    return Ok(out);
                };

                // Never create anything on top of managed commits.
                if crate::projection::commit::is_managed_workspace_by_message(
                    commit_with_refs
                        .id
                        .attach(repo.for_attach_only())
                        .object()?
                        .try_into_commit()?
                        .message_raw()?,
                ) {
                    return Ok(out);
                }

                // This means we are managed, but have lost our workspace commit, instead we
                // own a commit. This really shouldn't happen.
                tracing::warn!(
                    "Workspace segment {ws_sidx:?} is owning a non-workspace commit\
                     - this shouldn't be possible"
                )
            }
            Some(target_sidx) => {
                let target_rtb = &self[target_sidx];
                if self.is_connected_from_above(target_sidx, ws_sidx) {
                    out.extend(
                        self.first_commit_or_find_along_first_parent(target_rtb.id)
                            .and_then(|(c, sidx)| {
                                c.flags.contains(CommitFlags::InWorkspace).then_some(sidx)
                            }),
                    );
                    out.sort();
                    out.dedup();
                }
            }
        }
        Ok(out)
    }

    /// Perform operations on the current workspace, or do nothing if there is `None`.
    ///
    /// * workspace segments are either empty, or have just one managed commit.
    /// * insert empty segments as defined by the workspace that affects its downstream.
    /// * put workspace connection into the order defined in the workspace metadata.
    /// * set sibling segment IDs for unnamed segments that are descendents of an out-of-workspace but known segment.
    fn workspace_upgrades<T: RefMetadata>(
        &mut self,
        meta: &OverlayMetadata<'_, T>,
        repo: &OverlayRepo<'_>,
        refs_by_id: &RefsById,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        let Some((
            ws_sidx,
            ws_stacks,
            ws_data,
            ws_target_ref,
            ws_target_commit,
            ws_low_bound_in_ws_sidx,
            ws_low_bound,
        )) = self
            .to_workspace_state(workspace::Downgrade::Disallow)
            .ok()
            .and_then(|mut ws| {
                let md = ws.metadata.take();
                md.map(|md| {
                    let lower_bound_if_in_workspace = ws.lower_bound_segment_id.filter(|lb_sidx| {
                        ws.stacks
                            .iter()
                            .flat_map(|s| s.segments.iter().map(|s| s.id))
                            .any(|sid| sid == *lb_sidx)
                    });
                    (
                        ws.id,
                        ws.stacks,
                        md,
                        ws.target_ref,
                        ws.target_commit,
                        lower_bound_if_in_workspace,
                        ws.lower_bound_segment_id,
                    )
                })
            })
        else {
            return Ok(());
        };

        // Setup independent stacks, first by looking at potential bases.
        let candidates = self.candidates_for_independent_branches_in_workspace(
            ws_sidx,
            ws_target_ref.as_ref().map(|t| t.segment_index),
            ws_target_commit.as_ref().map(|t| t.segment_index),
            ws_low_bound,
            &ws_stacks,
            repo,
        )?;
        for base_sidx in candidates.iter().cloned() {
            let mut seen = BTreeSet::new();
            let base_segment = &self[base_sidx];
            // Also use the segment name as part of available refs to latch on to, and make
            // the segment anonymous if it actually gets used.
            let base_segment_name = base_segment.ref_info.clone();
            let matching_refs_per_stack: Vec<_> = find_all_desired_stack_refs_in_commit(
                &ws_data,
                base_segment_name
                    .as_ref()
                    .into_iter()
                    .map(|ri| &ri.ref_name)
                    .chain(
                        base_segment
                            .commits
                            .first()
                            .map(|c| c.refs.iter())
                            .into_iter()
                            .flatten()
                            .map(|ri| &ri.ref_name),
                    ),
                (&self.inner, ws_sidx, &ws_stacks, &candidates),
            )
            .collect();
            for refs_for_independent_branches in matching_refs_per_stack {
                // Matching refs can be repeated, even with unsound workspace metadata that mentions them multiple times.
                // Instead of catching this earlier, we handle deduplication right here where it matters.
                let unique_refs_for_independent_branches: Vec<_> = refs_for_independent_branches
                    .into_iter()
                    .filter_map(|rn| seen.insert(rn.clone()).then_some(rn))
                    .collect();
                if unique_refs_for_independent_branches.is_empty() {
                    continue;
                }
                let edges_connecting_base_with_ws_tip: Vec<EdgeOwned> = self
                    .inner
                    .edges_connecting(ws_sidx, base_sidx)
                    .map(Into::into)
                    .collect();
                create_independent_segments(
                    self,
                    ws_sidx,
                    base_sidx,
                    unique_refs_for_independent_branches,
                    meta,
                    worktree_by_branch,
                )?;
                for edge in edges_connecting_base_with_ws_tip {
                    self.inner.remove_edge(edge.id);
                }
            }
        }

        // Setup dependent stacks based on searching refs on existing workspace commits.
        // Note that we can still source names from previously used stacks just to be able to capture more
        // of the original intent, despite the graph having changed. This works because in the end, we are consuming
        // refs on commits that can't be reused once they have been moved into their own segment.
        let mut segments_to_possibly_delete = Vec::new();
        for stack in &ws_stacks {
            let mut last_created_segment = None;
            for ws_segment_sidx in stack
                .segments
                .iter()
                .flat_map(|segment| segment.commits_by_segment.iter().map(|t| t.0))
            {
                // Find all commit-refs which are mentioned in ws_data.stacks, for simplicity in any stack that matches (for now).
                // Stacks shouldn't be so different that they don't match reality anymore and each mutation has to re-set them to
                // match reality.
                let mut current_above = ws_segment_sidx;
                let mut truncate_commits_from = None;
                for commit_idx in 0..self[ws_segment_sidx].commits.len() {
                    let commit = &self[ws_segment_sidx].commits[commit_idx];
                    let has_inserted_segment_above = current_above != ws_segment_sidx;
                    let Some(refs_for_dependent_branches) =
                        find_all_desired_stack_refs_in_commit_for_dependent_branches(
                            &ws_data,
                            commit.ref_iter(),
                            None,
                        )
                        .next()
                    else {
                        // Now we have to assign this uninteresting commit to the last created segment, if there was one.
                        if has_inserted_segment_above {
                            self.push_commit_and_reconnect_outgoing(
                                commit.clone(),
                                current_above,
                                (ws_segment_sidx, commit_idx),
                            );
                        }
                        continue;
                    };

                    // In ws-stack segment order, map all the indices from top to bottom
                    let new_above = maybe_create_multiple_segments(
                        self,
                        current_above,
                        ws_segment_sidx,
                        Some(commit_idx),
                        refs_for_dependent_branches,
                        meta,
                        worktree_by_branch,
                    )?;
                    current_above = new_above;
                    truncate_commits_from.get_or_insert(commit_idx);
                }
                if let Some(truncate_from) = truncate_commits_from {
                    let segment = &mut self[ws_segment_sidx];
                    // Keep only the commits that weren't reassigned to other segments.
                    segment.commits.truncate(truncate_from);
                    segments_to_possibly_delete.push(ws_segment_sidx);
                }
                last_created_segment = Some(current_above);
            }

            if let Some((last_segment_id, base_sidx, commit)) =
                stack.segments.last().and_then(|s| {
                    let base_sidx = s.base_segment_id?;
                    let c = self.node_weight(base_sidx)?.commits.first()?;
                    Some((last_created_segment.unwrap_or(s.id), base_sidx, c))
                })
            {
                let Some(refs_for_dependent_branches) =
                    find_all_desired_stack_refs_in_commit_for_dependent_branches(
                        &ws_data,
                        self[base_sidx]
                            .ref_info
                            .as_ref()
                            .map(|ri| &ri.ref_name)
                            .filter(|_| !commit.refs.is_empty())
                            .into_iter()
                            .chain(commit.ref_iter()),
                        Some(stack),
                    )
                    .next()
                else {
                    continue;
                };

                let edges_from_segment_above = collect_edges_at_commit_reverse_order(
                    self,
                    (
                        last_segment_id,
                        self[last_segment_id].commits.len().checked_sub(1),
                    ),
                    Direction::Outgoing,
                );
                let new_sidx_above_base_sidx = maybe_create_multiple_segments(
                    self,
                    last_segment_id,
                    base_sidx,
                    None,
                    refs_for_dependent_branches.clone(),
                    meta,
                    worktree_by_branch,
                )?;

                // As we didn't allow the previous function to deal with the commit, we do it.
                let s = &mut self[base_sidx];
                s.commits
                    .first_mut()
                    .expect("we know there is one already")
                    .refs
                    .retain(|ri| !refs_for_dependent_branches.contains(&ri.ref_name));
                s.ref_info
                    .take_if(|ri| refs_for_dependent_branches.contains(&ri.ref_name));
                if s.ref_info.is_none() {
                    s.metadata = None;
                    if let Some(sibling) = s.sibling_segment_id.take() {
                        self[sibling].sibling_segment_id = None;
                    }
                }

                let s = &mut self[base_sidx];
                if let Some(refs) = s
                    .commits
                    .first_mut()
                    .filter(|c| !c.refs.is_empty())
                    .map(|c| &mut c.refs)
                    && let Some((name, md)) = disambiguate_refs_by_branch_metadata(
                        refs.iter().map(|ri| &ri.ref_name),
                        meta,
                    )
                {
                    refs.retain(|ri| ri.ref_name != name);
                    s.ref_info = Some(crate::RefInfo::from_ref(name, worktree_by_branch));
                    s.metadata = md;
                }
                reconnect_outgoing_edges(
                    self,
                    edges_from_segment_above,
                    (new_sidx_above_base_sidx, None),
                );
            }
        }

        for sidx in segments_to_possibly_delete {
            delete_anon_if_empty_and_reconnect(self, sidx);
        }

        // Redo workspace outgoing connections according to desired stack order.
        let mut edges_pointing_to_named_segment = self
            .edges_directed_in_order_of_creation(ws_sidx, Direction::Outgoing)
            .into_iter()
            .map(|e| {
                let rn = self[e.target()].ref_info.clone();
                (e.id(), e.target(), rn)
            })
            .collect::<Vec<_>>();

        let edges_original_order: Vec<_> = edges_pointing_to_named_segment
            .iter()
            .map(|(_e, sidx, _rn)| *sidx)
            .collect();
        edges_pointing_to_named_segment.sort_by_key(|(_e, sidx, ri)| {
            let res = ws_data.stacks.iter().position(|s| {
                s.is_in_workspace()
                    && s.branches
                        .first()
                        .is_some_and(|b| Some(&b.ref_name) == ri.as_ref().map(|ri| &ri.ref_name))
            });
            // This makes it so that edges that weren't mentioned in workspace metadata
            // retain their relative order, with first-come-first-serve semantics.
            // The expected case is that each segment is defined.
            res.or_else(|| {
                edges_original_order
                    .iter()
                    .position(|sidx_for_order| sidx_for_order == sidx)
            })
        });

        for (eid, target_sidx, _) in edges_pointing_to_named_segment {
            let weight = self
                .inner
                .remove_edge(eid)
                .expect("we found the edge before");
            // Reconnect according to the new order.
            self.inner.add_edge(ws_sidx, target_sidx, weight);
        }

        // Setup sibling IDs for all unnamed segments with a known segment ref in its future.
        for sidx in ws_stacks.iter().flat_map(|s| {
            s.segments
                .iter()
                .flat_map(|s| s.commits_by_segment.iter().map(|(sidx, _)| *sidx))
        }) {
            // The workspace might be stale by now as we delete empty segments.
            // Thus, be careful, and ignore non-existing ones - after all our workspace
            // is temporary, nothing to worry about.
            let Some(s) = self.inner.node_weight(sidx) else {
                continue;
            };
            if s.ref_info.is_some() || s.sibling_segment_id.is_some() {
                continue;
            }

            let num_outgoing = self
                .inner
                .neighbors_directed(sidx, Direction::Incoming)
                .count();
            if num_outgoing < 2 {
                continue;
            }

            let mut named_segment_id = None;
            self.visit_all_segments_excluding_start_until(sidx, Direction::Incoming, |s| {
                let prune = true;
                if named_segment_id.is_some()
                    || s.commits
                        .first()
                        .is_some_and(|c| c.flags.contains(CommitFlags::InWorkspace))
                {
                    return prune;
                }

                s.ref_info.as_ref().is_some_and(|ri| {
                    let is_known_to_workspace = ws_data
                        .stacks(AppliedAndUnapplied)
                        .any(|s| s.branches.iter().any(|b| b.ref_name == ri.ref_name));
                    if is_known_to_workspace {
                        named_segment_id = Some(s.id);
                    }
                    is_known_to_workspace
                })
            });
            self[sidx].sibling_segment_id = named_segment_id;
        }

        // The named-segment check is needed as we don't want to double-split unnamed segments.
        // What this really does is to pass ownership of the base commit from a named segment to an unnamed one,
        // as all algorithms kind of rely on it.
        // So if this ever becomes a problem, we can also try to adjust said algorithms downstream.
        if let Some(low_bound_segment_id) = ws_low_bound_in_ws_sidx
            && self[low_bound_segment_id].ref_info.is_some()
        {
            self.split_segment(
                low_bound_segment_id,
                0,
                None,
                Some(refs_by_id),
                meta,
                worktree_by_branch,
            )?;
        }
        Ok(())
    }

    /// Name ambiguous segments if they are reachable by remote tracking branch and
    /// if the first commit has (unambiguously) the matching local tracking branch.
    /// Also, link up all remote segments with their local ones, and vice versa.
    ///
    /// Additionally, restore possibly broken linkage to their siblings
    /// (from remote to local tracking branch), as this connection might have been destroyed by
    /// the insertion of empty segments. Again, instead of making that smarter, we fix it up here
    /// because it's simpler.
    fn improve_remote_segments(
        &mut self,
        repo: &OverlayRepo<'_>,
        symbolic_remote_names: &[String],
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        // Map (segment-to-be-named, [candidate-remote]), so we don't set a name if there is more
        // than one remote.
        let mut remotes_by_segment_map = BTreeMap::<
            SegmentIndex,
            Vec<(gix::refs::FullName, gix::refs::FullName, SegmentIndex)>,
        >::new();

        let mut remote_sidx_by_ref_name = BTreeMap::new();
        for (remote_sidx, remote_ref_name) in self.inner.node_indices().filter_map(|sidx| {
            self[sidx]
                .ref_info
                .as_ref()
                .map(|ri| &ri.ref_name)
                .filter(|rn| rn.category() == Some(Category::RemoteBranch))
                .map(|rn| (sidx, rn))
        }) {
            remote_sidx_by_ref_name.insert(remote_ref_name.clone(), remote_sidx);
            let start_idx = self[remote_sidx].commits.first().map(|_| 0);
            let mut walk = TopoWalk::start_from(remote_sidx, start_idx, Direction::Outgoing)
                .skip_tip_segment();

            while let Some((sidx, commit_range)) = walk.next(&self.inner) {
                let segment = &self[sidx];
                if segment.ref_info.is_some() {
                    // Assume simple linear histories - otherwise this could abort too early, and
                    // we'd need a complex traversal - not now.
                    break;
                }

                if segment.commits.is_empty() {
                    // skip over empty anonymous buckets, even though these shouldn't exist, ever.
                    tracing::warn!(
                        "Skipped segment {sidx} which was anonymous and empty",
                        sidx = sidx.index()
                    );
                    continue;
                } else if segment.commits[commit_range]
                    .iter()
                    .all(|c| c.flags.contains(CommitFlags::NotInRemote))
                {
                    // a candidate for naming, and we'd either expect all or none of the commits
                    // to be in or outside a remote.
                    let first_commit = segment.commits.first().expect("we know there is commits");
                    if let Some(local_tracking_branch) = first_commit.refs.iter().find_map(|ri| {
                        remotes::lookup_remote_tracking_branch_or_deduce_it(
                            repo,
                            ri.ref_name.as_ref(),
                            symbolic_remote_names,
                            configured_remote_tracking_branches,
                        )
                        .ok()
                        .flatten()
                        .and_then(|rrn| (&rrn == remote_ref_name).then_some(ri.ref_name.clone()))
                    }) {
                        remotes_by_segment_map.entry(sidx).or_default().push((
                            local_tracking_branch,
                            remote_ref_name.to_owned(),
                            remote_sidx,
                        ));
                    }
                    break;
                }
                // Assume that the segment is fully remote.
                continue;
            }
        }

        for (anon_sidx, mut disambiguated_name) in remotes_by_segment_map
            .into_iter()
            .filter(|(_, candidates)| candidates.len() == 1)
        {
            let s = &mut self[anon_sidx];
            let (local, remote, remote_sidx) =
                disambiguated_name.pop().expect("one item as checked above");
            s.ref_info = Some(crate::RefInfo::from_ref(local, worktree_by_branch));
            s.remote_tracking_ref_name = Some(remote);
            s.sibling_segment_id = Some(remote_sidx);
            let rn = s.ref_info.as_ref().expect("just set it");
            s.commits.first_mut().unwrap().refs.retain(|crn| crn != rn);
            // Assure the remote is also paired up!
            self[remote_sidx].sibling_segment_id = Some(s.id);
        }

        // NOTE: setting this directly at iteration time isn't great as the post-processing then
        //       also has to deal with these implicit connections. So it's best to redo them in the end.
        let mut links_from_remote_to_local = Vec::new();
        for segment in self.inner.node_weights_mut() {
            if segment.remote_tracking_ref_name.is_some() {
                continue;
            };
            let Some(ref_name) = segment.ref_info.as_ref().map(|ri| &ri.ref_name) else {
                continue;
            };
            segment.remote_tracking_ref_name = remotes::lookup_remote_tracking_branch_or_deduce_it(
                repo,
                ref_name.as_ref(),
                symbolic_remote_names,
                configured_remote_tracking_branches,
            )?;

            if let Some(remote_sidx) = segment
                .remote_tracking_ref_name
                .as_ref()
                .and_then(|rn| remote_sidx_by_ref_name.remove(rn.as_ref()))
            {
                segment.sibling_segment_id = Some(remote_sidx);
                links_from_remote_to_local.push((remote_sidx, segment.id));
            }
        }
        for (remote_sidx, local_sidx) in links_from_remote_to_local {
            self[remote_sidx].sibling_segment_id = Some(local_sidx);

            // If both remote and local point to the same commit, make sure that the remote points to the local segment.
            if let Some((
                (_remote_commit, _owner_of_commit_same_as_local),
                (_local_commmit, owner_of_commit_same_as_remote),
            )) = self
                .first_commit_or_find_along_first_parent(remote_sidx)
                .zip(self.first_commit_or_find_along_first_parent(local_sidx))
                .filter(|((a, a_sidx), (b, b_sidx))| a.id == b.id && a_sidx == b_sidx)
            {
                let outgoing: Vec<_> = self
                    .edges_directed_in_order_of_creation(remote_sidx, Direction::Outgoing)
                    .into_iter()
                    .map(EdgeOwned::from)
                    .collect();
                let remote_is_connected_to_local =
                    outgoing.iter().any(|e| e.target.id() == local_sidx);
                if !remote_is_connected_to_local
                    && let Some(edge) = outgoing.iter().find(|e| {
                        outgoing.len() == 1 || e.target.id() == owner_of_commit_same_as_remote
                    })
                {
                    self.inner.remove_edge(edge.id);
                    self.inner.add_edge(
                        remote_sidx,
                        local_sidx,
                        edge.weight
                            .adjusted_for(remote_sidx, local_sidx, &self.inner),
                    );
                }
            }
        }
        Ok(())
    }

    fn push_commit_and_reconnect_outgoing(
        &mut self,
        commit: Commit,
        current_above: SegmentIndex,
        (ws_segment_sidx, commit_idx): (SegmentIndex, CommitIndex),
    ) {
        let commit_id = commit.id;
        self[current_above].commits.push(commit);
        reconnect_outgoing(
            &mut self.inner,
            (ws_segment_sidx, commit_idx),
            (current_above, commit_id),
        );
    }

    // Fill in generation numbers by walking down the graph topologically.
    // This is called at the end of post-processing to ensure generations are correct
    // after all segment insertions and edge rewiring.
    fn compute_generation_numbers(&mut self) {
        let mut topo = petgraph::visit::Topo::new(&self.inner);
        while let Some(sidx) = topo.next(&self.inner) {
            let max_gen_of_incoming = self
                .inner
                .neighbors_directed(sidx, petgraph::Direction::Incoming)
                .map(|sidx| self[sidx].generation + 1)
                .max()
                .unwrap_or(0);
            self[sidx].generation = max_gen_of_incoming;
        }
    }

    /// Returns `true` if `below_sidx` is connected to `above_sidx` from above, so `above_sidx` has an
    /// outgoing connection to `below_sidx`.
    fn is_connected_from_above(&self, below_sidx: SegmentIndex, above_sidx: SegmentIndex) -> bool {
        self.edges_directed(below_sidx, Direction::Incoming)
            .any(|e| e.source() == above_sidx)
    }
}

/// Search `ws_data` for all matching names in `commit_refs` and return them, once per stack.
/// If `only_in_stack` is set, it will only search the stack in `ws_data` that either matches its stack-id or
/// its tip name, to avoid matching names that are not meant to be used in `only_in_stack`.
fn find_all_desired_stack_refs_in_commit_for_dependent_branches<'a>(
    ws_data: &'a ref_metadata::Workspace,
    commit_refs: impl Iterator<Item = &'a gix::refs::FullName> + Clone + 'a,
    only_in_stack: Option<&'a crate::projection::Stack>,
) -> impl Iterator<Item = Vec<gix::refs::FullName>> + 'a {
    ws_data.stacks(Applied).filter_map(move |stack| {
        if only_in_stack.is_some_and(|limit_to| {
            limit_to.id != Some(stack.id) && limit_to.ref_name() != stack.name()
        }) {
            return None;
        }
        let matching_refs: Vec<_> = stack
            .branches
            .iter()
            .filter_map(|s| commit_refs.clone().find(|rn| *rn == &s.ref_name).cloned())
            .collect();
        if matching_refs.is_empty() {
            return None;
        }
        Some(matching_refs)
    })
}

fn find_all_desired_stack_refs_in_commit<'a>(
    ws_data: &'a ref_metadata::Workspace,
    commit_refs: impl Iterator<Item = &'a gix::refs::FullName> + Clone + 'a,
    graph_and_ws_idx_and_candidates: (
        &'a PetGraph,
        SegmentIndex,
        &'a [crate::projection::Stack],
        &'a [SegmentIndex],
    ),
) -> impl Iterator<Item = Vec<gix::refs::FullName>> + 'a {
    let (graph, ws_idx, ws_stacks, candidates) = graph_and_ws_idx_and_candidates;
    ws_data.stacks(Applied).filter_map(move |stack| {
        if ws_stacks
            .iter()
            .filter(|s| !s.segments.iter().any(|s| candidates.contains(&s.id)))
            .any(|existing_stack| existing_stack.id == Some(stack.id))
        {
            return None;
        }
        let matching_refs: Vec<_> = stack
            .branches
            .iter()
            .filter_map(|s| commit_refs.clone().find(|rn| *rn == &s.ref_name).cloned())
            .collect();
        if matching_refs.is_empty() {
            return None;
        }

        // We match any part of a stack above, so have to assure we don't recreate
        // part of an existing stack as new stack.
        let is_used_in_existing_stack = stack.branches.first().is_some_and(|top_segment_name| {
            graph
                .neighbors_directed(ws_idx, Direction::Outgoing)
                .filter(|sidx| !candidates.contains(sidx))
                .any(|stack_sidx| {
                    graph[stack_sidx].ref_name() == Some(top_segment_name.ref_name.as_ref())
                })
        });
        if is_used_in_existing_stack {
            return None;
        }
        Some(matching_refs)
    })
}

/// **Warning**: this can make workspace stacks stale, i.e. let them refer to non-existing segments.
///              all accesses from hereon must be done with care. On the other hand, we can ignore
///              that as our workspace is just temporary.
fn delete_anon_if_empty_and_reconnect(graph: &mut Graph, sidx: SegmentIndex) {
    let segment = &graph[sidx];
    let may_delete = segment.commits.is_empty() && segment.ref_info.is_none();
    if !may_delete {
        return;
    }

    let mut outgoing = graph.inner.edges_directed(sidx, Direction::Outgoing);
    let Some(first_outgoing) = outgoing.next() else {
        return;
    };

    if outgoing.next().is_some() {
        return;
    }

    tracing::debug!(
        ?sidx,
        "Deleting seemingly isolated and now completely unused segment"
    );
    // Reconnect
    let new_target = first_outgoing.target();
    let incoming: Vec<_> = graph
        .inner
        .edges_directed(sidx, Direction::Incoming)
        .map(EdgeOwned::from)
        .collect();
    let (target_commit_id, target_commit_idx) = graph[new_target]
        .commits
        .first()
        .map(|c| (Some(c.id), Some(0)))
        .unwrap_or_default();
    for edge in incoming.iter().rev() {
        graph.inner.add_edge(
            edge.source,
            new_target,
            Edge {
                src: edge.weight.src,
                src_id: edge.weight.src_id,
                dst: target_commit_idx,
                dst_id: target_commit_id,
            },
        );
    }
    graph.inner.remove_node(sidx);

    if let Some(ep_sidx) = graph
        .entrypoint
        .as_mut()
        .map(|t| &mut t.0)
        .filter(|ep_sidx| **ep_sidx == sidx)
    {
        *ep_sidx = new_target;
    }
    if let Some(extra_target) = graph.extra_target.as_mut() {
        *extra_target = new_target;
    }
}

/// Create as many new segments as refs in `matching_refs`, connect them to each other in order, and finally connect them
/// with `above_idx` and `below_idx` to integrate them into the workspace that is bounded by these segments.
fn create_independent_segments<T: RefMetadata>(
    graph: &mut Graph,
    above_idx: SegmentIndex,
    below_idx: SegmentIndex,
    matching_refs: Vec<gix::refs::FullName>,
    meta: &OverlayMetadata<'_, T>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<()> {
    assert!(!matching_refs.is_empty());

    let mut above = above_idx;
    let mut new_refs = graph[below_idx].commits[0].refs.clone();
    for ref_name in matching_refs {
        let new_segment = branch_segment_from_name_and_meta(
            Some((ref_name.clone(), None)),
            meta,
            None,
            worktree_by_branch,
        )?;
        let new_segment_sidx = graph.connect_new_segment(
            above,
            graph[above].last_commit_index(),
            new_segment,
            None,
            None,
        );
        above = new_segment_sidx;

        match new_refs.iter().position(|ri| ri.ref_name == ref_name) {
            None => {
                let s = &mut graph[below_idx];
                if s.ref_name() != Some(ref_name.as_ref()) {
                    bail!(
                        "BUG: ref-names must either be present in the first commit, or be the segment name: below_idx = {below_idx:?}, below.ref_name = {below_name:?}, above.ref_name = {above_name}",
                        below_name = s.ref_name().map(|rn| rn.as_bstr()),
                        above_name = ref_name.as_bstr()
                    )
                }
                s.ref_info = None;
                s.metadata = None;
                let sibling = s.sibling_segment_id.take();
                graph[new_segment_sidx].sibling_segment_id = sibling;

                if let Some((ep_sidx, ep_commit_idx)) = graph.entrypoint.as_mut()
                    && *ep_sidx == below_idx
                {
                    *ep_sidx = new_segment_sidx;
                    *ep_commit_idx = None;
                }
            }
            Some(pos) => {
                new_refs.remove(pos);
            }
        }
    }
    graph.connect_segments(above, None, below_idx, Some(0));
    if let Some(first_comit) = graph[below_idx].commits.first_mut() {
        first_comit.refs = new_refs;
    }
    Ok(())
}

/// Maybe create a new stack from `N` (where `N` > 1) refs that match a ref in `ws_stack` (in the order given there), with `N-1` segments being empty on top
/// of the last one `N`.
/// `commit_parent_below` is the segment to use `commit_idx` on to get its data, and assumed below `above_idx`.
/// We also use this information to re-link segments (vaguely).
/// If `commit_idx` is `None` (hack), don't add a commit at all, leave the segments empty.
/// Return the index of the bottom-most created segment.
/// There may be any amount of new segments above the `bottom_segment_index`.
/// Note that the Segment at `bottom_segment_index` will own `commit`.
/// Also note that we reconnect commit-by-commit, so the outer processing has to do that.
/// Note that it may avoid creating a new segment.
fn maybe_create_multiple_segments<T: RefMetadata>(
    graph: &mut Graph,
    mut above_idx: SegmentIndex,
    commit_parent_below: SegmentIndex,
    commit_idx: Option<CommitIndex>,
    matching_refs: Vec<gix::refs::FullName>,
    meta: &OverlayMetadata<'_, T>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<SegmentIndex> {
    assert!(
        !matching_refs.is_empty(),
        "BUG: We really expect to create new segments here"
    );
    let commit = commit_idx.map(|cidx| &graph[commit_parent_below].commits[cidx]);

    let iter_len = matching_refs.len();

    let commit = commit.map(|commit| {
        let mut c = commit.clone();
        c.refs.retain(|ri| !matching_refs.contains(&ri.ref_name));
        c
    });
    let matching_refs = matching_refs
        .into_iter()
        .enumerate()
        .map(|(idx, ref_name)| {
            let (mut first, mut last) = (false, false);
            if idx == 0 {
                first = true;
            }
            if idx + 1 == iter_len {
                last = true;
            }
            (first, last, ref_name)
        });
    for (is_first, is_last, ref_name) in matching_refs {
        let new_segment = branch_segment_from_name_and_meta(
            Some((ref_name, None)),
            meta,
            None,
            worktree_by_branch,
        )?;
        let above_commit_idx = {
            let s = &graph[above_idx];
            let cidx = commit.as_ref().and_then(|c| s.commit_index_of(c.id));
            if cidx.is_some() {
                // We will take the current commit, so must commit to the one above.
                // This works just once, for the actually passed parent commit.
                cidx.and_then(|cidx| cidx.checked_sub(1))
            } else {
                // Otherwise, assure the connection is valid by using the last commit.
                s.last_commit_index()
            }
        };
        let new_segment = graph.connect_new_segment(
            above_idx,
            above_commit_idx,
            new_segment,
            (is_last && commit.is_some()).then_some(0),
            is_last.then_some(commit.as_ref().map(|c| c.id)).flatten(),
        );
        above_idx = new_segment;
        if is_first {
            // connect incoming edges (and disconnect from source)
            // Connect to the commit if we have one.
            let edges = collect_edges_at_commit_reverse_order(
                &graph.inner,
                (commit_parent_below, commit_idx),
                Direction::Incoming,
            );
            for edge in &edges {
                graph.inner.remove_edge(edge.id);
            }
            for edge in edges.into_iter().rev() {
                let (target, target_cidx) = if commit_idx == Some(0) {
                    // the current target of the edge will be empty after we steal its commit.
                    // Thus, we want to keep pointing to it to naturally reach the commit later.
                    (edge.target, None)
                } else {
                    // The new segment is the shortest way to the commit we loose.
                    (new_segment, (is_last && commit.is_some()).then_some(0))
                };
                graph.inner.add_edge(
                    edge.source,
                    target,
                    Edge {
                        src: edge.weight.src,
                        src_id: edge.weight.src_id,
                        dst: target_cidx,
                        dst_id: target_cidx.and_then(|_| commit.as_ref().map(|c| c.id)),
                    },
                );
            }
        }
        if is_last {
            // connect outgoing edges (and disconnect them)
            if let Some((commit, commit_idx)) = commit.zip(commit_idx) {
                let commit_id = commit.id;
                graph[new_segment].commits.push(commit);

                reconnect_outgoing(
                    &mut graph.inner,
                    (commit_parent_below, commit_idx),
                    (new_segment, commit_id),
                );
            }
            break;
        }
    }
    Ok(above_idx)
}

/// This removes outgoing connections from `source_sidx` and places them on the given commit
/// of `target`.
fn reconnect_outgoing(
    graph: &mut PetGraph,
    (source_sidx, source_cidx): (SegmentIndex, CommitIndex),
    (target_sidx, target_cidx): (SegmentIndex, gix::ObjectId),
) {
    let edges = collect_edges_at_commit_reverse_order(
        graph,
        (source_sidx, Some(source_cidx)),
        Direction::Outgoing,
    );
    reconnect_outgoing_edges(graph, edges, (target_sidx, Some(target_cidx)))
}

/// Delete all `edges` and recreate the edges with `target` as new source.
fn reconnect_outgoing_edges(
    graph: &mut PetGraph,
    edges: Vec<EdgeOwned>,
    (target_sidx, target_first_commit_id): (SegmentIndex, Option<gix::ObjectId>),
) {
    for edge in &edges {
        graph.remove_edge(edge.id);
    }
    for edge in edges.into_iter().rev() {
        let src = target_first_commit_id.and_then(|id| graph[target_sidx].commit_index_of(id));
        graph.add_edge(
            target_sidx,
            edge.target,
            Edge {
                src,
                src_id: target_first_commit_id,
                dst: edge.weight.dst,
                dst_id: edge.weight.dst_id,
            },
        );
    }
}

fn collect_edges_at_commit_reverse_order(
    graph: &PetGraph,
    (segment, commit): (SegmentIndex, Option<CommitIndex>),
    direction: Direction,
) -> Vec<EdgeOwned> {
    graph
        .edges_directed(segment, direction)
        .filter(|&e| match direction {
            Direction::Incoming => e.weight().dst == commit,
            Direction::Outgoing => e.weight().src == commit,
        })
        .map(Into::into)
        .collect()
}
