//! New graphy workspace

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use but_graph::{
    MutableNodeGraph, NodeIndex, NodeKind,
    edit::Pick,
    workspace::{ReferenceStatus, Subgraph},
};
use gix::prelude::ObjectIdExt;
use renderdag::{Ancestor, GraphRowRenderer, LinkLine, NodeLine, PadLine, Renderer};

use crate::{ref_info::Commit, ui::CommitState};

pub(crate) fn find_segment_and_stack<'a>(
    workspace: &'a but_graph::Workspace,
    name: &gix::refs::FullNameRef,
) -> Option<(
    &'a but_graph::workspace::Stack,
    &'a but_graph::workspace::StackSegment,
)> {
    workspace.stacks.iter().find_map(|stack| {
        stack
            .segments
            .iter()
            .find_map(|segment| (segment.ref_name() == Some(name)).then_some((stack, segment)))
    })
}

pub(crate) fn find_workspace_commit(
    workspace: &but_graph::Workspace,
    id: gix::ObjectId,
) -> Option<&but_graph::workspace::StackCommit> {
    workspace
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .flat_map(|segment| &segment.commits)
        .find(|commit| commit.id == id)
}

pub(crate) fn workspace_tip_id(workspace: &but_graph::Workspace) -> Option<gix::ObjectId> {
    workspace
        .id
        .and_then(|id| node_commit_id(&workspace.graph, id))
}

pub(crate) fn node_commit_id(
    graph: &but_graph::Graph,
    index: but_graph::NodeIndex,
) -> Option<gix::ObjectId> {
    match graph.nodes().get(index)?.kind() {
        but_graph::NodeKind::Commit { id } => Some(*id),
        but_graph::NodeKind::Reference(reference) => reference.ref_info.commit_id,
        but_graph::NodeKind::Boundary { .. } | but_graph::NodeKind::None => None,
    }
}

pub(crate) fn workspace_is_entrypoint(workspace: &but_graph::Workspace) -> bool {
    workspace
        .stacks
        .iter()
        .all(|stack| stack.segments.iter().all(|segment| !segment.is_entrypoint))
}

pub(crate) fn workspace_contains_ref(
    workspace: &but_graph::Workspace,
    name: &gix::refs::FullNameRef,
) -> bool {
    find_segment_and_stack(workspace, name).is_some()
}

pub(crate) fn resolved_target_commit_id(workspace: &but_graph::Workspace) -> Option<gix::ObjectId> {
    workspace
        .stored_target_commit_id()
        .or_else(|| workspace.target_ref_tip_commit_id())
}

pub(crate) fn target_matches_branch(
    workspace: &but_graph::Workspace,
    name: &gix::refs::FullNameRef,
) -> bool {
    let Some(target) = workspace.target_ref.as_ref() else {
        return false;
    };
    target.ref_name.as_ref() == name
        || workspace
            .graph
            .node_by_ref_name(name)
            .is_some_and(|(_, reference)| {
                reference.remote_tracking_ref_name.as_ref() == Some(&target.ref_name)
            })
}

pub(crate) fn ref_reachable_from_entrypoint(
    workspace: &but_graph::Workspace,
    name: &gix::refs::FullNameRef,
) -> bool {
    if workspace
        .ref_name()
        .filter(|_| workspace_is_entrypoint(workspace))
        == Some(name)
    {
        return true;
    }
    if workspace_is_entrypoint(workspace) {
        return workspace_contains_ref(workspace, name);
    }
    let Some((stack, entrypoint)) = workspace.stacks.iter().find_map(|stack| {
        stack
            .segments
            .iter()
            .position(|segment| segment.is_entrypoint)
            .map(|index| (stack, index))
    }) else {
        return false;
    };
    stack
        .segments
        .get(entrypoint..)
        .into_iter()
        .flatten()
        .any(|segment| segment.ref_name() == Some(name))
}

/// A graph row's data
#[expect(clippy::large_enum_variant)]
pub enum GraphRowData {
    /// A commit :D
    Commit {
        /// The commit.
        commit: Commit,
        /// The commit's state (local-only / local-and-remote / integrated), as
        /// computed by the graph's workspace projection.
        state: CommitState,
    },
    /// A reference
    Reference {
        /// The name of the reference
        ref_name: gix::refs::FullName,
        /// More information about the reference, computed by the graph's
        /// workspace projection. `None` for references the projection didn't
        /// status (e.g. non-local-branch references).
        additional_ref_info: Option<ReferenceStatus>,
    },
}

/// A row in the graph
pub struct GraphRow {
    /// Data
    pub data: GraphRowData,

    /// The node columns for this row.
    pub node_line: Vec<NodeLine>,

    /// The link columns for this row, if a link row is necessary.
    pub link_line: Option<Vec<LinkLine>>,

    /// The location of any terminators, if necessary.  Other columns should be
    /// filled in with pad lines.
    pub term_line: Option<Vec<bool>>,

    /// The pad columns for this row.
    pub pad_lines: Vec<PadLine>,
}

/// A linear run of rows.
pub struct LinearSegment {
    /// The reference that starts this segment, if any.
    pub reference_idx: Option<usize>,
    /// The row indices in this segment.
    pub row_idxs: Vec<usize>,
}

/// A reference and the rows reachable from it, down to the next reference. A
/// commit reachable from more than one reference is included in each of them.
pub struct ReferenceSegment {
    /// The reference row index.
    pub reference_idx: usize,
    /// The row indices in this segment.
    pub row_idxs: Vec<usize>,
}

/// A stack
pub struct Stack {
    /// The rows
    pub rows: Vec<GraphRow>,
    /// Linear runs split by references.
    pub linear_segments: Vec<LinearSegment>,
    /// Per-reference rows; a shared commit appears in every reference that
    /// reaches it.
    pub reference_segments: Vec<ReferenceSegment>,
}

/// The Graph Workspace that has been decorated with a bunch of types
pub struct DetailedGraphWorkspace {
    /// The stacks
    pub stacks: Vec<Stack>,
}

/// A detailed graph workspace
pub fn detailed_graph_workspace(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
) -> Result<DetailedGraphWorkspace> {
    let graph = workspace.graph.clone().into_mut(repo)?;
    let ws = graph.graph_workspace()?;

    Ok(DetailedGraphWorkspace {
        stacks: ws
            .stacks
            .iter()
            .map(|stack| stack_rows(&graph, stack, &ws.reference_status, &ws.commit_state))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn stack_rows(
    graph: &MutableNodeGraph,
    stack: &Subgraph,
    reference_status: &HashMap<NodeIndex, ReferenceStatus>,
    commit_state: &HashMap<NodeIndex, CommitState>,
) -> Result<Stack> {
    let mut visible_nodes = HashSet::new();
    for selector in &stack.nodes {
        if is_visible_step(graph, *selector) {
            visible_nodes.insert(*selector);
        }
    }
    let parents_by_node = visible_nodes
        .iter()
        .copied()
        .map(|node| Ok((node, visible_parents(graph, &stack.nodes, node)?)))
        .collect::<Result<HashMap<_, _>>>()?;

    // Seed the traversal from the stack's visible tips (nodes no visible child
    // points at), ordered deterministically: commits before references, then by
    // id / refname. This keeps the render order stable without leaning on graph
    // internals or hash iteration order.
    let has_visible_child: HashSet<NodeIndex> =
        parents_by_node.values().flatten().copied().collect();
    let mut tips = vec![];
    for &node in &visible_nodes {
        if !has_visible_child.contains(&node) {
            tips.push((seed_key(graph, node), node));
        }
    }
    tips.sort_by(|(a, _), (b, _)| a.cmp(b));
    let seeds: Vec<NodeIndex> = tips.into_iter().map(|(_, node)| node).collect();

    let mut renderer = GraphRowRenderer::<NodeIndex>::new();
    let mut rows: Vec<(NodeIndex, GraphRow)> = vec![];
    for node in topological_order(&visible_nodes, &parents_by_node, &seeds) {
        let parents = parents_by_node
            .get(&node)
            .into_iter()
            .flatten()
            .copied()
            .map(Ancestor::Parent)
            .collect();
        let rendered = renderer.next_row(node, parents, String::new(), String::new());
        rows.push((
            node,
            GraphRow {
                data: row_data(graph, node, reference_status, commit_state)?,
                node_line: rendered.node_line,
                link_line: rendered.link_line,
                term_line: rendered.term_line,
                pad_lines: rendered.pad_lines,
            },
        ));
    }

    let row_idxs_by_selector = rows
        .iter()
        .enumerate()
        .map(|(idx, (selector, _))| (*selector, idx))
        .collect::<HashMap<_, _>>();
    let children_by_node = children_by_node(&parents_by_node);

    Ok(Stack {
        linear_segments: linear_segments(&rows, &parents_by_node, &children_by_node),
        reference_segments: reference_segments(&rows, &parents_by_node, &row_idxs_by_selector),
        rows: rows.into_iter().map(|(_, row)| row).collect(),
    })
}

fn is_visible_step(graph: &MutableNodeGraph, selector: NodeIndex) -> bool {
    if graph.pick_at(selector).is_some() {
        return true;
    }
    match graph.nodes()[selector].kind() {
        NodeKind::Reference(reference) => {
            reference.ref_info.ref_name.category() == Some(gix::refs::Category::LocalBranch)
        }
        // Tombstones and shallow boundaries are never visible.
        _ => false,
    }
}

fn visible_parents(
    graph: &MutableNodeGraph,
    stack_nodes: &HashSet<NodeIndex>,
    selector: NodeIndex,
) -> Result<Vec<NodeIndex>> {
    fn walk(
        graph: &MutableNodeGraph,
        stack_nodes: &HashSet<NodeIndex>,
        selector: NodeIndex,
        seen: &mut HashSet<NodeIndex>,
        out: &mut Vec<NodeIndex>,
    ) -> Result<()> {
        let mut parents = graph.direct_parents(selector)?;
        parents.sort_by_key(|(_, order)| *order);
        for (parent, _) in parents {
            if !stack_nodes.contains(&parent) || !seen.insert(parent) {
                continue;
            }
            if is_visible_step(graph, parent) {
                out.push(parent);
            } else {
                walk(graph, stack_nodes, parent, seen, out)?;
            }
        }
        Ok(())
    }

    let mut out = vec![];
    walk(graph, stack_nodes, selector, &mut HashSet::new(), &mut out)?;
    Ok(out)
}

/// Deterministic ordering key for seed tips: commits before references, then by
/// id / refname. Mirrors `but_graph::edit::testing`'s `compare_heads`.
fn seed_key(graph: &MutableNodeGraph, selector: NodeIndex) -> (u8, String) {
    if let Some(Pick { id, .. }) = graph.pick_at(selector) {
        return (0, id.to_string());
    }
    match graph.nodes()[selector].kind() {
        NodeKind::Reference(reference) => (1, reference.ref_info.ref_name.as_bstr().to_string()),
        // Tombstones and shallow boundaries sort last.
        _ => (2, String::new()),
    }
}

/// Children-first topological order over `nodes`, seeded from `seeds` (the
/// stack's visible tips, in deterministic order).
///
/// A node is emitted only once every child pointing at it (its incoming edges
/// within `nodes`) has been emitted, so shared parents land below all of their
/// children. Parents are followed in edge order, so the walk descends each
/// branch tip-to-base before moving to the next seed. Mirrors
/// `but_graph::edit::testing`'s `topological_order`.
fn topological_order(
    nodes: &HashSet<NodeIndex>,
    parents_by_node: &HashMap<NodeIndex, Vec<NodeIndex>>,
    seeds: &[NodeIndex],
) -> Vec<NodeIndex> {
    // `in_degree` counts the children still to be emitted before a node is ready.
    let mut in_degree: HashMap<NodeIndex, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    for parents in parents_by_node.values() {
        for parent in parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg += 1;
            }
        }
    }

    // Iterative DFS (recursion would blow the stack on long branches). Popping a
    // node runs the pre-visit the recursive form does on entry: skip while not
    // yet eligible, else emit, drop this node's contribution to each parent,
    // then push the parents so they're explored in edge order.
    let mut out = vec![];
    let mut visited = HashSet::new();
    let mut stack: Vec<NodeIndex> = seeds.iter().rev().copied().collect();
    while let Some(node) = stack.pop() {
        if visited.contains(&node) || in_degree.get(&node).is_some_and(|&d| d > 0) {
            continue;
        }
        visited.insert(node);
        out.push(node);

        let parents = parents_by_node.get(&node).map(Vec::as_slice).unwrap_or(&[]);
        for parent in parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg = deg.saturating_sub(1);
            }
        }
        for &parent in parents.iter().rev() {
            stack.push(parent);
        }
    }
    out
}

fn linear_segments(
    rows: &[(NodeIndex, GraphRow)],
    parents_by_node: &HashMap<NodeIndex, Vec<NodeIndex>>,
    children_by_node: &HashMap<NodeIndex, Vec<NodeIndex>>,
) -> Vec<LinearSegment> {
    let mut segments = vec![LinearSegment {
        reference_idx: None,
        row_idxs: vec![],
    }];
    for (idx, (selector, row)) in rows.iter().enumerate() {
        if matches!(row.data, GraphRowData::Reference { .. }) {
            segments.push(LinearSegment {
                reference_idx: Some(idx),
                row_idxs: vec![idx],
            });
            continue;
        }

        let is_fork_or_merge = parents_by_node
            .get(selector)
            .is_some_and(|parents| parents.len() > 1)
            || children_by_node
                .get(selector)
                .is_some_and(|children| children.len() > 1);
        if is_fork_or_merge
            && segments
                .last()
                .is_some_and(|segment| !segment.row_idxs.is_empty())
        {
            segments.push(LinearSegment {
                reference_idx: None,
                row_idxs: vec![],
            });
        }
        if let Some(segment) = segments.last_mut() {
            segment.row_idxs.push(idx);
        }
        if is_fork_or_merge {
            segments.push(LinearSegment {
                reference_idx: None,
                row_idxs: vec![],
            });
        }
    }
    segments
        .into_iter()
        .filter(|segment| segment.reference_idx.is_some() || !segment.row_idxs.is_empty())
        .collect()
}

fn children_by_node(
    parents_by_node: &HashMap<NodeIndex, Vec<NodeIndex>>,
) -> HashMap<NodeIndex, Vec<NodeIndex>> {
    let mut children_by_node: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for (child, parents) in parents_by_node {
        for parent in parents {
            children_by_node.entry(*parent).or_default().push(*child);
        }
    }
    children_by_node
}

fn reference_segments(
    rows: &[(NodeIndex, GraphRow)],
    parents_by_node: &HashMap<NodeIndex, Vec<NodeIndex>>,
    row_idxs_by_selector: &HashMap<NodeIndex, usize>,
) -> Vec<ReferenceSegment> {
    rows.iter()
        .enumerate()
        .filter(|(_, (_, row))| matches!(row.data, GraphRowData::Reference { .. }))
        .map(|(reference_idx, (reference, _))| {
            let mut segment_selectors = HashSet::from([*reference]);
            let mut tips = vec![*reference];
            let mut row_idxs = vec![reference_idx];
            while let Some(tip) = tips.pop() {
                for parent in parents_by_node.get(&tip).into_iter().flatten() {
                    let Some(parent_idx) = row_idxs_by_selector.get(parent).copied() else {
                        continue;
                    };
                    // Stop at references: each reference owns the commits down to
                    // the next one. A commit reachable from several references is
                    // therefore claimed by each of them.
                    if rows
                        .get(parent_idx)
                        .is_some_and(|(_, row)| matches!(row.data, GraphRowData::Reference { .. }))
                    {
                        continue;
                    }
                    if segment_selectors.insert(*parent) {
                        row_idxs.push(parent_idx);
                        tips.push(*parent);
                    }
                }
            }
            row_idxs.sort_unstable();
            ReferenceSegment {
                reference_idx,
                row_idxs,
            }
        })
        .collect()
}

fn row_data(
    graph: &MutableNodeGraph,
    selector: NodeIndex,
    reference_status: &HashMap<NodeIndex, ReferenceStatus>,
    commit_state: &HashMap<NodeIndex, CommitState>,
) -> Result<GraphRowData> {
    // `pick_at` also covers convergence boundaries: they are real, addressable
    // commits and render as commit rows.
    if let Some(Pick { id, .. }) = graph.pick_at(selector) {
        return Ok(GraphRowData::Commit {
            commit: but_core::Commit::from_id(id.attach(graph.repo()))?.into(),
            state: commit_state
                .get(&selector)
                .cloned()
                .unwrap_or(CommitState::LocalOnly),
        });
    }
    match graph.nodes()[selector].kind() {
        NodeKind::Reference(reference) => Ok(GraphRowData::Reference {
            ref_name: reference.ref_info.ref_name.clone(),
            additional_ref_info: reference_status.get(&selector).cloned(),
        }),
        _ => unreachable!("tombstones and boundaries are not visible rows"),
    }
}
