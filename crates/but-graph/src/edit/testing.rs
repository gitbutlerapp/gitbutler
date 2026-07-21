#![deny(missing_docs)]
//! Testing utilities

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use anyhow::Result;
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

use crate::{
    BoundaryKind, Node, NodeIndex, NodeKind,
    edit::{MutableNodeGraph, NodePolicy, Rebased},
    workspace::Subgraph,
};

/// An extension trait that adds debugging output for graphs
pub trait Testing {
    /// Creates an ASCII graph similar to `git log --graph --oneline` with commit titles
    fn steps_ascii(&self) -> String;
}

impl Testing for MutableNodeGraph {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(self.nodes(), &self.policy, |id| {
            lookup_commit_title(self.repo(), id)
        })
    }
}

impl Testing for Rebased {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(self.graph.nodes(), &self.policy, |id| {
            lookup_commit_title(self.repo(), id)
        })
    }
}

/// An extension trait that adds debugging output for graphs
pub trait TestingDot {
    /// Creates a dot graph with labels
    fn steps_dot(&self) -> String;
}

impl TestingDot for MutableNodeGraph {
    fn steps_dot(&self) -> String {
        steps_dot(self.nodes(), &self.policy)
    }
}

impl TestingDot for Rebased {
    fn steps_dot(&self) -> String {
        steps_dot(self.graph.nodes(), &self.policy)
    }
}

/// A per-node view synthesized from [`NodeKind`] + [`NodePolicy`] purely for
/// rendering, mirroring how the rebase reads nodes: a convergence boundary is
/// an (immutable) pick, a shallow boundary behaves like a removed node.
enum RenderStep {
    Pick {
        id: gix::ObjectId,
    },
    Reference {
        refname: gix::refs::FullName,
        mutable: bool,
    },
    None,
}

fn render_step(nodes: &[Node], policy: &[NodePolicy], index: NodeIndex) -> RenderStep {
    match nodes[index].kind() {
        NodeKind::Commit { id }
        | NodeKind::Boundary {
            id,
            reason: BoundaryKind::Convergence,
        } => RenderStep::Pick { id: *id },
        NodeKind::Reference(reference) => RenderStep::Reference {
            refname: reference.ref_info.ref_name.clone(),
            mutable: matches!(policy[index], NodePolicy::Reference { mutable: true }),
        },
        NodeKind::Boundary {
            reason: BoundaryKind::Shallow,
            ..
        }
        | NodeKind::None => RenderStep::None,
    }
}

fn steps_dot(nodes: &[Node], policy: &[NodePolicy]) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("digraph {\n");
    for index in 0..nodes.len() {
        let label = match render_step(nodes, policy, index) {
            RenderStep::Pick { id } => format!("pick: {id}"),
            RenderStep::Reference { refname, .. } => {
                format!("reference: {}", refname.as_bstr())
            }
            RenderStep::None => "none".into(),
        };
        writeln!(out, "    {index} [ label=\"{label}\" ]").expect("infallible");
    }
    for (index, node) in nodes.iter().enumerate() {
        for (order, parent) in node.parents().iter().enumerate() {
            writeln!(out, "    {index} -> {parent} [ label=\"order: {order}\" ]")
                .expect("infallible");
        }
    }
    out.push('}');
    out
}

/// Looks up the commit title (first line of message) for a given commit id
fn lookup_commit_title(repo: &gix::Repository, id: gix::ObjectId) -> Option<String> {
    let object = repo.find_object(id).ok()?;
    let commit = object.try_into_commit().ok()?;
    let message = commit.message().ok()?;
    Some(message.title.to_string().trim().to_string())
}

impl RenderStep {
    fn to_symbol(&self) -> char {
        match self {
            Self::Pick { .. } => '●',
            Self::Reference { .. } => '◎',
            Self::None => '◌',
        }
    }
}

/// Format a step for display, optionally with a commit title
fn format_step(step: &RenderStep, title: Option<String>) -> String {
    match step {
        RenderStep::Pick { id } => {
            let mut sha = id.to_string();
            sha.truncate(7);
            match title {
                Some(t) => format!("{sha} {t}"),
                None => sha,
            }
        }
        RenderStep::Reference { refname, mutable } => {
            let name = refname.as_bstr().to_string();
            if *mutable {
                name
            } else {
                format!("{name} (immutable)")
            }
        }
        RenderStep::None => "no-op".to_string(),
    }
}

/// A deterministic ordering for the head nodes so snapshots are stable: picks
/// before references, then by id / refname.
fn compare_heads(nodes: &[Node], policy: &[NodePolicy], a: NodeIndex, b: NodeIndex) -> Ordering {
    match (
        &render_step(nodes, policy, a),
        &render_step(nodes, policy, b),
    ) {
        (
            RenderStep::Reference { refname, .. },
            RenderStep::Reference {
                refname: refname_b, ..
            },
        ) => refname.cmp(refname_b),
        (RenderStep::Pick { id }, RenderStep::Pick { id: id_b }) => id.cmp(id_b),
        (RenderStep::Reference { .. }, RenderStep::Pick { .. }) => Ordering::Greater,
        (RenderStep::Pick { .. }, RenderStep::Reference { .. }) => Ordering::Less,
        (RenderStep::None, RenderStep::None) => Ordering::Equal,
        (_, RenderStep::None) => Ordering::Greater,
        (RenderStep::None, _) => Ordering::Less,
    }
}

/// Children-first topological order over `nodes`, seeded from `heads`.
///
/// Only edges between nodes in `nodes` are followed, so this works for a full
/// graph (where `nodes` is every index) as well as a subgraph that doesn't
/// include its parents.
fn topological_order(
    graph: &[Node],
    nodes: &HashSet<NodeIndex>,
    heads: &[NodeIndex],
) -> Vec<NodeIndex> {
    // Incoming edges from *within* the node set.
    let mut in_degree: HashMap<NodeIndex, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    for &n in nodes {
        for parent in graph[n].parents() {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg += 1;
            }
        }
    }

    let mut result = Vec::new();
    let mut visited: HashSet<NodeIndex> = HashSet::new();

    fn dfs(
        node: NodeIndex,
        graph: &[Node],
        nodes: &HashSet<NodeIndex>,
        visited: &mut HashSet<NodeIndex>,
        in_degree: &mut HashMap<NodeIndex, usize>,
        result: &mut Vec<NodeIndex>,
    ) {
        if visited.contains(&node) || in_degree.get(&node).is_some_and(|&d| d > 0) {
            return;
        }

        visited.insert(node);
        result.push(node);

        let parents: Vec<_> = graph[node]
            .parents()
            .iter()
            .copied()
            .filter(|p| nodes.contains(p))
            .collect();
        for parent in &parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg = deg.saturating_sub(1);
            }
        }
        for parent in parents {
            dfs(parent, graph, nodes, visited, in_degree, result);
        }
    }

    for &head in heads {
        dfs(
            head,
            graph,
            nodes,
            &mut visited,
            &mut in_degree,
            &mut result,
        );
    }

    result
}

/// Render a (sub)graph of steps as a box-drawing DAG (à la `git log --graph`)
/// using `sapling-renderdag`.
///
/// `nodes` is the set of steps to draw and `heads` are the tips to seed the
/// ordering from; parents outside `nodes` are simply dropped, so this renders
/// both full graphs and subgraphs.
fn render_step_graph<F>(
    graph: &[Node],
    policy: &[NodePolicy],
    nodes: &HashSet<NodeIndex>,
    heads: &[NodeIndex],
    mut get_title: F,
) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let mut heads = heads.to_vec();
    heads.sort_by(|a, b| compare_heads(graph, policy, *a, *b));

    let mut renderer = GraphRowRenderer::<NodeIndex>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();

    let mut out = String::new();
    for node in topological_order(graph, nodes, &heads) {
        let step = render_step(graph, policy, node);
        let title = match &step {
            RenderStep::Pick { id } => get_title(*id),
            _ => None,
        };
        let parents = graph[node]
            .parents()
            .iter()
            .copied()
            .filter(|p| nodes.contains(p))
            .map(Ancestor::Parent)
            .collect();
        out.push_str(&renderer.next_row(
            node,
            parents,
            step.to_symbol().to_string(),
            format_step(&step, title),
        ));
    }
    out.trim_end().to_string()
}

/// Render the full graph as a box-drawing DAG.
pub(crate) fn render_ascii_graph<F>(graph: &[Node], policy: &[NodePolicy], get_title: F) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let nodes: HashSet<NodeIndex> = (0..graph.len())
        .filter(|index| {
            !matches!(
                graph[*index].kind(),
                crate::NodeKind::Boundary {
                    reason: crate::BoundaryKind::Shallow,
                    ..
                }
            )
        })
        .collect();
    let heads = crate::node::child_most(graph)
        .into_iter()
        .filter(|head| nodes.contains(head))
        .collect::<Vec<_>>();
    render_step_graph(graph, policy, &nodes, &heads, get_title)
}

impl MutableNodeGraph {
    /// Render a [`Subgraph`] (e.g. one of the parts of
    /// [`MutableNodeGraph::graph_workspace`]) as a box-drawing DAG, in the same
    /// style as [`Testing::steps_ascii`].
    pub fn subgraph_ascii(&self, subgraph: &Subgraph) -> String {
        let nodes: HashSet<NodeIndex> = subgraph.nodes.iter().copied().collect();
        let heads: Vec<NodeIndex> = subgraph.heads.clone();
        render_step_graph(self.nodes(), &self.policy, &nodes, &heads, |id| {
            lookup_commit_title(self.repo(), id)
        })
    }

    /// Render an entire [`MutableNodeGraph::graph_workspace`] projection for
    /// snapshot tests: the commits above the workspace, the workspace commit,
    /// then each stack in turn. Each section is rendered with
    /// [`MutableNodeGraph::subgraph_ascii`].
    pub fn graph_workspace_ascii(&self) -> Result<String> {
        let ws = self.graph_workspace()?;
        let body = |rendered: String| {
            if rendered.is_empty() {
                "(empty)".to_string()
            } else {
                rendered
            }
        };

        let mut sections = vec![format!(
            "# Above workspace\n{}",
            body(self.subgraph_ascii(&ws.above_workspace))
        )];

        let workspace_commit = ws.workspace_commit.map(|index| Subgraph {
            heads: vec![index],
            nodes: [index].into(),
        });
        sections.push(format!(
            "# Workspace commit\n{}",
            body(
                workspace_commit
                    .map(|s| self.subgraph_ascii(&s))
                    .unwrap_or_default()
            )
        ));

        for (i, stack) in ws.stacks.iter().enumerate() {
            sections.push(format!("# Stack {i}\n{}", body(self.subgraph_ascii(stack))));
        }

        Ok(sections.join("\n\n"))
    }
}
