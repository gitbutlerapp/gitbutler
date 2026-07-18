use std::collections::BTreeSet;

use but_core::ref_metadata::{self, StackKind::Applied, StackKind::AppliedAndUnapplied};
use petgraph::Direction;

use crate::{CommitFlags, Graph, SegmentIndex, utils::SegmentVisitScratch};

impl Graph {
    /// Link anonymous workspace segments to known branches that continue outside the workspace.
    ///
    /// The legacy workspace projection uses this compatibility link to recover the branch name and
    /// outside commits without changing the underlying commit ancestry.
    pub(super) fn link_anonymous_workspace_siblings(
        &mut self,
        ws_sidx: SegmentIndex,
        ws_stacks: &[crate::workspace::Stack],
        ws_data: &ref_metadata::Workspace,
    ) {
        let unique_ws_segment_ids: BTreeSet<SegmentIndex> = ws_stacks
            .iter()
            .flat_map(|s| {
                s.segments
                    .iter()
                    .flat_map(|s| s.commits_by_segment.iter().map(|(sidx, _)| *sidx))
            })
            .collect();
        let mut segment_visit_scratch = SegmentVisitScratch::new(self);
        for &sidx in &unique_ws_segment_ids {
            // The workspace might be stale by now as empty segments are deleted.
            let Some(s) = self.inner.node_weight(sidx) else {
                continue;
            };
            if s.ref_info.is_some() || s.sibling_segment_id.is_some() {
                continue;
            }

            let num_incoming = self
                .inner
                .neighbors_directed(sidx, Direction::Incoming)
                .count();
            if num_incoming < 2 {
                continue;
            }

            let mut named_segment_id = None;
            segment_visit_scratch.visit_excluding_start_until(
                self,
                sidx,
                Direction::Incoming,
                |s| {
                    let prune = true;
                    if named_segment_id.is_some()
                        || s.commits
                            .first()
                            .is_some_and(|c| c.flags.contains(CommitFlags::InWorkspace))
                    {
                        return prune;
                    }

                    s.ref_info.as_ref().is_some_and(|ri| {
                        let is_known_to_workspace =
                            ws_data.contains_ref(ri.ref_name.as_ref(), AppliedAndUnapplied);
                        if is_known_to_workspace {
                            named_segment_id = Some(s.id);
                        }
                        is_known_to_workspace
                    })
                },
            );
            if let Some(named_sid) = named_segment_id
                && self[sidx].sibling_segment_id.is_none()
            {
                // Don't set sibling if the named segment is already known to the workspace
                // by direct connection. However, if the named segment is *not* a direct
                // workspace child and there are no further workspace segments below this
                // anonymous one, the sibling is the only way to identify the stack.
                let segment_name = self[named_sid]
                    .ref_info
                    .as_ref()
                    .expect("BUG: named segment must have name")
                    .ref_name
                    .as_ref();
                let is_stack_tip = ws_data.stack_names(Applied).any(|sn| sn == segment_name);
                if !is_stack_tip {
                    self[sidx].sibling_segment_id = Some(named_sid);
                } else {
                    let named_is_direct_parent = self
                        .inner
                        .neighbors_directed(sidx, Direction::Incoming)
                        .any(|n| n == named_sid);
                    let named_direct_parent_has_outside_commits = named_is_direct_parent && {
                        let mut has_outside_commits = false;
                        segment_visit_scratch.visit_including_start_until(
                            self,
                            named_sid,
                            Direction::Outgoing,
                            |segment| {
                                let prune = true;
                                if segment
                                    .commits
                                    .iter()
                                    .any(|c| c.flags.contains(CommitFlags::InWorkspace))
                                {
                                    return prune;
                                }
                                has_outside_commits |= !segment.commits.is_empty();
                                has_outside_commits
                            },
                        );
                        has_outside_commits
                    };
                    let named_is_direct_ws_child = self
                        .inner
                        .neighbors_directed(ws_sidx, Direction::Outgoing)
                        .any(|n| n == named_sid);
                    let has_ws_segments_below = self
                        .inner
                        .neighbors_directed(sidx, Direction::Outgoing)
                        .any(|n| unique_ws_segment_ids.contains(&n));
                    if named_direct_parent_has_outside_commits
                        || (!named_is_direct_ws_child && !has_ws_segments_below)
                    {
                        self[sidx].sibling_segment_id = Some(named_sid);
                    }
                }
            }
        }
    }

    /// Fill in generation numbers after segment construction and edge wiring.
    pub(super) fn compute_generation_numbers(&mut self) {
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
}
