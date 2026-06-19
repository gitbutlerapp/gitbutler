//! New graphy workspace

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, ExtraRef, GraphEditorOptions, LookupStep, Pick, Selector, Step,
};
use gix::prelude::ObjectIdExt;
use renderdag::{Ancestor, GraphRowRenderer, LinkLine, NodeLine, PadLine, Renderer};

use crate::{ref_info::Commit, ui::PushStatus};

/// More information about a reference
pub struct AdditionalRefInfo {
    /// Does it have a remote? If so, who dis?
    pub remote_ref: Option<gix::refs::FullName>,
    /// Push status for just this reference.
    pub push_status: PushStatus,
    /// Push status for this reference combined with whether any parents will
    /// also result in a force push.
    pub combined_push_status: PushStatus,
}

/// A graph row's data
#[expect(clippy::large_enum_variant)]
pub enum GraphRowData {
    /// A commit :D
    Commit(Commit),
    /// A reference
    Reference {
        /// The name of the reference
        ref_name: gix::refs::FullName,
        /// More information about the reference
        additional_ref_info: Option<AdditionalRefInfo>,
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
    let target_ref = workspace.graph.project_meta.target_ref.clone();
    let opts = GraphEditorOptions {
        extra_refs: target_ref
            .iter()
            .map(|r| ExtraRef::immutable(r.as_ref()))
            .collect(),
        ..Default::default()
    };
    let editor = Editor::create_with_opts(workspace, meta, repo, &opts)?;
    let ws = editor.graph_workspace()?;

    Ok(DetailedGraphWorkspace {
        stacks: ws
            .stacks
            .iter()
            .map(|stack| stack_rows(&editor, stack))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn stack_rows<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack: &but_rebase::graph_rebase::Subgraph,
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

    let mut renderer = GraphRowRenderer::<Selector>::new();
    let mut rows: Vec<(Selector, GraphRow)> = vec![];
    for node in topological_order(&visible_nodes, &parents_by_node) {
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
                data: row_data(editor, node)?,
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
        Step::Reference { refname } => refname.category() == Some(gix::refs::Category::LocalBranch),
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

fn topological_order(
    nodes: &HashSet<Selector>,
    parents_by_node: &HashMap<Selector, Vec<Selector>>,
) -> Vec<Selector> {
    let mut in_degree: HashMap<Selector, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    for parents in parents_by_node.values() {
        for parent in parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg += 1;
            }
        }
    }

    // Min-priority frontier keyed by a stable string for deterministic render
    // order. Debug strings are unique per node within a stack.
    let key = |selector: &Selector| format!("{selector:?}");
    let mut frontier: BTreeMap<String, Selector> = nodes
        .iter()
        .copied()
        .filter(|n| in_degree.get(n) == Some(&0))
        .map(|n| (key(&n), n))
        .collect();

    let mut out = vec![];
    while let Some((_, node)) = frontier.pop_first() {
        out.push(node);
        for parent in parents_by_node.get(&node).into_iter().flatten() {
            let Some(deg) = in_degree.get_mut(parent) else {
                continue;
            };
            *deg = deg.saturating_sub(1);
            if *deg == 0 {
                frontier.insert(key(parent), *parent);
            }
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
) -> Result<GraphRowData> {
    Ok(match editor.lookup_step(selector)? {
        Step::Pick(Pick { id, .. }) => {
            GraphRowData::Commit(but_core::Commit::from_id(id.attach(editor.repo()))?.into())
        }
        Step::Reference { refname } => GraphRowData::Reference {
            ref_name: refname,
            additional_ref_info: None,
        },
        Step::None => unreachable!("None steps are not visible rows"),
    })
}

// Frontend types:
// pub enum GraphRowData {
//     Commit(Commit),
//     Reference {
//         ref_name: gix::refs::FullName,
//         remote_ref: Option<gix::refs::FullName>,
//         // TODO: Have a partial stack state
//         push_status: PushStatus,
//         pr_number: Option<usize>
//     },
// }

// type GraphRow = {
//   data: ReferenceOrCommit,
//   // line drawing instructions...jVj
// }

// type Graph = {
//   stacks: {
//     rows: GraphRow[],
//     upstreamStuffs: {
//       rows: GrpahRow[], // <- It's own commits
//       placeBelow: number
//     }[],
//     linearSections: { referenceIdx: number | undefined, rowIdxs: number[] }[],
//     referenceSections: { referenceIdx: number, rowIdxs: number[] }[],
//   }[],
