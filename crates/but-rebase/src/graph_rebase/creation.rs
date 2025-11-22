use std::collections::BTreeMap;

use anyhow::Result;
use but_graph::{Commit, CommitFlags, Graph, Segment};
use petgraph::Direction;

use crate::graph_rebase::{Edge, Editor, Step, StepGraph, StepGraphIndex};

/// Provides an extension for creating an Editor out of the segment graph
pub trait GraphExt {
    /// Creates an editor.
    fn create_editor(&self) -> Result<Editor>;
}

impl GraphExt for Graph {
    /// Creates an editor out of the segment graph.
    fn create_editor(&self) -> Result<Editor> {
        // TODO(CTO): Look into traversing "in workspace" segments that are not reachable from the entrypoint
        // TODO(CTO): Look into stopping at the common base
        let entrypoint = self.lookup_entrypoint()?;

        // Commits in this list are ordred such that iterating in reverse will
        // have any relevant parent commits already inserted in the graph.
        let mut commits: Vec<Commit> = Vec::new();
        // References are ordered from child-most to parent-most
        let mut references: BTreeMap<gix::ObjectId, Vec<gix::refs::FullName>> = BTreeMap::new();

        self.visit_all_segments_including_start_until(
            entrypoint.segment_index,
            Direction::Outgoing,
            |segment| {
                // Make a note to create a reference for named segments
                if let Some(refname) = segment.ref_name()
                    && let Some(commit) = find_nearest_commit(self, segment)
                {
                    references
                        .entry(commit.id)
                        .and_modify(|rs| rs.push(refname.to_owned()))
                        .or_insert_with(|| vec![refname.to_owned()]);
                }

                // Make a note to create a references that sit on commits
                for commit in &segment.commits {
                    if !commit.refs.is_empty() {
                        commit.flags.contains(CommitFlags::InWorkspace);
                        let refs = commit
                            .refs
                            .iter()
                            .map(|r| r.ref_name.clone())
                            .collect::<Vec<_>>();
                        if let Some(entry) = references.get_mut(&commit.id) {
                            entry.extend(refs);
                        } else {
                            references.insert(commit.id, refs);
                        }
                    }
                }

                commits.extend(segment.commits.clone());

                false
            },
        );

        // When adding child-nodes, this lookup tells us where to find the
        // relevant "parent" to point to.
        let mut steps_for_commits: BTreeMap<gix::ObjectId, StepGraphIndex> = BTreeMap::new();
        let mut graph = StepGraph::new();

        let mut last_inserted = None;
        while let Some(c) = commits.pop() {
            let mut ni = graph.add_node(Step::Pick { id: c.id });

            for (order, p) in c.parent_ids.iter().enumerate() {
                if let Some(parent_ni) = steps_for_commits.get(p) {
                    graph.add_edge(ni, *parent_ni, Edge { order });
                }
            }

            if let Some(refs) = references.get_mut(&c.id) {
                // We insert in reverse to preserve the child-most to
                // parent-most ordering that the frontend sees in the step graph
                for r in refs.iter().rev() {
                    let ref_ni = graph.add_node(Step::Reference { refname: r.clone() });
                    graph.add_edge(ref_ni, ni, Edge { order: 0 });
                    ni = ref_ni;
                }
            }

            last_inserted = Some(ni.clone());
            steps_for_commits.insert(c.id, ni);
        }

        Ok(Editor {
            graph,
            initial_references: references.values().flatten().cloned().collect(),
            heads: last_inserted.into_iter().collect(),
        })
    }
}

/// Find the commit that is nearest to the top of the segment via a first parent
/// traversal.
fn find_nearest_commit<'graph>(
    graph: &'graph Graph,
    segment: &'graph Segment,
) -> Option<&'graph Commit> {
    let mut target = Some(segment);
    while let Some(s) = target {
        if let Some(c) = s.commits.first() {
            return Some(c);
        }

        target = graph
            .segments_below_in_order(s.id)
            .next()
            .map(|s| &graph[s.1]);
    }

    None
}
