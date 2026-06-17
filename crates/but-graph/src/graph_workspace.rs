//! A graph based workspace projection

use anyhow::Result;
use but_core::{WORKSPACE_REF_NAME, ref_metadata::ProjectMeta};
use petgraph::{Direction, visit::EdgeRef as _};
use std::collections::HashSet;

use crate::{
    EntryPointCommit, FirstParent, SegmentIndex, workspace::commit::is_managed_workspace_by_message,
};

/// A structure that gives a frame of reference to a key subgraph in the
/// workspace framing. This could be the subgraph of all commits above the
/// workspace, or the nodes that make up a "stack".
///
/// Rather than being a full graph structure, this provides pointers into the
/// main but_graph::Graph structure.
pub struct Subgraph {
    /// Nodes in the subgraph that only have incoming edges
    pub heads: Vec<SegmentIndex>,
    /// All the nodes in the specified subgraph
    pub nodes: HashSet<SegmentIndex>,
}

impl Subgraph {
    fn empty() -> Self {
        Self {
            heads: vec![],
            nodes: HashSet::new(),
        }
    }
}

/// Provides a frame of reference for the standardized view of the world.
///
/// This is intended to be used only inside the but-workspace crate.
pub struct GraphWorkspace {
    /// Pulls information of out of the
    pub graph: crate::Graph,

    /// If we're on the workspace branch, any commits in the rev-set
    /// `HEAD ^workspace_commit ^target_sha` will be included in this subgraph.
    pub above_workspace: Subgraph,

    /// If we are on the workspace branch, and a workspace commit can be found,
    /// this will be set.
    pub workspace_commit: Option<crate::Commit>,

    /// If we're on the workspace branch, this will contain a list of subgraphs
    /// that represents a stack. These are commits that follow the rev-set
    /// `workspace_commit_parents ^target_sha`
    ///
    /// We consider a stack beneath the workspace commit to be mutually
    /// exclusive sub-graphs of commits that don't have any incoming or outgoing
    /// edges to other commits in other stacks.
    ///
    /// As a natural extension, if we failed to find the workspace commit, this
    /// list will be empty since all the commits will deemed "above_workspace".
    ///
    /// If we're outside of the workspace branch, there will be one stack that
    /// contains all commits in the rev-set `HEAD ^target_sha`.
    pub stacks: Vec<Subgraph>,
}

impl GraphWorkspace {
    fn empty(graph: crate::Graph) -> Self {
        Self {
            graph,
            above_workspace: Subgraph::empty(),
            workspace_commit: None,
            stacks: vec![],
        }
    }

    /// Creates a [`GraphWorkspace`] from a [`crate::Graph`].
    ///
    /// A current implementation assumption is that if we have a workspace
    /// commit, it will be a segment that just contains itself. This will need
    /// to change.
    ///
    /// The need to sub-index into the but-graph is one that I'm not thrilled
    /// about given the not insignificant increase in complexity. I would lean
    /// towards mapping the but-graph to a stepped graph over sub-indexing the
    /// but-graph, especially since constructing a stepped graph can be done
    /// very quickly.
    pub fn create(
        repo: &gix::Repository,
        graph: crate::Graph,
        project_meta: &ProjectMeta,
    ) -> Result<GraphWorkspace> {
        let Some((entrypoint_sidx, EntryPointCommit::AtCommit(_entrypoint_commit))) =
            graph.entrypoint
        else {
            return Ok(Self::empty(graph));
        };

        // In the case of no target sha:
        // In PGM: We have one giant stack that contains all commits
        // In A workspace:
        //   If we find a workspace commit, we have stacks that reach the full history.
        //   If we don't find a workspace commit, all commits from HEAD are considered above the workspace.

        if graph.entrypoint_ref == Some(WORKSPACE_REF_NAME.try_into()?) {
            let workspace_commit = if let Some(target_commit_id) = project_meta.target_commit_id {
                // This _might_ bring in more commits than expected, if there
                // are commits above the target_commit_id in it's segment, but
                // this is not super likely and a test case for later...
                let target_commit_sidx = graph.segment_id_by_commit_id(target_commit_id)?;

                let mut out = None;
                'outer: for segment in graph.find_segments_reachable_from_a_not_b(
                    entrypoint_sidx,
                    target_commit_sidx,
                    FirstParent::No,
                ) {
                    for commit in &segment.commits {
                        let gix_commit = repo.find_commit(commit.id)?;

                        if is_managed_workspace_by_message(gix_commit.message_raw()?) {
                            out = Some((segment.id, commit.clone()));
                            break 'outer;
                        }
                    }
                }

                out
            } else {
                let mut out = None;
                graph.visit_all_segments_including_start_until(
                    entrypoint_sidx,
                    Direction::Outgoing,
                    |segment| {
                        'outer: for commit in &segment.commits {
                            if let Ok(gix_commit) = repo.find_commit(commit.id)
                                && let Ok(decoded_message) = gix_commit.message_raw()
                                && is_managed_workspace_by_message(decoded_message)
                            {
                                out = Some((segment.id, commit.clone()));
                                break 'outer;
                            }
                        }

                        out.is_some()
                    },
                );
                out
            };

            let target_commit_sidx = project_meta
                .target_commit_id
                .map(|t| graph.segment_id_by_commit_id(t))
                .transpose()?;

            let head_not_target_commit =
                all_commits_until_optional_limit(&graph, entrypoint_sidx, target_commit_sidx)?;

            if let Some((workspace_commit_sidx, workspace_commit)) = workspace_commit {
                let (above_workspace, stacks) = divide_workspace_into_stacks(
                    &graph,
                    head_not_target_commit,
                    workspace_commit_sidx,
                )?;

                Ok(Self {
                    graph,
                    above_workspace,
                    workspace_commit: Some(workspace_commit),
                    stacks,
                })
            } else {
                Ok(Self {
                    graph,
                    above_workspace: head_not_target_commit,
                    workspace_commit: None,
                    stacks: vec![],
                })
            }
        } else {
            // We're pegging

            // This _might_ bring in more commits than expected, if there
            // are commits above the target_commit_id in it's segment, but
            // this is not super likely and a test case for later...
            let target_commit_sidx = project_meta
                .target_commit_id
                .map(|t| graph.segment_id_by_commit_id(t))
                .transpose()?;

            let stack =
                all_commits_until_optional_limit(&graph, entrypoint_sidx, target_commit_sidx)?;

            Ok(Self {
                graph,
                above_workspace: Subgraph::empty(),
                workspace_commit: None,
                stacks: vec![stack],
            })
        }
    }
}

fn divide_workspace_into_stacks(
    graph: &crate::Graph,
    head_not_target_commit: Subgraph,
    workspace_commit_sidx: SegmentIndex,
) -> Result<(Subgraph, Vec<Subgraph>)> {
    let mut initial_stacks = graph
        .edges_directed(workspace_commit_sidx, Direction::Outgoing)
        .map(|edge| Subgraph {
            heads: vec![edge.target()],
            nodes: [edge.target()].into(),
        })
        .collect::<Vec<_>>();

    for stack in &mut initial_stacks {
        let mut tips = stack.heads.clone();
        while let Some(tip) = tips.pop() {
            for edge in graph.edges_directed(tip, Direction::Outgoing) {
                if !head_not_target_commit.nodes.contains(&edge.target()) {
                    continue;
                }

                if stack.nodes.insert(edge.target()) {
                    tips.push(edge.target())
                }
            }
        }
    }

    let mut deduplicated_stacks = vec![];
    while let Some(mut out) = initial_stacks.pop() {
        for bix in (0..initial_stacks.len()).rev() {
            #[expect(clippy::indexing_slicing)]
            if out
                .nodes
                .iter()
                .any(|o| initial_stacks[bix].nodes.contains(o))
            {
                let b = initial_stacks.swap_remove(bix);

                out.nodes.extend(b.nodes);
                out.heads.extend(b.heads);
            }
        }

        deduplicated_stacks.push(out);
    }

    let mut outside_nodes = head_not_target_commit.nodes.clone();
    for stack in &deduplicated_stacks {
        outside_nodes = outside_nodes.difference(&stack.nodes).cloned().collect();
    }

    let above_workspace = Subgraph {
        heads: head_not_target_commit
            .heads
            .iter()
            .cloned()
            .filter(|h| *h == workspace_commit_sidx)
            .collect(),
        nodes: outside_nodes,
    };

    Ok((above_workspace, deduplicated_stacks))
}

fn all_commits_until_optional_limit(
    graph: &crate::Graph,
    start: SegmentIndex,
    limit: Option<SegmentIndex>,
) -> Result<Subgraph> {
    let all_segments = if let Some(limit) = limit {
        graph
            .find_segments_reachable_from_a_not_b(start, limit, FirstParent::No)
            .map(|s| s.id)
            .collect::<HashSet<_>>()
    } else {
        // all segments beneath the entrypoint
        let mut out = HashSet::new();

        graph.visit_all_segments_including_start_until(start, Direction::Outgoing, |segment| {
            out.insert(segment.id);
            false
        });

        out
    };

    Ok(Subgraph {
        heads: vec![start],
        nodes: all_segments,
    })
}

// /// An enriched segment that represents a linear portion of a subgraph.
// ///
// /// Having segments has been a request from the Sam and Olly.
// pub struct UiSegment<Row> {
//     rows: Vec<Row>,
// }

// /// An enriched workspace that provides all the information that the frontend
// /// might want to display.
// ///
// /// This representation uses the renderdag library to convert the graph
// /// structures into lists render-able line drawing instructions.
// ///
// /// The `Row` type parameter represents an output of renderdag which is either a
// /// commit or reference.
// pub struct UiWorkspace<Row> {
//     pub above_workspace: Vec<UiSegment<Row>>,
// }
