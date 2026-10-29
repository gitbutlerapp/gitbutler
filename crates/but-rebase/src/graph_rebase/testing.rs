#![deny(missing_docs)]
//! Testing utilities

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use anyhow::Result;
use but_core::RefMetadata;
use petgraph::{
    dot::{Config, Dot},
    visit::{EdgeRef, IntoEdgeReferences},
};
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

#[cfg(test)]
use crate::graph_rebase::Edge;
use crate::graph_rebase::{
    Editor, Pick, Selector, Step, StepGraph, StepGraphIndex, SuccessfulRebase, workspace::Subgraph,
};

/// An extension trait that adds debugging output for graphs
pub trait Testing {
    /// Creates an ASCII graph similar to `git log --graph --oneline` with commit titles
    fn steps_ascii(&self) -> String;
}

impl<M: RefMetadata> Testing for Editor<'_, '_, M> {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.graph, |id| lookup_commit_title(&self.repo, id))
    }
}

impl<M: RefMetadata> Testing for SuccessfulRebase<'_, '_, M> {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.graph, |id| lookup_commit_title(&self.repo, id))
    }
}
/// An extension trait that adds debugging output for graphs
pub trait TestingDot {
    /// Creates a dot graph with labels
    fn steps_dot(&self) -> String;
}

impl<M: RefMetadata> TestingDot for Editor<'_, '_, M> {
    fn steps_dot(&self) -> String {
        self.graph.steps_dot()
    }
}

impl<M: RefMetadata> TestingDot for SuccessfulRebase<'_, '_, M> {
    fn steps_dot(&self) -> String {
        self.graph.steps_dot()
    }
}

impl TestingDot for StepGraph {
    fn steps_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_attr_getters(
                &self,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|_, v| format!("label=\"order: {}\"", v.weight().order),
                &|_, (_, step)| {
                    match step {
                        Step::Pick(Pick { id, .. }) => format!("label=\"pick: {id}\""),
                        Step::Reference { refname, .. } => {
                            format!("label=\"reference: {}\"", refname.as_bstr())
                        }
                        Step::None => "label=\"none\"".into(),
                    }
                },
            )
        )
    }
}

/// Looks up the commit title (first line of message) for a given commit id
fn lookup_commit_title(repo: &gix::Repository, id: gix::ObjectId) -> Option<String> {
    let object = repo.find_object(id).ok()?;
    let commit = object.try_into_commit().ok()?;
    let message = commit.message().ok()?;
    Some(message.title.to_string().trim().to_string())
}

trait ToSymbol {
    fn to_symbol(&self) -> char;
}

impl ToSymbol for Step {
    fn to_symbol(&self) -> char {
        match self {
            Self::Pick(_) => '●',
            Self::Reference { .. } => '◎',
            Self::None => '◌',
        }
    }
}

/// Format a step for display, optionally with a commit title
fn format_step(step: &Step, title: Option<String>) -> String {
    match step {
        Step::Pick(Pick { id, .. }) => {
            let mut sha = id.to_string();
            sha.truncate(7);
            match title {
                Some(t) => format!("{sha} {t}"),
                None => sha,
            }
        }
        Step::Reference { refname, mutable } => {
            let name = refname.as_bstr().to_string();
            if *mutable {
                name
            } else {
                format!("{name} (immutable)")
            }
        }
        Step::None => "no-op".to_string(),
    }
}

/// Find head nodes (no incoming edges)
fn find_heads(graph: &StepGraph) -> Vec<StepGraphIndex> {
    let mut has_incoming: HashSet<StepGraphIndex> = HashSet::new();
    for edge in graph.edge_references() {
        has_incoming.insert(edge.target());
    }
    graph
        .node_indices()
        .filter(|idx| !has_incoming.contains(idx))
        .collect()
}

/// Get parents sorted by edge order
fn get_sorted_parents(graph: &StepGraph, node: StepGraphIndex) -> Vec<StepGraphIndex> {
    let mut parents: Vec<_> = graph
        .edges(node)
        .map(|e| (e.weight().order, e.target()))
        .collect();
    parents.sort_by_key(|(order, _)| *order);
    parents.into_iter().map(|(_, p)| p).collect()
}

/// A deterministic ordering for the head nodes so snapshots are stable: picks
/// before references, then by id / refname.
fn compare_heads(graph: &StepGraph, a: StepGraphIndex, b: StepGraphIndex) -> Ordering {
    match (&graph[a], &graph[b]) {
        (
            Step::Reference { refname, .. },
            Step::Reference {
                refname: refname_b, ..
            },
        ) => refname.cmp(refname_b),
        (Step::Pick(Pick { id, .. }), Step::Pick(Pick { id: id_b, .. })) => id.cmp(id_b),
        (Step::Reference { .. }, Step::Pick(_)) => Ordering::Greater,
        (Step::Pick(_), Step::Reference { .. }) => Ordering::Less,
        (Step::None, Step::None) => Ordering::Equal,
        (_, Step::None) => Ordering::Greater,
        (Step::None, _) => Ordering::Less,
    }
}

/// Children-first topological order over `nodes`, seeded from `heads`.
///
/// Only edges between nodes in `nodes` are followed, so this works for a full
/// graph (where `nodes` is every index) as well as a subgraph that doesn't
/// include its parents.
fn topological_order(
    graph: &StepGraph,
    nodes: &HashSet<StepGraphIndex>,
    heads: &[StepGraphIndex],
) -> Vec<StepGraphIndex> {
    // Incoming edges from *within* the node set.
    let mut in_degree: HashMap<StepGraphIndex, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    for &n in nodes {
        for parent in get_sorted_parents(graph, n) {
            if let Some(deg) = in_degree.get_mut(&parent) {
                *deg += 1;
            }
        }
    }

    let mut result = Vec::new();
    let mut visited: HashSet<StepGraphIndex> = HashSet::new();

    fn dfs(
        node: StepGraphIndex,
        graph: &StepGraph,
        nodes: &HashSet<StepGraphIndex>,
        visited: &mut HashSet<StepGraphIndex>,
        in_degree: &mut HashMap<StepGraphIndex, usize>,
        result: &mut Vec<StepGraphIndex>,
    ) {
        if visited.contains(&node) || in_degree.get(&node).is_some_and(|&d| d > 0) {
            return;
        }

        visited.insert(node);
        result.push(node);

        let parents: Vec<_> = get_sorted_parents(graph, node)
            .into_iter()
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
    graph: &StepGraph,
    nodes: &HashSet<StepGraphIndex>,
    heads: &[StepGraphIndex],
    mut get_title: F,
) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let mut heads = heads.to_vec();
    heads.sort_by(|a, b| compare_heads(graph, *a, *b));

    let mut renderer = GraphRowRenderer::<StepGraphIndex>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();

    let mut out = String::new();
    for node in topological_order(graph, nodes, &heads) {
        let step = &graph[node];
        let title = match step {
            Step::Pick(Pick { id, .. }) => get_title(*id),
            _ => None,
        };
        let parents = get_sorted_parents(graph, node)
            .into_iter()
            .filter(|p| nodes.contains(p))
            .map(Ancestor::Parent)
            .collect();
        out.push_str(&renderer.next_row(
            node,
            parents,
            step.to_symbol().to_string(),
            format_step(step, title),
        ));
    }
    out.trim_end().to_string()
}

/// Render the full step graph as a box-drawing DAG.
pub(crate) fn render_ascii_graph<F>(graph: &StepGraph, get_title: F) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let nodes: HashSet<StepGraphIndex> = graph.node_indices().collect();
    let heads = find_heads(graph);
    render_step_graph(graph, &nodes, &heads, get_title)
}

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Render a [`Subgraph`] (e.g. one of the parts of [`Editor::graph_workspace`])
    /// as a box-drawing DAG, in the same style as [`Testing::steps_ascii`].
    pub fn subgraph_ascii(&self, subgraph: &Subgraph) -> String {
        let resolve = |s: &Selector| self.history.normalize_selector(*s).ok().map(|s| s.id);
        let nodes: HashSet<StepGraphIndex> = subgraph.nodes.iter().filter_map(resolve).collect();
        let heads: Vec<StepGraphIndex> = subgraph.heads.iter().filter_map(resolve).collect();
        render_step_graph(&self.graph, &nodes, &heads, |id| {
            lookup_commit_title(&self.repo, id)
        })
    }

    /// Render an entire [`Editor::graph_workspace`] projection for snapshot
    /// tests: the commits above the workspace, the workspace commit, then each
    /// stack in turn. Each section is rendered with [`Editor::subgraph_ascii`].
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

        let workspace_commit = ws.workspace_commit.map(|selector| Subgraph {
            heads: vec![selector],
            nodes: [selector].into(),
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn make_pick(hex: &str) -> Step {
        Step::Pick(Pick::new_pick(gix::ObjectId::from_str(hex).unwrap()))
    }

    fn make_ref(name: &str) -> Step {
        Step::new_reference(gix::refs::FullName::try_from(format!("refs/heads/{name}")).unwrap())
    }

    /// Helper to build a graph and add edges with order
    fn add_edge(graph: &mut StepGraph, from: StepGraphIndex, to: StepGraphIndex, order: usize) {
        graph.add_edge(from, to, Edge { order });
    }

    #[test]
    fn linear_graph() {
        // Simple linear: A -> B -> C -> D
        let mut graph = StepGraph::new();
        let a = graph.add_node(make_ref("main"));
        let b = graph.add_node(make_pick("1111111111111111111111111111111111111111"));
        let c = graph.add_node(make_pick("2222222222222222222222222222222222222222"));
        let d = graph.add_node(make_pick("3333333333333333333333333333333333333333"));
        let none = graph.add_node(Step::None);

        add_edge(&mut graph, a, b, 0);
        add_edge(&mut graph, b, c, 0);
        add_edge(&mut graph, c, d, 0);
        add_edge(&mut graph, d, none, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●  1111111
●  2222222
●  3333333
◌  no-op
"#]]
        );
    }

    #[test]
    fn two_way_merge() {
        // Two-way merge:
        //   M
        //  / \
        // A   B
        //  \ /
        //   C
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));

        // M has two parents: A (first) and B (second)
        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        // Both A and B have C as parent
        add_edge(&mut graph, a, c, 0);
        add_edge(&mut graph, b, c, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎    refs/heads/main
├─╮
● │  aaaaaaa
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn three_way_merge() {
        // Three-way merge:
        //     M
        //   / | \
        //  A  B  C
        //   \ | /
        //     D
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));

        // M has three parents
        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);
        // All converge to D
        add_edge(&mut graph, a, d, 0);
        add_edge(&mut graph, b, d, 0);
        add_edge(&mut graph, c, d, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎      refs/heads/main
├─┬─╮
● │ │  aaaaaaa
│ ● │  bbbbbbb
├─╯ │
│   ●  ccccccc
├───╯
●  ddddddd
"#]]
        );
    }

    #[test]
    fn nested_merge_first_leg_forks_into_three() {
        // First leg of a 2-way merge forks into 3:
        //       M
        //      / \
        //     F   B
        //   / | \  \
        //  X  Y  Z  \
        //   \ | /   |
        //     C-----+
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff")); // fork point
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let x = graph.add_node(make_pick("1111111111111111111111111111111111111111"));
        let y = graph.add_node(make_pick("2222222222222222222222222222222222222222"));
        let z = graph.add_node(make_pick("3333333333333333333333333333333333333333"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));

        // M has two parents: F (first) and B (second)
        add_edge(&mut graph, m, f, 0);
        add_edge(&mut graph, m, b, 1);

        // F forks into X, Y, Z
        add_edge(&mut graph, f, x, 0);
        add_edge(&mut graph, f, y, 1);
        add_edge(&mut graph, f, z, 2);

        // X, Y, Z all converge to C
        add_edge(&mut graph, x, c, 0);
        add_edge(&mut graph, y, c, 0);
        add_edge(&mut graph, z, c, 0);

        // B also goes to C
        add_edge(&mut graph, b, c, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎    refs/heads/main
├─╮
● │      fffffff
├───┬─╮
● │ │ │  1111111
│ │ ● │  2222222
├───╯ │
│ │   ●  3333333
├─────╯
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn four_way_merge() {
        // Four-way merge
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));
        let base = graph.add_node(make_pick("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));

        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);
        add_edge(&mut graph, m, d, 3);

        add_edge(&mut graph, a, base, 0);
        add_edge(&mut graph, b, base, 0);
        add_edge(&mut graph, c, base, 0);
        add_edge(&mut graph, d, base, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎        refs/heads/main
├─┬─┬─╮
● │ │ │  aaaaaaa
│ ● │ │  bbbbbbb
├─╯ │ │
│   ● │  ccccccc
├───╯ │
│     ●  ddddddd
├─────╯
●  eeeeeee
"#]]
        );
    }

    #[test]
    fn asymmetric_merge_long_first_branch() {
        // Asymmetric merge where first branch is longer:
        //   M
        //  / \
        // A1  B
        // |   |
        // A2  |
        // |   |
        // A3  |
        //  \ /
        //   C
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a1 = graph.add_node(make_pick("a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"));
        let a2 = graph.add_node(make_pick("a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2"));
        let a3 = graph.add_node(make_pick("a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));

        add_edge(&mut graph, m, a1, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, a1, a2, 0);
        add_edge(&mut graph, a2, a3, 0);
        add_edge(&mut graph, a3, c, 0);
        add_edge(&mut graph, b, c, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎    refs/heads/main
├─╮
● │  a1a1a1a
● │  a2a2a2a
● │  a3a3a3a
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn consecutive_forks() {
        // A forks to B,C; B immediately forks to D,E; all merge to F
        //       A
        //      / \
        //     B   C
        //    / \   \
        //   D   E   |
        //    \ /    |
        //     F-----+
        let mut graph = StepGraph::new();
        let a = graph.add_node(make_ref("main"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));
        let e = graph.add_node(make_pick("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff"));

        // A forks to B, C
        add_edge(&mut graph, a, b, 0);
        add_edge(&mut graph, a, c, 1);

        // B forks to D, E
        add_edge(&mut graph, b, d, 0);
        add_edge(&mut graph, b, e, 1);

        // D, E, C all converge to F
        add_edge(&mut graph, d, f, 0);
        add_edge(&mut graph, e, f, 0);
        add_edge(&mut graph, c, f, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎    refs/heads/main
├─╮
● │    bbbbbbb
├───╮
● │ │  ddddddd
│ │ ●  eeeeeee
├───╯
│ ●  ccccccc
├─╯
●  fffffff
"#]]
        );
    }

    #[test]
    fn wide_merge_with_first_branch_forking_into_three() {
        // M 3-way merges F,B,C; the first branch F itself forks into X,Y,Z,
        // and everything converges back at D.
        //          M
        //        / | \
        //       F  B  C
        //      /|\  \ |
        //     X Y Z  \|
        //      \|/    |
        //       D-----+
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let x = graph.add_node(make_pick("1111111111111111111111111111111111111111"));
        let y = graph.add_node(make_pick("2222222222222222222222222222222222222222"));
        let z = graph.add_node(make_pick("3333333333333333333333333333333333333333"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));

        // M forks to F, B, C
        add_edge(&mut graph, m, f, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);

        // F forks to X, Y, Z
        add_edge(&mut graph, f, x, 0);
        add_edge(&mut graph, f, y, 1);
        add_edge(&mut graph, f, z, 2);

        // X, Y, Z, B, C all converge to D
        add_edge(&mut graph, x, d, 0);
        add_edge(&mut graph, y, d, 0);
        add_edge(&mut graph, z, d, 0);
        add_edge(&mut graph, b, d, 0);
        add_edge(&mut graph, c, d, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎      refs/heads/main
├─┬─╮
● │ │      fffffff
├─────┬─╮
● │ │ │ │  1111111
│ │ │ ● │  2222222
├─────╯ │
│ │ │   ●  3333333
├───────╯
│ ● │  bbbbbbb
├─╯ │
│   ●  ccccccc
├───╯
●  ddddddd
"#]]
        );
    }

    #[test]
    fn fork_target_shared_with_a_sibling_branch() {
        // A fork (D -> E, F) where one target (F) is also reached by a sibling
        // branch (C), so F has two children in different stacks.
        //
        //       M
        //      /|\
        //     A B C
        //     |   |
        //     D   |   <- D continues from A
        //    / \ /
        //   E   F     <- D forks to E and F, F is shared with C
        //    \ /
        //     base
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));
        let e = graph.add_node(make_pick("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff"));
        let base = graph.add_node(make_pick("0000000000000000000000000000000000000000"));

        // M forks to A, B, C
        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);

        // A -> D
        add_edge(&mut graph, a, d, 0);

        // C -> F (C's branch leads to F)
        add_edge(&mut graph, c, f, 0);

        // D forks to E and F
        add_edge(&mut graph, d, e, 0);
        add_edge(&mut graph, d, f, 1);

        // B -> base, E -> base, F -> base
        add_edge(&mut graph, b, base, 0);
        add_edge(&mut graph, e, base, 0);
        add_edge(&mut graph, f, base, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎      refs/heads/main
├─┬─╮
● │ │  aaaaaaa
● │ │    ddddddd
├─────╮
● │ │ │  eeeeeee
│ ● │ │  bbbbbbb
├─╯ │ │
│   ● │  ccccccc
│   ├─╯
│   ●  fffffff
├───╯
●  0000000
"#]]
        );
    }

    #[test]
    fn fork_with_multiple_branches_merging_to_same_point() {
        // Tests a diamond pattern where multiple branches merge to a single point.
        // D forks to E and G, where G is also the merge target for B and C.
        // Then E and G merge at F.
        //
        //        M
        //      / | \
        //     A  B  C
        //     |  |  |
        //     D  |  |   <- D is on A's branch
        //    / \ |  |
        //   E   \|  |   <- D forks to E and G
        //   |    \ /
        //   |     G     <- B, C, and D's second branch merge at G
        //    \   /
        //      F        <- E and G merge at F
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));
        let e = graph.add_node(make_pick("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let g = graph.add_node(make_pick("9999999999999999999999999999999999999999"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff"));

        // M forks to A, B, C
        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);

        // A -> D
        add_edge(&mut graph, a, d, 0);

        // D forks to E and G
        add_edge(&mut graph, d, e, 0);
        add_edge(&mut graph, d, g, 1);

        // B, C merge to G
        add_edge(&mut graph, b, g, 0);
        add_edge(&mut graph, c, g, 0);

        // E and G merge to F
        add_edge(&mut graph, e, f, 0);
        add_edge(&mut graph, g, f, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎      refs/heads/main
├─┬─╮
● │ │  aaaaaaa
● │ │    ddddddd
├─────╮
● │ │ │  eeeeeee
│ ● │ │  bbbbbbb
│ ├───╯
│ │ ●  ccccccc
│ ├─╯
│ ●  9999999
├─╯
●  fffffff
"#]]
        );
    }

    #[test]
    fn three_way_fork_with_a_shared_target() {
        // A 3-way fork (D -> E, F, shared) where one target (shared) is also
        // reached by a sibling branch (C).
        //
        //      M
        //     /|\
        //    A B C
        //    |   |
        //    D   |    <- D continues from A
        //   /|\ /
        //  E F shared <- D forks to E, F, shared where shared comes from C
        //   \|/
        //    base
        let mut graph = StepGraph::new();
        let m = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = graph.add_node(make_pick("cccccccccccccccccccccccccccccccccccccccc"));
        let d = graph.add_node(make_pick("dddddddddddddddddddddddddddddddddddddddd"));
        let e = graph.add_node(make_pick("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = graph.add_node(make_pick("ffffffffffffffffffffffffffffffffffffffff"));
        let shared = graph.add_node(make_pick("1111111111111111111111111111111111111111"));
        let base = graph.add_node(make_pick("0000000000000000000000000000000000000000"));

        // M forks to A, B, C
        add_edge(&mut graph, m, a, 0);
        add_edge(&mut graph, m, b, 1);
        add_edge(&mut graph, m, c, 2);

        // A -> D
        add_edge(&mut graph, a, d, 0);

        // C -> shared
        add_edge(&mut graph, c, shared, 0);

        // D forks to E, F, shared (shared is also reached via C)
        add_edge(&mut graph, d, e, 0);
        add_edge(&mut graph, d, f, 1);
        add_edge(&mut graph, d, shared, 2);

        // E, F, shared merge to base; B is left as a dangling tip.
        add_edge(&mut graph, e, base, 0);
        add_edge(&mut graph, f, base, 0);
        add_edge(&mut graph, shared, base, 0);

        let output = render_ascii_graph(&graph, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎      refs/heads/main
├─┬─╮
● │ │  aaaaaaa
● │ │      ddddddd
├─────┬─╮
● │ │ │ │  eeeeeee
│ │ │ ● │  fffffff
├─────╯ │
│ ● │   │  bbbbbbb
│   ●   │  ccccccc
│   ├───╯
│   ●  1111111
├───╯
●  0000000
"#]]
        );
    }

    #[test]
    fn subgraph_drops_parents_outside_the_node_set() {
        // main -> a -> b -> base, rendering only the subgraph {a, b}.
        // `main` (a child of `a`) and `base` (a parent of `b`) are outside the
        // set, so neither is drawn and `b` renders as a root.
        let mut graph = StepGraph::new();
        let main = graph.add_node(make_ref("main"));
        let a = graph.add_node(make_pick("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = graph.add_node(make_pick("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let base = graph.add_node(make_pick("0000000000000000000000000000000000000000"));

        add_edge(&mut graph, main, a, 0);
        add_edge(&mut graph, a, b, 0);
        add_edge(&mut graph, b, base, 0);

        let nodes: HashSet<StepGraphIndex> = [a, b].into_iter().collect();
        let output = render_step_graph(&graph, &nodes, &[a], |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
●  aaaaaaa
●  bbbbbbb
"#]]
        );
    }
}
