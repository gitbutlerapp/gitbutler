use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use but_core::RefMetadata;
use gix::reference::Category;
use petgraph::{
    Direction,
    graph::NodeIndex,
    prelude::EdgeRef,
    visit::{IntoEdgeReferences, NodeIndexable},
};

use crate::{
    CommitIndex, Edge, Graph, SegmentIndex,
    init::{
        PetGraph,
        overlay::OverlayMetadata,
        types::EdgeOwned,
        walk::{RefsById, WorktreeByBranch, branch_segment_from_name_and_meta},
    },
    utils::SegmentTable,
};

/// A place history is checked out at, owning the segments no earlier checkout reaches.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Checkout {
    Entrypoint,
    Worktree(usize),
}

impl Graph {
    /// Rebuild outgoing edges so petgraph traversal follows the source commit's
    /// `parent_order`. Petgraph traverses edges in reverse creation order, so
    /// edges are re-added from last parent to first parent.
    pub(crate) fn rebuild_edges_in_parent_order(&mut self) {
        for sidx in self.segments().collect::<Vec<_>>() {
            self.rebuild_outgoing_edges_in_parent_order(sidx);
        }
    }

    /// Like [`Self::rebuild_edges_in_parent_order()`], for `sidx` alone.
    fn rebuild_outgoing_edges_in_parent_order(&mut self, sidx: SegmentIndex) {
        let mut outgoing_edges: Vec<EdgeOwned> = self
            .inner
            .edges_directed(sidx, Direction::Outgoing)
            .map(EdgeOwned::from)
            .collect();
        if outgoing_edges.len() < 2 {
            return;
        }
        outgoing_edges.sort_by_key(|e| std::cmp::Reverse(e.weight.parent_order));
        for edge in &outgoing_edges {
            self.inner.remove_edge(edge.id);
        }
        for edge in &outgoing_edges {
            self.inner.add_edge(edge.source, edge.target, edge.weight);
        }
    }

    /// Fill in generation numbers by walking down the graph topologically, so each segment
    /// knows how far down the graph it is.
    pub(crate) fn compute_generation_numbers(&mut self) {
        let mut topo = petgraph::visit::Topo::new(&self.inner);
        while let Some(sidx) = topo.next(&self.inner) {
            let max_gen_of_incoming = self
                .inner
                .neighbors_directed(sidx, Direction::Incoming)
                .map(|sidx| self[sidx].generation + 1)
                .max()
                .unwrap_or(0);
            self[sidx].generation = max_gen_of_incoming;
        }
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
            .inner
            .edges_directed(sidx, Direction::Outgoing)
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
            0,
        );

        // Move all outgoing edges from the old segment to the new segment,
        // re-sourced at its last commit.
        let last_commit_id = self[new_segment_sidx]
            .commits
            .last()
            .map(|commit| commit.id);
        reconnect_outgoing_edges(
            &mut self.inner,
            edges_to_reconnect,
            (new_segment_sidx, last_commit_id),
        );

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

    /// Make every connection that leaves a worktree's lane land on a commit
    /// instead of the local branch naming it.
    ///
    /// A connection into a named segment implicitly passes that segment's
    /// reference, so inserting below the reference would rewrite everything
    /// connected to it. That is right within one checkout's lane, where a
    /// branch rests on the branch below it, and wrong across lanes: a worktree
    /// forks from the commit it was created at, and must not follow the
    /// branch another checkout has there. Where a worktree's lane connects
    /// into another [checkout's](Checkout) history, the commit-owning segment
    /// is thus made anonymous, its name extracted into an empty segment that
    /// keeps its place in its own lane.
    ///
    /// Only rewritable names can couple a worktree to a rewrite, so only local
    /// branches are extracted: routing through a remote reference is harmless
    /// as the editor never rewrites remotes, and workspace segments must
    /// survive as the workspace's own head.
    pub(crate) fn land_worktree_lanes_on_commits<T: RefMetadata>(
        &mut self,
        meta: &OverlayMetadata<'_, T>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        let owners = self.checkout_owners();
        let mut leaving_by_branch = BTreeMap::<SegmentIndex, Vec<EdgeOwned>>::new();
        for edge in self.inner.edge_references() {
            let source_owner = owners.get(edge.source());
            let leaves_worktree_lane = matches!(source_owner, Some(Checkout::Worktree(_)))
                && owners.get(edge.target()) != source_owner;
            let target = &self[edge.target()];
            let lands_on_local_branch = edge.weight().dst == Some(0)
                && target
                    .ref_name()
                    .is_some_and(|rn| rn.category() == Some(Category::LocalBranch))
                && target.workspace_metadata().is_none();
            if leaves_worktree_lane && lands_on_local_branch {
                leaving_by_branch
                    .entry(edge.target())
                    .or_default()
                    .push(edge.into());
            }
        }
        let mut retargeted = BTreeSet::new();
        for (branch_sidx, leaving) in leaving_by_branch {
            let commits_sidx =
                self.split_segment(branch_sidx, 0, None, None, meta, worktree_by_branch)?;
            for edge in leaving {
                self.inner.remove_edge(edge.id);
                self.connect_segments_with_ids(
                    edge.source,
                    edge.weight.src,
                    edge.weight.src_id,
                    commits_sidx,
                    Some(0),
                    edge.weight.dst_id,
                    edge.weight.parent_order,
                );
                retargeted.insert(edge.source);
            }
        }
        for sidx in retargeted {
            self.rebuild_outgoing_edges_in_parent_order(sidx);
        }
        Ok(())
    }

    /// The first [checkout](Checkout) each segment is reachable from, with the
    /// entrypoint ahead of the worktrees in [tip order](Graph::worktree_tips).
    fn checkout_owners(&self) -> SegmentTable<Option<Checkout>> {
        let mut owners = SegmentTable::new(self.inner.node_bound(), None);
        let entrypoint = self
            .entrypoint
            .map(|(sidx, _)| (Checkout::Entrypoint, sidx));
        let worktrees = self
            .worktree_tips
            .iter()
            .enumerate()
            .filter_map(|(idx, tip)| {
                Some((Checkout::Worktree(idx), self.worktree_tip_segment(tip)?))
            });
        for (checkout, tip_sidx) in entrypoint.into_iter().chain(worktrees) {
            self.visit_all_segments_including_start_until(
                tip_sidx,
                Direction::Outgoing,
                |segment| !owners.set_if_empty(segment.id, Some(checkout)),
            );
        }
        owners
    }
}

/// Delete all `edges` and recreate them with `target` as new source.
///
/// `edges` are expected in traversal order. Petgraph traverses edges in reverse insertion order,
/// so they are re-added in reverse to preserve the same traversal order after reconnection.
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
                parent_order: edge.weight.parent_order,
                workspace_parent_order: edge.weight.workspace_parent_order,
            },
        );
    }
}
