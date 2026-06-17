//! A graph based workspace projection, framed from the rebase [`Editor`].
//!
//! Rather than being its own graph, this points into the editor's internal step
//! graph via [`Selector`]s, so consumers can frame the mutations they're about
//! to perform against the same selectors they'll act on.

use std::collections::HashSet;

use anyhow::Result;
use but_core::{RefMetadata, WORKSPACE_REF_NAME};
use but_graph::workspace::commit::is_managed_workspace_by_message;
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{Checkout, Editor, Pick, Selector, Step, StepGraph, StepGraphIndex};

/// A structure that gives a frame of reference to a key subgraph in the
/// workspace framing. This could be the subgraph of all commits above the
/// workspace, or the nodes that make up a "stack".
///
/// Rather than being a full graph structure, this provides pointers into the
/// editor's internal step graph.
pub struct Subgraph {
    /// Nodes in the subgraph that only have incoming edges
    pub heads: Vec<Selector>,
    /// All the nodes in the specified subgraph
    pub nodes: HashSet<Selector>,
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
    /// If we're on the workspace branch, any commits in the rev-set
    /// `HEAD ^workspace_commit ^target_sha` will be included in this subgraph.
    pub above_workspace: Subgraph,

    /// If we are on the workspace branch, and a workspace commit can be found,
    /// this will be set.
    pub workspace_commit: Option<Selector>,

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
    ///
    /// # Known limitation: stacks sharing a target segment collapse into one
    ///
    /// Today, stacks that converge on a shared segment - most importantly the
    /// target (`origin/main`) segment every real workspace stack sits on - get
    /// merged into a single stack instead of staying separate. This is a
    /// consequence of how the editor's step graph is built, *not* of the rebase
    /// topology, so a fixture can look like N obviously-distinct stacks and
    /// still come back as one.
    ///
    /// The segment's head reference becomes its first node (see `Editor::create`
    /// in `creation.rs`), and each child stack attaches to that node. So when
    /// two stacks share the target segment, they both point at its ref node and
    /// the split treats them as one. A target doesn't help: it excludes the
    /// target *commit*, but the ref node sits above that commit and survives.
    ///
    /// In this scenario, the but graph really ought to be providing a graph
    /// that doesn't let us put the node there.
    pub stacks: Vec<Subgraph>,
}

impl GraphWorkspace {
    fn empty() -> Self {
        Self {
            above_workspace: Subgraph::empty(),
            workspace_commit: None,
            stacks: vec![],
        }
    }
}

/// The index-level analog of [`Subgraph`], used internally so the traversal and
/// set-algebra stay on cheap `StepGraphIndex`es; converted to selectors once at
/// the boundary.
struct NodeSet {
    heads: Vec<StepGraphIndex>,
    nodes: HashSet<StepGraphIndex>,
}

impl NodeSet {
    /// Convert into a [`Subgraph`] by pointing every index at `revision` - the
    /// editor revision the node set was traversed against.
    fn into_subgraph(self, revision: usize) -> Subgraph {
        Subgraph {
            heads: self
                .heads
                .into_iter()
                .map(|id| Selector { id, revision })
                .collect(),
            nodes: self
                .nodes
                .into_iter()
                .map(|id| Selector { id, revision })
                .collect(),
        }
    }
}

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Build a graph-based workspace projection framed from this editor.
    pub fn graph_workspace(&self) -> Result<GraphWorkspace> {
        let Some(entrypoint_ix) = self.head_index() else {
            return Ok(GraphWorkspace::empty());
        };

        // In the case of no target sha:
        // In PGM: We have one giant stack that contains all commits
        // In A workspace:
        //   If we find a workspace commit, we have stacks that reach the full history.
        //   If we don't find a workspace commit, all commits from HEAD are considered above the workspace.

        let ws_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
        let on_workspace = matches!(
            &self.graph[entrypoint_ix],
            Step::Reference { refname } if *refname == ws_ref
        );

        let target_ix = self.target_selector().map(|s| s.id);
        let revision = self.history.current_revision();

        if on_workspace {
            let head_not_target_commit =
                all_commits_until_optional_limit(&self.graph, entrypoint_ix, target_ix);

            // The workspace commit, if present, lives somewhere in `HEAD ^target`.
            let workspace_commit = head_not_target_commit.nodes.iter().copied().find_map(|ix| {
                let Step::Pick(Pick { id, .. }) = &self.graph[ix] else {
                    return None;
                };
                let gix_commit = self.repo.find_commit(*id).ok()?;
                is_managed_workspace_by_message(gix_commit.message_raw().ok()?).then_some(ix)
            });

            if let Some(workspace_commit_ix) = workspace_commit {
                let (above_workspace, stacks) = divide_workspace_into_stacks(
                    &self.graph,
                    head_not_target_commit,
                    workspace_commit_ix,
                );

                Ok(GraphWorkspace {
                    above_workspace: above_workspace.into_subgraph(revision),
                    workspace_commit: Some(self.new_selector(workspace_commit_ix)),
                    stacks: stacks
                        .into_iter()
                        .map(|s| s.into_subgraph(revision))
                        .collect(),
                })
            } else {
                Ok(GraphWorkspace {
                    above_workspace: head_not_target_commit.into_subgraph(revision),
                    workspace_commit: None,
                    stacks: vec![],
                })
            }
        } else {
            // We're pegging.
            let stack = all_commits_until_optional_limit(&self.graph, entrypoint_ix, target_ix);

            Ok(GraphWorkspace {
                above_workspace: Subgraph::empty(),
                workspace_commit: None,
                stacks: vec![stack.into_subgraph(revision)],
            })
        }
    }

    /// The entrypoint (`HEAD`) reference node, or `None` if HEAD isn't on a ref.
    fn head_index(&self) -> Option<StepGraphIndex> {
        self.checkouts
            .iter()
            .find_map(|Checkout::Head { selector, .. }| {
                self.history
                    .normalize_selector(*selector)
                    .ok()
                    .map(|s| s.id)
            })
    }

    /// The target commit's node, if a target is configured and present.
    fn target_selector(&self) -> Option<Selector> {
        let target = self.workspace.graph.project_meta.target_commit_id?;
        let selector = self.try_select_commit(target)?;
        self.history.normalize_selector(selector).ok()
    }
}

/// Every step reachable from `start` following parent edges (`Outgoing`).
fn reachable_from(graph: &StepGraph, start: StepGraphIndex) -> HashSet<StepGraphIndex> {
    let mut out = HashSet::new();
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        if !out.insert(n) {
            continue;
        }
        stack.extend(
            graph
                .edges_directed(n, Direction::Outgoing)
                .map(|e| e.target()),
        );
    }
    out
}

/// The rev-set `start ^excluded`: steps reachable from `start` but not
/// `excluded`.
///
/// We could write a more efficient algorithm, but I'm keen on first focusing on
/// the correctness of the rest of the system before delving too deep into the
/// weeds WRT performance.
fn a_not_b(
    graph: &StepGraph,
    start: StepGraphIndex,
    excluded: StepGraphIndex,
) -> HashSet<StepGraphIndex> {
    let excluded_set = reachable_from(graph, excluded);
    let mut out = HashSet::new();
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        if excluded_set.contains(&n) || !out.insert(n) {
            continue;
        }
        stack.extend(
            graph
                .edges_directed(n, Direction::Outgoing)
                .map(|e| e.target()),
        );
    }
    out
}

/// All steps in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`. The analog of the original `all_commits_until_optional_limit`.
fn all_commits_until_optional_limit(
    graph: &StepGraph,
    start: StepGraphIndex,
    limit: Option<StepGraphIndex>,
) -> NodeSet {
    let nodes = match limit {
        Some(limit) => a_not_b(graph, start, limit),
        None => reachable_from(graph, start),
    };
    NodeSet {
        heads: vec![start],
        nodes,
    }
}

/// Split the region beneath the workspace commit into mutually-exclusive stacks,
/// returning `(above_workspace, stacks)`.
fn divide_workspace_into_stacks(
    graph: &StepGraph,
    head_not_target: NodeSet,
    workspace_commit_ix: StepGraphIndex,
) -> (NodeSet, Vec<NodeSet>) {
    // Each parent of the workspace commit seeds a stack.
    let mut initial_stacks = graph
        .edges_directed(workspace_commit_ix, Direction::Outgoing)
        .map(|edge| NodeSet {
            heads: vec![edge.target()],
            nodes: [edge.target()].into(),
        })
        .collect::<Vec<_>>();

    for stack in &mut initial_stacks {
        let mut tips = stack.heads.clone();
        while let Some(tip) = tips.pop() {
            for edge in graph.edges_directed(tip, Direction::Outgoing) {
                if !head_not_target.nodes.contains(&edge.target()) {
                    continue;
                }
                if stack.nodes.insert(edge.target()) {
                    tips.push(edge.target());
                }
            }
        }
    }

    // Merge stacks that share any node (they aren't actually distinct).
    //
    // NOTE: a shared node here includes *reference* nodes, not just commits.
    // A segment's head ref is its first node (see `creation.rs`), so stacks that
    // converge on a shared segment - typically the target's - both point at its
    // ref node and collapse into one, even when a target excludes the segment's
    // commit. This is the known limitation documented on `GraphWorkspace::stacks`.
    let mut deduplicated = vec![];
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
        deduplicated.push(out);
    }

    let mut outside = head_not_target.nodes.clone();
    for stack in &deduplicated {
        outside = outside.difference(&stack.nodes).copied().collect();
    }
    outside.remove(&workspace_commit_ix);

    let above_workspace = NodeSet {
        // The entrypoint is the tip of everything above the workspace commit.
        heads: head_not_target
            .heads
            .iter()
            .cloned()
            .filter(|h| *h != workspace_commit_ix)
            .collect(),
        nodes: outside,
    };

    (above_workspace, deduplicated)
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

#[cfg(test)]
mod test {
    use std::str::FromStr as _;

    use super::{a_not_b, reachable_from};
    use crate::graph_rebase::{Edge, Step, StepGraph, StepGraphIndex};

    fn pick(graph: &mut StepGraph) -> StepGraphIndex {
        let id = gix::ObjectId::from_str("1000000000000000000000000000000000000000").unwrap();
        graph.add_node(Step::new_pick(id))
    }

    /// `a -> b -> base` and `c -> base` (edges point child -> parent).
    /// `a ^c` must drop `base` (shared with `c`) but keep `a`, `b`.
    #[test]
    fn a_not_b_excludes_shared_ancestry() {
        let mut g = StepGraph::new();
        let a = pick(&mut g);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.add_edge(a, b, Edge { order: 0 });
        g.add_edge(b, base, Edge { order: 0 });
        g.add_edge(c, base, Edge { order: 0 });

        assert_eq!(a_not_b(&g, a, c), [a, b].into_iter().collect());
        assert_eq!(reachable_from(&g, a), [a, b, base].into_iter().collect());
    }
}
