//! Display graphs to the frontend yey

use std::{
    collections::{HashSet, VecDeque},
    hash::{DefaultHasher, Hash as _, Hasher},
};

use anyhow::{Result, bail};
use but_graph::{
    Commit, FirstParent, Segment, SegmentIndex, SegmentMetadata,
    petgraph::{Direction, visit::EdgeRef},
};
use gitbutler_commit::commit_ext::CommitMessageBstr as _;
use renderdag::{Ancestor, GraphRowRenderer, Renderer};
use serde::Serialize;

struct Subgraph {
    heads: Vec<SegmentIndex>,
    nodes: HashSet<SegmentIndex>,
}

/// A row, either a commit or a reference
pub enum RowData<'a> {
    /// A commit
    Commit(&'a Commit),
    /// A reference
    Reference(&'a gix::refs::FullNameRef),
}

struct Row<'a> {
    id: u64,
    ancestors: Vec<Ancestor<u64>>,
    data: RowData<'a>,
}

/// Some SubSegment
#[derive(Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct GrStack {
    /// The rows!!!
    pub rows: Vec<GraphRow>,
    // /// Segments yey
    // pub segments: Vec<GrSegment>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GrStack);

/// Graph segment
#[derive(Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct GrSegment {
    /// The rows!!!
    pub rows: Vec<GraphRow>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GrSegment);

/// A row...from graph <shocked pikachu face>
#[derive(Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct GraphRow {
    /// The stuff the row contains
    pub data: String,

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
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GraphRow);

/// A column in the node row.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum NodeLine {
    /// Blank.
    Blank,

    /// Vertical line indicating an ancestor.
    Ancestor,

    /// Vertical line indicating a parent.
    Parent,

    /// The node for this row.
    Node,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NodeLine);

impl From<renderdag::NodeLine> for NodeLine {
    fn from(value: renderdag::NodeLine) -> Self {
        match value {
            renderdag::NodeLine::Blank => Self::Blank,
            renderdag::NodeLine::Ancestor => Self::Ancestor,
            renderdag::NodeLine::Parent => Self::Parent,
            renderdag::NodeLine::Node => Self::Node,
        }
    }
}

/// A column in a padding row.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum PadLine {
    /// Blank.
    Blank,

    /// Vertical line indicating an ancestor.
    Ancestor,

    /// Vertical line indicating a parent.
    Parent,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PadLine);

impl From<renderdag::PadLine> for PadLine {
    fn from(value: renderdag::PadLine) -> Self {
        match value {
            renderdag::PadLine::Blank => Self::Blank,
            renderdag::PadLine::Parent => Self::Parent,
            renderdag::PadLine::Ancestor => Self::Ancestor,
        }
    }
}

/// A column in a linking row.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct LinkLine(u16);

impl From<renderdag::LinkLine> for LinkLine {
    fn from(value: renderdag::LinkLine) -> Self {
        Self(value.bits())
    }
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(LinkLine);

/// The bestest workspace
pub fn render_workspace(
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
) -> Result<Vec<GrStack>> {
    let stacks = render_stacks(workspace, GraphRowRenderer::new, |row_data| {
        Ok(match &row_data {
            RowData::Commit(commit) => repo.find_commit(commit.id)?.message_bstr().to_string(),
            RowData::Reference(ref_name) => ref_name.to_string(),
        })
    })?;

    let mut out = vec![];

    for stack in stacks {
        out.push(GrStack {
            rows: stack
                .into_iter()
                .map(|row| GraphRow {
                    data: row.message,
                    node_line: row
                        .node_line
                        .iter()
                        .cloned()
                        .map(|n| n.into())
                        .collect::<Vec<_>>(),
                    link_line: row
                        .link_line
                        .map(|ll| ll.iter().cloned().map(|n| n.into()).collect::<Vec<_>>()),
                    term_line: row.term_line,
                    pad_lines: row
                        .pad_lines
                        .iter()
                        .cloned()
                        .map(|n| n.into())
                        .collect::<Vec<_>>(),
                })
                .collect(),
        })
    }

    Ok(out)
}

/// Render out stacks
pub fn render_stacks<R>(
    workspace: &but_graph::Workspace,
    build_renderer: impl Fn() -> R,
    format_message: impl Fn(&RowData) -> Result<String>,
) -> Result<Vec<Vec<R::Output>>>
where
    R: Renderer<u64>,
{
    let stacks = build_stack_rows(workspace)?;

    let mut renderings = vec![];

    for subgraph in stacks {
        let mut graph = build_renderer();

        let mut out = Vec::with_capacity(subgraph.len());

        for row in subgraph {
            out.push(graph.next_row(
                row.id,
                row.ancestors,
                if matches!(row.data, RowData::Commit(_)) {
                    "●".into()
                } else {
                    "◎".into()
                },
                format_message(&row.data)?,
            ));
        }

        renderings.push(out);
    }

    Ok(renderings)
}

/// Does nothing :D
fn build_stack_rows<'ws>(workspace: &'ws but_graph::Workspace) -> Result<Vec<Vec<Row<'ws>>>> {
    let graph = &workspace.graph;
    let Some(target_commit) = &workspace.target_commit else {
        bail!("Only bounded workspaces for experiment");
    };
    let entrypoint = graph.entrypoint()?;
    let heads = if matches!(
        entrypoint.segment.metadata,
        Some(SegmentMetadata::Workspace(_))
    ) {
        let parents = graph.edges_directed(entrypoint.segment.id, Direction::Outgoing);
        parents.map(|p| p.target()).collect::<Vec<_>>()
    } else {
        vec![entrypoint.segment.id]
    };

    let mut subgraphs: Vec<Subgraph> = vec![];
    'outer: for head in &heads {
        let segments = graph
            .find_segments_reachable_from_a_not_b(
                *head,
                target_commit.segment_index,
                FirstParent::No,
            )
            .collect::<Vec<_>>();

        for subgraph in &mut subgraphs {
            if segments.iter().any(|s| subgraph.nodes.contains(&s.id)) {
                subgraph.heads.push(*head);
                subgraph.nodes.extend(segments.iter().map(|s| s.id));
                continue 'outer;
            }
        }

        subgraphs.push(Subgraph {
            heads: vec![*head],
            nodes: segments.iter().map(|s| s.id).collect(),
        });
    }

    let mut stepped_subgraphs: Vec<Vec<Row<'ws>>> = vec![];

    for subgraph in subgraphs {
        stepped_subgraphs.push(subgraph_to_steps(graph, &subgraph)?);
    }

    Ok(stepped_subgraphs)
}

fn subgraph_to_steps<'g>(graph: &'g but_graph::Graph, subgraph: &Subgraph) -> Result<Vec<Row<'g>>> {
    let mut out = vec![];
    let mut tips = subgraph.heads.iter().cloned().collect::<VecDeque<_>>();
    let mut seen = subgraph.heads.iter().cloned().collect::<HashSet<_>>();

    while let Some(tip) = tips.pop_front() {
        let s = &graph[tip];
        if let Some(ref_info) = &s.ref_info {
            let refname = &ref_info.ref_name;
            let ancestors = if let Some(c) = s.commits.first() {
                vec![Ancestor::Parent(hash_oid(c.id))]
            } else {
                graph
                    .edges_directed(tip, Direction::Outgoing)
                    .map(|e| {
                        Ok(if subgraph.nodes.contains(&e.target()) {
                            Ancestor::Parent(hash_segment(&graph[e.target()])?)
                        } else {
                            Ancestor::Anonymous
                        })
                    })
                    .collect::<Result<_>>()?
            };
            out.push(Row {
                id: hash_reference(refname),
                ancestors,
                data: RowData::Reference(refname.as_ref()),
            })
        }

        for (idx, commit) in s.commits.iter().enumerate() {
            let ancestors = if idx == s.commits.len() - 1 {
                graph
                    .edges_directed(tip, Direction::Outgoing)
                    .map(|e| {
                        Ok(if subgraph.nodes.contains(&e.target()) {
                            Ancestor::Parent(hash_segment(&graph[e.target()])?)
                        } else {
                            Ancestor::Anonymous
                        })
                    })
                    .collect::<Result<_>>()?
            } else {
                vec![Ancestor::Parent(hash_oid(
                    s.commits
                        .get(idx + 1)
                        .expect("BUG: This is the second to last commit in the array")
                        .id,
                ))]
            };

            out.push(Row {
                id: hash_oid(commit.id),
                ancestors,
                data: RowData::Commit(commit),
            });
        }

        for parent in graph.edges_directed(tip, Direction::Outgoing) {
            if !subgraph.nodes.contains(&parent.target()) {
                continue;
            }
            if seen.insert(parent.target()) {
                tips.push_back(parent.target());
            }
        }
    }
    Ok(out)
}

fn hash_segment(s: &Segment) -> Result<u64> {
    if let Some(ref_info) = &s.ref_info {
        return Ok(hash_reference(&ref_info.ref_name));
    }
    if let Some(c) = &s.commits.first() {
        return Ok(hash_oid(c.id));
    }

    bail!("Tried to make a hash for an empty segment")
}

fn hash_oid(a: gix::ObjectId) -> u64 {
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    hasher.finish()
}

fn hash_reference(a: &gix::refs::FullName) -> u64 {
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod test {
    use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

    #[test]
    fn foobar() {
        // Imagining a graph a -> b -> c; a -> x -> c;
        let mut graph = GraphRowRenderer::new().output().build_box_drawing();
        let out = vec![
            graph.next_row(
                "A",
                vec![Ancestor::Parent("B"), Ancestor::Parent("X")],
                "*".into(),
                "This is A".into(),
            ),
            graph.next_row(
                "B",
                vec![Ancestor::Parent("C")],
                "*".into(),
                "This is B".into(),
            ),
            graph.next_row(
                "X",
                vec![Ancestor::Parent("C")],
                "*".into(),
                "This is X".into(),
            ),
            graph.next_row(
                "C",
                vec![Ancestor::Anonymous],
                "*".into(),
                "This is C".into(),
            ),
        ];
        for line in out {
            print!("{line}");
        }
        println!();
    }
}
