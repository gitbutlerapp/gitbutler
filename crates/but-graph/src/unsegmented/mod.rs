//! An unsegmented graph. Like an ant that had it's body parts fused together in
//! a tragic golfing accident.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use petgraph::{Direction, graph::NodeIndex, stable_graph::StableDiGraph, visit::EdgeRef as _};

use crate::{Commit, Graph, SegmentIndex};

/// A node in the unsegmented graph
pub enum Node {
    /// A commit step. See [`crate::Commit`] for details.
    Commit(Commit),
    /// A reference step.
    Reference(gix::refs::FullName),
    /// A node whose parents are it's children's parents
    None,
}

struct Edge {
    order: usize,
}

type UsGraphIdx = NodeIndex;
type UnsegmentedGraph = StableDiGraph<Node, Edge, UsGraphIdx>;

/// Like the but-graph, but without segments
pub struct UsGraph {
    inner: UnsegmentedGraph,
}

impl UsGraph {
    /// Make a [`UsGraph`] out of a a segmented [`crate::Graph`]
    ///
    /// Currently requires a gix repo to ensure that edge ordering is valid.
    pub fn create(but_graph: &Graph, repo: &gix::Repository) -> Result<Self> {
        // This first creates runs of nodes and associates them with the
        // but-graph segments. We then do a second pass over all the segments
        // and use the but_graph to connect up the runs. Finally, we validate
        // that each Pick step's parents match the commit's actual parents,
        // and if not, we disconnect and rewire directly to the correct
        // parent commits.

        /*
        let workspace_commit_id = but_graph.managed_entrypoint_commit(repo)?.map(|c| c.id);

        let mut commits: Vec<Commit> = vec![];
        let mut commit_to_idx = HashMap::<gix::ObjectId, SegmentIndex>::new();
        let mut commit_to_pick_ix = HashMap::<gix::ObjectId, UsGraphIdx>::new();
        let mut graph = UnsegmentedGraph::new();
        let mut head_selectors = vec![];
        let mut references = vec![];
        struct NodeSegment {
            nodes: Vec<UsGraphIdx>,
        }

        let mut segments = HashMap::<SegmentIndex, NodeSegment>::new();

        for sid in but_graph.node_indices() {
            let segment = &but_graph[sid];
            let mut nodes = vec![];

            if let Some(reference) = segment.ref_name() {
                let refname = reference.to_owned();
                references.push(refname.clone());
                let ix = graph.add_node(Node::Reference(refname.clone()));
                nodes.push(ix);
            }

            for commit in &segment.commits {
                commits.push(commit.clone());
                commit_to_idx.insert(commit.id, segment.id);

                let refs = commit
                    .refs
                    .iter()
                    .map(|r| r.ref_name.clone())
                    .collect::<Vec<_>>();

                for reference in refs {
                    references.push(reference.to_owned());
                    let ix = graph.add_node(Node::Reference(reference.clone()));
                    if let Some(previous_ix) = nodes.last() {
                        graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                    }
                    nodes.push(ix);
                }

                let ix = graph.add_node(Node::Commit(commit.clone()));
                commit_to_pick_ix.insert(commit.id, ix);
                if let Some(previous_ix) = nodes.last() {
                    graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                }
                nodes.push(ix);
            }

            if nodes.is_empty() {
                tracing::debug!("Empty node added - this is probably impossible");
                let ix = graph.add_node(Node::None);
                nodes.push(ix);
            }

            segments.insert(segment.id, NodeSegment { nodes });
        }

        let commit_ids = commits.iter().map(|c| c.id).collect::<HashSet<_>>();

        for sidx in segments.keys() {
            let Some(source) = segments.get(sidx).and_then(|n| n.nodes.last()) else {
                continue;
            };

            'inner: for (order, edge) in but_graph
                .edges_directed(*sidx, Direction::Outgoing)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .enumerate()
            {
                let Some(target) = segments.get(&edge.target()).and_then(|n| n.nodes.first())
                else {
                    tracing::warn!(
                        "Dropping parent edge for segment {sidx:?}: edge target {:?} has no nodes",
                        edge.target()
                    );
                    continue 'inner;
                };

                graph.add_edge(*source, *target, Edge { order });
            }
        }

        for c in &commits {
            if Some(c.id) == workspace_commit_id {
                continue;
            }

            let Some(&pick_ix) = commit_to_pick_ix.get(&c.id) else {
                continue;
            };

            // Skip commits with preserved parents (partial traversal — already handled above)
            if let ::Pick(Pick {
                preserved_parents: Some(_),
                ..
            }) = &graph[pick_ix]
            {
                continue;
            }

            // Resolve what the graph thinks are the parents of this pick
            let graph_parents = util::collect_ordered_parents(&graph, pick_ix);
            let graph_parent_ids: Vec<gix::ObjectId> = graph_parents
                .iter()
                .filter_map(|idx| match &graph[*idx] {
                    Step::Pick(Pick { id, .. }) => Some(*id),
                    _ => None,
                })
                .collect();

            if graph_parent_ids == c.parent_ids {
                continue;
            }

            tracing::warn!(
                "usgraph inconsistent with the commit graph.\nParents for commit {} do not match.\n\nFound:{:?}\nExpected:{:?}\n\nThese IDs may be in memory, but may be helpful for debugging.",
                c.id,
                graph_parent_ids
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>(),
                c.parent_ids
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>(),
            );

            let outgoing_edge_ids: Vec<_> = graph
                .edges_directed(pick_ix, Direction::Outgoing)
                .map(|e| e.id())
                .collect();
            for edge_id in outgoing_edge_ids {
                graph.remove_edge(edge_id);
            }

            'inner: for (order, parent_id) in c.parent_ids.iter().enumerate() {
                let Some(&target_ix) = commit_to_pick_ix.get(parent_id) else {
                    tracing::warn!(
                        "Dropping parent edge for commit {} (parent fix): parent {parent_id} not found in pick map",
                        c.id
                    );
                    continue 'inner;
                };

                graph.add_edge(pick_ix, target_ix, Edge { order });
            }
        }

        Ok(Self { inner: graph })
        */
        todo!()
    }
}
