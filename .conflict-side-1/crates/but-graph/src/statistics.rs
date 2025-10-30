use petgraph::Direction;

use crate::{
    CommitFlags, CommitIndex, Graph, SegmentIndex, SegmentMetadata, init::types::TopoWalk,
};

impl Graph {
    /// Return the number segments whose commits are all exclusively in a remote.
    pub fn statistics(&self) -> Statistics {
        let mut out = Statistics::default();
        let Statistics {
            segments,
            segments_integrated,
            segments_remote,
            segments_with_remote_tracking_branch,
            segments_empty,
            segments_unnamed,
            segments_in_workspace,
            segments_in_workspace_and_integrated,
            segments_with_workspace_metadata,
            segments_with_branch_metadata,
            entrypoint_in_workspace,
            segments_behind_of_entrypoint,
            segments_ahead_of_entrypoint,
            entrypoint,
            segment_entrypoint_incoming,
            segment_entrypoint_outgoing,
            top_segments,
            segments_at_bottom,
            connections,
            commits,
            commit_references,
            commits_at_cutoff,
        } = &mut out;

        *segments = self.inner.node_count();
        *connections = self.inner.edge_count();
        *top_segments = self
            .tip_segments()
            .map(|s| {
                let s = &self[s];
                (
                    s.ref_name.clone(),
                    s.id,
                    s.non_empty_flags_of_first_commit(),
                )
            })
            .collect();
        *segments_at_bottom = self.base_segments().count();
        *entrypoint = self.entrypoint.unwrap_or_default();

        if let Ok(ep) = self.lookup_entrypoint() {
            *entrypoint_in_workspace = ep
                .segment
                .commits
                .first()
                .map(|c| c.flags.contains(CommitFlags::InWorkspace));
            *segment_entrypoint_incoming = self
                .inner
                .edges_directed(ep.segment_index, Direction::Incoming)
                .count();
            *segment_entrypoint_outgoing = self
                .inner
                .edges_directed(ep.segment_index, Direction::Outgoing)
                .count();
            for (storage, direction, start_cidx) in [
                (
                    segments_behind_of_entrypoint,
                    Direction::Outgoing,
                    ep.segment.commits.first().map(|_| 0),
                ),
                (
                    segments_ahead_of_entrypoint,
                    Direction::Incoming,
                    ep.segment.commits.last().map(|_| ep.segment.commits.len()),
                ),
            ] {
                let mut walk = TopoWalk::start_from(ep.segment_index, start_cidx, direction)
                    .skip_tip_segment();
                while walk.next(&self.inner).is_some() {
                    *storage += 1;
                }
            }
        }

        for n in self.inner.node_indices().map(|n| &self[n]) {
            *commits += n.commits.len();

            if n.ref_name.is_none() {
                *segments_unnamed += 1;
            }
            if n.remote_tracking_ref_name.is_some() {
                *segments_with_remote_tracking_branch += 1;
            }
            match n.metadata {
                None => {}
                Some(SegmentMetadata::Workspace(_)) => {
                    *segments_with_workspace_metadata += 1;
                }
                Some(SegmentMetadata::Branch(_)) => {
                    *segments_with_branch_metadata += 1;
                }
            }
            // We assume proper segmentation, so the first commit is all we need
            match n.commits.first() {
                Some(c) => {
                    if c.flags.contains(CommitFlags::InWorkspace) {
                        *segments_in_workspace += 1
                    }
                    if c.flags.contains(CommitFlags::Integrated) {
                        *segments_integrated += 1
                    }
                    if c.flags
                        .contains(CommitFlags::InWorkspace | CommitFlags::Integrated)
                    {
                        *segments_in_workspace_and_integrated += 1
                    }
                    if c.flags.is_remote() {
                        *segments_remote += 1;
                    }
                }
                None => {
                    *segments_empty += 1;
                }
            }

            *commit_references += n.commits.iter().map(|c| c.refs.len()).sum::<usize>();
        }

        for sidx in self.inner.node_indices() {
            *commits_at_cutoff += usize::from(self[sidx].commits.last().is_some_and(|c| {
                !c.parent_ids.is_empty()
                    && self
                        .inner
                        .edges_directed(sidx, Direction::Outgoing)
                        .next()
                        .is_none()
            }));
        }
        out
    }
}

/// All kinds of numbers generated from a graph, returned by [Graph::statistics()].
///
/// Note that the segment counts aren't mutually exclusive, so the sum of these fields can be more
/// than the total of segments.
#[derive(Default, Debug, Clone)]
pub struct Statistics {
    /// The number of segments in the graph.
    pub segments: usize,
    /// Segments where all commits are integrated.
    pub segments_integrated: usize,
    /// Segments where all commits are on a remote tracking branch.
    pub segments_remote: usize,
    /// Segments where the remote tracking branch is set
    pub segments_with_remote_tracking_branch: usize,
    /// Segments that are empty.
    pub segments_empty: usize,
    /// Segments that are anonymous.
    pub segments_unnamed: usize,
    /// Segments that are reachable by the workspace commit.
    pub segments_in_workspace: usize,
    /// Segments that are reachable by the workspace commit and are integrated.
    pub segments_in_workspace_and_integrated: usize,
    /// Segments that have metadata for workspaces.
    pub segments_with_workspace_metadata: usize,
    /// Segments that have metadata for branches.
    pub segments_with_branch_metadata: usize,
    /// `true` if the start of the traversal is in a workspace.
    /// `None` if the information could not be determined, maybe because the entrypoint
    /// is invalid (bug) or it's empty (unusual)
    pub entrypoint_in_workspace: Option<bool>,
    /// Segments, excluding the entrypoint, that can be reached downwards through the entrypoint.
    pub segments_behind_of_entrypoint: usize,
    /// Segments, excluding the entrypoint, that can be reached upwards through the entrypoint.
    pub segments_ahead_of_entrypoint: usize,
    /// The entrypoint of the graph traversal.
    pub entrypoint: (SegmentIndex, Option<CommitIndex>),
    /// The number of incoming connections into the entrypoint segment.
    pub segment_entrypoint_incoming: usize,
    /// The number of outgoing connections into the entrypoint segment.
    pub segment_entrypoint_outgoing: usize,
    /// Segments without incoming connections.
    pub top_segments: Vec<(
        Option<gix::refs::FullName>,
        SegmentIndex,
        Option<CommitFlags>,
    )>,
    /// Segments without outgoing connections.
    pub segments_at_bottom: usize,
    /// Connections between segments.
    pub connections: usize,
    /// All commits within segments.
    pub commits: usize,
    /// All references stored with commits, i.e. not the ref-names absorbed by segments.
    pub commit_references: usize,
    /// The traversal was stopped at this many commits.
    pub commits_at_cutoff: usize,
}
