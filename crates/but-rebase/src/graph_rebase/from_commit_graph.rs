//! SPIKE (commit-graph-experiment): build a [`StepGraph`] directly from a commit-first
//! [`but_graph::CommitGraph`], with no segment graph involved.
//!
//! Today `creation.rs` builds the StepGraph by walking segments, then *corrects* it (steps 7-8):
//! for every pick whose graph-derived parents disagree with the commit's real `parent_ids`, it
//! rips out the edges and re-adds them straight from `parent_ids`. That correction is the whole job
//! once you have a commit graph — so here it IS the construction:
//!
//! * one [`Step::Pick`] per commit (a workspace pick for the workspace commit),
//! * one [`Step::Reference`] per ref on a commit, applied at that pick,
//! * one parent edge per `parent_ids` entry, `order` = its index in the parent array.
//!
//! No `Connection.src_id`/`dst_id` lookups, no segment iteration, no insertion-order tie-break.
//! Parents missing from the graph (a partial traversal) are preserved on the pick, exactly as
//! `creation.rs` does.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use crate::graph_rebase::{Edge, Pick, Step, StepGraph, StepGraphIndex, util};

/// A commit's id mapped to its ordered parent commit ids — the rebase topology.
pub(crate) type ParentMap = BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>;

/// Build a [`StepGraph`] straight from `cg`. See the module docs.
pub(crate) fn step_graph_from_commit_graph(
    cg: &but_graph::CommitGraph,
    workspace_commit_id: Option<gix::ObjectId>,
) -> StepGraph {
    let mut graph = StepGraph::new();
    let mut commit_to_pick: HashMap<gix::ObjectId, StepGraphIndex> = HashMap::new();

    // A Pick (and any References) per commit.
    for id in cg.commit_ids() {
        let pick = if workspace_commit_id == Some(id) {
            Pick::new_workspace_pick(id)
        } else {
            Pick::new_pick(id)
        };
        let pick_ix = graph.add_node(Step::Pick(pick));
        commit_to_pick.insert(id, pick_ix);
        // Each ref on the commit is applied at this pick (Reference -> Pick).
        for refname in cg.refs_at(id) {
            let ref_ix = graph.add_node(Step::Reference { refname });
            graph.add_edge(ref_ix, pick_ix, Edge { order: 0 });
        }
    }

    // Parent edges read straight off `parent_ids`; `order` = position in that array.
    for id in cg.commit_ids() {
        let pick_ix = commit_to_pick[&id];
        let parents = cg.all_parent_ids(id);
        let mut has_missing = false;
        for (order, parent) in parents.iter().enumerate() {
            match commit_to_pick.get(parent) {
                Some(&parent_ix) => {
                    graph.add_edge(pick_ix, parent_ix, Edge { order });
                }
                None => has_missing = true,
            }
        }
        // Partial graph: preserve the original parents rather than re-pointing them.
        if has_missing
            && !parents.is_empty()
            && let Step::Pick(p) = &mut graph[pick_ix]
        {
            p.preserved_parents = Some(parents);
        }
    }

    graph
}

/// Each Pick's commit id → its ordered parent commit ids (descending through References) — i.e. the
/// rebase topology. Used to compare two StepGraphs for equivalence regardless of node numbering.
pub(crate) fn ordered_parent_commits(graph: &StepGraph) -> ParentMap {
    let mut map = BTreeMap::new();
    for ix in graph.node_indices() {
        if let Step::Pick(p) = &graph[ix] {
            let parents = util::collect_ordered_parents(graph, ix)
                .into_iter()
                .filter_map(|pix| match &graph[pix] {
                    Step::Pick(pp) => Some(pp.id),
                    _ => None,
                })
                .collect();
            map.insert(p.id, parents);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_graph::{Commit, CommitFlags, CommitGraph};

    fn oid(b: u8) -> gix::ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = b;
        gix::ObjectId::from_bytes_or_panic(&bytes)
    }

    fn commit(b: u8, parents: &[u8]) -> Commit {
        Commit {
            id: oid(b),
            parent_ids: parents.iter().map(|&p| oid(p)).collect(),
            flags: CommitFlags::empty(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn linear_history_picks_chain_in_parent_order() {
        // c -> b -> a (child -> parent).
        let cg = CommitGraph::from_commits(
            [commit(0xC, &[0xB]), commit(0xB, &[0xA]), commit(0xA, &[])],
            Some(oid(0xC)),
        );
        let graph = step_graph_from_commit_graph(&cg, None);
        let map = ordered_parent_commits(&graph);
        assert_eq!(map[&oid(0xC)], vec![oid(0xB)]);
        assert_eq!(map[&oid(0xB)], vec![oid(0xA)]);
        assert_eq!(map[&oid(0xA)], Vec::<gix::ObjectId>::new());
    }

    #[test]
    fn merge_keeps_first_parent_order_from_commit_data_alone() {
        // Diamond: m merges a (first) and b (second); both on base.
        let cg = CommitGraph::from_commits(
            [
                commit(0x4, &[0x2, 0x3]),
                commit(0x2, &[0x1]),
                commit(0x3, &[0x1]),
                commit(0x1, &[]),
            ],
            Some(oid(0x4)),
        );
        let graph = step_graph_from_commit_graph(&cg, None);
        let map = ordered_parent_commits(&graph);
        // The merge's parent order comes straight from parent_ids — no Connection payload needed.
        assert_eq!(map[&oid(0x4)], vec![oid(0x2), oid(0x3)]);
        assert_eq!(map[&oid(0x2)], vec![oid(0x1)]);
        assert_eq!(map[&oid(0x3)], vec![oid(0x1)]);
        assert_eq!(map[&oid(0x1)], Vec::<gix::ObjectId>::new());
    }

    #[test]
    fn missing_parents_are_preserved_for_a_partial_graph() {
        // `a`'s parent `0x9` is not in the graph (partial traversal).
        let cg = CommitGraph::from_commits([commit(0xA, &[0x9])], Some(oid(0xA)));
        let graph = step_graph_from_commit_graph(&cg, None);
        let ix = graph
            .node_indices()
            .find(|&ix| matches!(&graph[ix], Step::Pick(p) if p.id == oid(0xA)))
            .unwrap();
        let Step::Pick(p) = &graph[ix] else {
            unreachable!()
        };
        assert_eq!(p.preserved_parents.as_deref(), Some([oid(0x9)].as_slice()));
    }
}
