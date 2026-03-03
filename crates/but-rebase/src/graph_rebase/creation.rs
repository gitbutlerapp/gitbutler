use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_graph::{Commit, Graph, SegmentIndex};
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{
    Checkout, Edge, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, StepGraphIndex,
    SuccessfulRebase, util,
};

/// Provides an extension for creating an Editor out of the segment graph
pub trait GraphExt {
    /// Creates an editor.
    fn to_editor(&self, repo: &gix::Repository) -> Result<Editor>;
}

impl GraphExt for Graph {
    /// Creates an editor out of the segment graph.
    fn to_editor(&self, repo: &gix::Repository) -> Result<Editor> {
        // This first creates runs of nodes and associates them with the
        // but-graph segments. We then do a second pass over all the segments
        // and use the but_graph to connect up the runs. Finally, we validate
        // that each Pick step's parents match the commit's actual parents,
        // and if not, we disconnect and rewire directly to the correct
        // parent commits.

        // TODO(CTO): Look into traversing "in workspace" segments that are not
        // reachable from the entrypoint TODO(CTO): Look into stopping at the
        // common base
        let entrypoint = self.lookup_entrypoint()?;
        let workspace_commit_id = self.managed_entrypoint_commit(repo)?.map(|c| c.id);

        let mut commits: Vec<Commit> = vec![];
        let mut commit_to_idx = HashMap::<gix::ObjectId, SegmentIndex>::new();
        let mut commit_to_pick_ix = HashMap::<gix::ObjectId, StepGraphIndex>::new();
        let mut graph = StepGraph::new();
        let mut head_selectors = vec![];
        let mut references = vec![];
        struct NodeSegment {
            nodes: Vec<StepGraphIndex>,
        }

        let mut segments = HashMap::<SegmentIndex, NodeSegment>::new();

        self.visit_all_segments_including_start_until(
            entrypoint.segment_index,
            Direction::Outgoing,
            |segment| {
                let mut nodes = vec![];

                if let Some(reference) = segment.ref_name() {
                    let refname = reference.to_owned();
                    references.push(refname.clone());
                    let ix = graph.add_node(Step::Reference {
                        refname: refname.clone(),
                    });
                    if Some(reference) == entrypoint.segment.ref_name() {
                        head_selectors.push(Selector {
                            id: ix,
                            revision: 0,
                        });
                    }
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
                        let ix = graph.add_node(Step::Reference {
                            refname: reference.clone(),
                        });
                        if let Some(previous_ix) = nodes.last() {
                            graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                        }
                        nodes.push(ix);
                    }

                    let pick = if workspace_commit_id == Some(commit.id) {
                        Pick::new_workspace_pick(commit.id)
                    } else {
                        Pick::new_pick(commit.id)
                    };
                    let ix = graph.add_node(Step::Pick(pick));
                    commit_to_pick_ix.insert(commit.id, ix);
                    if let Some(previous_ix) = nodes.last() {
                        graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                    }
                    nodes.push(ix);
                }

                if nodes.is_empty() {
                    tracing::debug!("Empty node added - this is probably impossible");
                    let ix = graph.add_node(Step::None);
                    nodes.push(ix);
                }

                segments.insert(segment.id, NodeSegment { nodes });

                false
            },
        );

        let commit_ids = commits.iter().map(|c| c.id).collect::<HashSet<_>>();

        for c in &commits {
            let has_no_parents = c.parent_ids.is_empty();
            let missing_parent_steps = c.parent_ids.iter().any(|p| !commit_ids.contains(p));

            // If the commit has parents, but at least one of them is not
            // in the graph, this means but-graph did a partial traversal
            // and we want to preserve the commit as it is.
            if !has_no_parents && missing_parent_steps {
                let Some(idx) = commit_to_pick_ix.get(&c.id) else {
                    bail!("BUG: Listed commit does not have corresponding idx.");
                };

                let Step::Pick(pick) = &mut graph[*idx] else {
                    bail!("BUG: Listed commit does not have corresponding pick step.");
                };

                pick.preserved_parents = Some(c.parent_ids.clone());
            };
        }

        for sidx in segments.keys() {
            let Some(source) = segments.get(sidx).and_then(|n| n.nodes.last()) else {
                continue;
            };

            'inner: for (order, edge) in self
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
            if let Step::Pick(Pick {
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
                "but-graph inconsistent with the commit graph.\nParents for commit {} do not match.\n\nFound:{:?}\nExpected:{:?}\n\nThese IDs may be in memory, but may be helpful for debugging.",
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

        Ok(Editor {
            graph,
            initial_references: references,
            // TODO(CTO): We need to eventually list all worktrees that we own
            // here so we can `safe_checkout` them too.
            checkouts: head_selectors.into_iter().map(Checkout::Head).collect(),
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
        })
    }
}

impl SuccessfulRebase {
    /// Converts a SuccessfulRebase back into another editor for multi-step operations
    pub fn to_editor(self) -> Editor {
        Editor {
            graph: self.graph,
            initial_references: self.initial_references,
            checkouts: self.checkouts,
            repo: self.repo,
            history: self.history,
        }
    }
}
