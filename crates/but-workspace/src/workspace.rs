//! New graphy workspace

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, LookupStep, Pick, Selector, Step, workspace::ReferenceStatus,
};
use gix::prelude::ObjectIdExt;
use renderdag::{Ancestor, GraphRowRenderer, LinkLine, NodeLine, PadLine, Renderer};

use crate::{ref_info::Commit, ui::CommitState};

/// A graph row's data
#[expect(clippy::large_enum_variant)]
pub enum GraphRowData {
    /// A commit :D
    Commit {
        /// The commit.
        commit: Commit,
        /// The commit's state (local-only / local-and-remote / integrated), as
        /// computed by the Editor's workspace projection.
        state: CommitState,
    },
    /// A reference
    Reference {
        /// The name of the reference
        ref_name: gix::refs::FullName,
        /// More information about the reference, computed by the Editor's
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
pub fn detailed_graph_workspace<M: RefMetadata>(
    workspace: &mut but_graph::Workspace,
    meta: &mut M,
    repo: &gix::Repository,
) -> Result<DetailedGraphWorkspace> {
    let editor = Editor::create(workspace, meta, repo)?;
    let ws = editor.graph_workspace()?;

    Ok(DetailedGraphWorkspace {
        stacks: ws
            .stacks
            .iter()
            .map(|stack| stack_rows(&editor, stack, &ws.reference_status, &ws.commit_state))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn stack_rows<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack: &but_rebase::graph_rebase::Subgraph,
    reference_status: &HashMap<Selector, ReferenceStatus>,
    commit_state: &HashMap<Selector, CommitState>,
) -> Result<Stack> {
    let mut visible_nodes = HashSet::new();
    for selector in &stack.nodes {
        if is_visible_step(editor, *selector)? {
            visible_nodes.insert(*selector);
        }
    }
    let parents_by_node = visible_nodes
        .iter()
        .copied()
        .map(|node| Ok((node, visible_parents(editor, &stack.nodes, node)?)))
        .collect::<Result<HashMap<_, _>>>()?;

    // Seed the traversal from the stack's visible tips (nodes no visible child
    // points at), ordered deterministically: commits before references, then by
    // id / refname. This keeps the render order stable without leaning on graph
    // internals or hash iteration order.
    let has_visible_child: HashSet<Selector> =
        parents_by_node.values().flatten().copied().collect();
    let mut tips = vec![];
    for &node in &visible_nodes {
        if !has_visible_child.contains(&node) {
            tips.push((seed_key(editor, node)?, node));
        }
    }
    tips.sort_by(|(a, _), (b, _)| a.cmp(b));
    let seeds: Vec<Selector> = tips.into_iter().map(|(_, node)| node).collect();

    let mut renderer = GraphRowRenderer::<Selector>::new();
    let mut rows: Vec<(Selector, GraphRow)> = vec![];
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
                data: row_data(editor, node, reference_status, commit_state)?,
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

fn is_visible_step<M: RefMetadata>(editor: &Editor<'_, '_, M>, selector: Selector) -> Result<bool> {
    Ok(match editor.lookup_step(selector)? {
        Step::Pick(_) => true,
        Step::Reference { refname, .. } => {
            refname.category() == Some(gix::refs::Category::LocalBranch)
        }
        Step::None => false,
    })
}

fn visible_parents<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack_nodes: &HashSet<Selector>,
    selector: Selector,
) -> Result<Vec<Selector>> {
    fn walk<M: RefMetadata>(
        editor: &Editor<'_, '_, M>,
        stack_nodes: &HashSet<Selector>,
        selector: Selector,
        seen: &mut HashSet<Selector>,
        out: &mut Vec<Selector>,
    ) -> Result<()> {
        let mut parents = editor.direct_parents(selector)?;
        parents.sort_by_key(|(_, order)| *order);
        for (parent, _) in parents {
            if !stack_nodes.contains(&parent) || !seen.insert(parent) {
                continue;
            }
            if is_visible_step(editor, parent)? {
                out.push(parent);
            } else {
                walk(editor, stack_nodes, parent, seen, out)?;
            }
        }
        Ok(())
    }

    let mut out = vec![];
    walk(editor, stack_nodes, selector, &mut HashSet::new(), &mut out)?;
    Ok(out)
}

/// Deterministic ordering key for seed tips: commits before references, then by
/// id / refname. Mirrors `graph_rebase::testing::compare_heads`.
fn seed_key<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> Result<(u8, String)> {
    Ok(match editor.lookup_step(selector)? {
        Step::Pick(Pick { id, .. }) => (0, id.to_string()),
        Step::Reference { refname, .. } => (1, refname.as_bstr().to_string()),
        Step::None => (2, String::new()),
    })
}

/// Children-first topological order over `nodes`, seeded from `seeds` (the
/// stack's visible tips, in deterministic order).
///
/// A node is emitted only once every child pointing at it (its incoming edges
/// within `nodes`) has been emitted, so shared parents land below all of their
/// children. Parents are followed in edge order, so the walk descends each
/// branch tip-to-base before moving to the next seed. Mirrors
/// `graph_rebase::testing::topological_order`.
fn topological_order(
    nodes: &HashSet<Selector>,
    parents_by_node: &HashMap<Selector, Vec<Selector>>,
    seeds: &[Selector],
) -> Vec<Selector> {
    // `in_degree` counts the children still to be emitted before a node is ready.
    let mut in_degree: HashMap<Selector, usize> = nodes.iter().map(|&n| (n, 0)).collect();
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
    let mut stack: Vec<Selector> = seeds.iter().rev().copied().collect();
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
    rows: &[(Selector, GraphRow)],
    parents_by_node: &HashMap<Selector, Vec<Selector>>,
    children_by_node: &HashMap<Selector, Vec<Selector>>,
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
    parents_by_node: &HashMap<Selector, Vec<Selector>>,
) -> HashMap<Selector, Vec<Selector>> {
    let mut children_by_node: HashMap<Selector, Vec<Selector>> = HashMap::new();
    for (child, parents) in parents_by_node {
        for parent in parents {
            children_by_node.entry(*parent).or_default().push(*child);
        }
    }
    children_by_node
}

fn reference_segments(
    rows: &[(Selector, GraphRow)],
    parents_by_node: &HashMap<Selector, Vec<Selector>>,
    row_idxs_by_selector: &HashMap<Selector, usize>,
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

fn row_data<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
    reference_status: &HashMap<Selector, ReferenceStatus>,
    commit_state: &HashMap<Selector, CommitState>,
) -> Result<GraphRowData> {
    Ok(match editor.lookup_step(selector)? {
        Step::Pick(Pick { id, .. }) => GraphRowData::Commit {
            commit: but_core::Commit::from_id(id.attach(editor.repo()))?.into(),
            state: commit_state
                .get(&selector)
                .cloned()
                .unwrap_or(CommitState::LocalOnly),
        },
        Step::Reference { refname, .. } => GraphRowData::Reference {
            ref_name: refname,
            additional_ref_info: reference_status.get(&selector).cloned(),
        },
        Step::None => unreachable!("None steps are not visible rows"),
    })
}
