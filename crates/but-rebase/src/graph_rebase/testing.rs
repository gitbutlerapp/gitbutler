#![deny(missing_docs)]
//! Testing utilities

use std::collections::HashSet;

use petgraph::{
    dot::{Config, Dot},
    visit::{EdgeRef, IntoEdgeReferences},
};

#[cfg(test)]
use crate::graph_rebase::Edge;
use crate::graph_rebase::{Editor, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase};

/// An extension trait that adds debugging output for graphs
pub trait Testing {
    /// Creates an ASCII graph similar to `git log --graph --oneline` with commit titles
    fn steps_ascii(&self) -> String;
}

impl Testing for Editor {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.graph, |id| lookup_commit_title(&self.repo, id))
    }
}

impl Testing for SuccessfulRebase {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.graph, |id| lookup_commit_title(&self.repo, id))
    }
}
/// An extension trait that adds debugging output for graphs
pub trait TestingDot {
    /// Creates a dot graph with labels
    fn steps_dot(&self) -> String;
}

impl TestingDot for Editor {
    fn steps_dot(&self) -> String {
        self.graph.steps_dot()
    }
}

impl TestingDot for SuccessfulRebase {
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
                        Step::Reference { refname } => {
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

mod chars {
    pub const STEP_PICK: char = '●';
    pub const STEP_REFERENCE: char = '◎';
    pub const STEP_NONE: char = '◌';
    pub const VERT: char = '│';
    pub const HORIZ: char = '─';
    pub const FORK_DOWN: char = '┬';
    pub const MERGE_UP: char = '┴';
    pub const CORNER_DR: char = '╮';
    pub const CORNER_UR: char = '╯';
    pub const CROSS: char = '╪'; // double horizontal crossing vertical - shows passover
    pub const VERT_RIGHT: char = '├';
    pub const TERM_UP: char = '╵'; // branch termination going up
}

trait ToSymbol {
    fn to_symbol(&self) -> char;
}

impl ToSymbol for Step {
    fn to_symbol(&self) -> char {
        match self {
            Self::Pick(_) => chars::STEP_PICK,
            Self::Reference { .. } => chars::STEP_REFERENCE,
            Self::None => chars::STEP_NONE,
        }
    }
}

/// A layout event to be rendered
#[derive(Debug, Clone)]
enum LayoutEvent {
    /// A node at a specific column
    Node {
        col: usize,
        node: StepGraphIndex,
        /// Which columns have active rails (for drawing vertical lines)
        active: Vec<bool>,
    },
    /// Branches forking from a node to multiple parents
    Fork {
        from_col: usize,
        to_cols: Vec<usize>,
        active: Vec<bool>,
    },
    /// Multiple branches merging into one
    Merge { from_cols: Vec<usize>, active: Vec<bool> },
    /// A branch terminates (root node - no parents)
    Terminate { col: usize, active: Vec<bool> },
}

/// Rail-based layout state
struct LayoutState {
    /// What node each rail is waiting for (None = empty rail)
    rails: Vec<Option<StepGraphIndex>>,
}

impl LayoutState {
    fn new() -> Self {
        Self { rails: Vec::new() }
    }

    /// Get a snapshot of which rails are active
    fn active_snapshot(&self) -> Vec<bool> {
        self.rails.iter().map(|r| r.is_some()).collect()
    }

    /// Find all rails waiting for a specific node
    fn rails_waiting_for(&self, node: StepGraphIndex) -> Vec<usize> {
        self.rails
            .iter()
            .enumerate()
            .filter_map(|(i, r)| if *r == Some(node) { Some(i) } else { None })
            .collect()
    }

    /// Find first empty rail, or create a new one
    fn find_or_create_empty_rail(&mut self) -> usize {
        self.rails.iter().position(|r| r.is_none()).unwrap_or_else(|| {
            self.rails.push(None);
            self.rails.len() - 1
        })
    }

    /// Find first empty rail at or after `start`, or create one
    fn find_or_create_empty_rail_from(&mut self, start: usize) -> usize {
        for i in start..self.rails.len() {
            if self.rails[i].is_none() {
                return i;
            }
        }
        self.rails.push(None);
        self.rails.len() - 1
    }

    /// Set a rail to wait for a node
    fn set_rail(&mut self, col: usize, node: Option<StepGraphIndex>) {
        while self.rails.len() <= col {
            self.rails.push(None);
        }
        self.rails[col] = node;
    }

    /// Clear a rail
    fn clear_rail(&mut self, col: usize) {
        if col < self.rails.len() {
            self.rails[col] = None;
        }
    }

    /// Find if any rail is already waiting for this node
    fn find_rail_for(&self, node: StepGraphIndex) -> Option<usize> {
        self.rails.iter().position(|r| *r == Some(node))
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
        Step::Reference { refname } => refname.as_bstr().to_string(),
        Step::None => "no-op".to_string(),
    }
}

/// Find head nodes (no incoming edges)
fn find_heads(graph: &StepGraph) -> Vec<StepGraphIndex> {
    let mut has_incoming: HashSet<StepGraphIndex> = HashSet::new();
    for edge in graph.edge_references() {
        has_incoming.insert(edge.target());
    }
    graph.node_indices().filter(|idx| !has_incoming.contains(idx)).collect()
}

/// Get parents sorted by edge order
fn get_sorted_parents(graph: &StepGraph, node: StepGraphIndex) -> Vec<StepGraphIndex> {
    let mut parents: Vec<_> = graph.edges(node).map(|e| (e.weight().order, e.target())).collect();
    parents.sort_by_key(|(order, _)| *order);
    parents.into_iter().map(|(_, p)| p).collect()
}

/// Topological sort using Kahn's algorithm with DFS-style branch following
fn topological_order(graph: &StepGraph, heads: &[StepGraphIndex]) -> Vec<StepGraphIndex> {
    use std::collections::HashMap;

    let mut result = Vec::new();
    let mut visited: HashSet<StepGraphIndex> = HashSet::new();

    // Count incoming edges
    let mut in_degree: HashMap<StepGraphIndex, usize> = HashMap::new();
    for idx in graph.node_indices() {
        in_degree.insert(idx, 0);
    }
    for edge in graph.edge_references() {
        *in_degree.get_mut(&edge.target()).unwrap() += 1;
    }

    fn dfs(
        node: StepGraphIndex,
        graph: &StepGraph,
        visited: &mut HashSet<StepGraphIndex>,
        in_degree: &mut HashMap<StepGraphIndex, usize>,
        result: &mut Vec<StepGraphIndex>,
    ) {
        if visited.contains(&node) || in_degree[&node] > 0 {
            return;
        }

        visited.insert(node);
        result.push(node);

        let parents = get_sorted_parents(graph, node);
        for parent in &parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg = deg.saturating_sub(1);
            }
        }
        for parent in parents {
            dfs(parent, graph, visited, in_degree, result);
        }
    }

    for &head in heads {
        dfs(head, graph, &mut visited, &mut in_degree, &mut result);
    }

    result
}

/// Phase 1: Compute layout events
fn compute_layout(graph: &StepGraph, order: &[StepGraphIndex]) -> Vec<LayoutEvent> {
    let mut state = LayoutState::new();
    let mut events = Vec::new();

    for &node in order {
        let parents = get_sorted_parents(graph, node);

        // Find rails waiting for this node
        let waiting = state.rails_waiting_for(node);

        // Determine this node's column
        let col = if waiting.is_empty() {
            // New head - assign to first empty rail
            let c = state.find_or_create_empty_rail();
            state.set_rail(c, Some(node));
            c
        } else {
            // Use leftmost waiting rail
            waiting[0]
        };

        // Handle merge if multiple rails converge
        if waiting.len() > 1 {
            // Clear all but the leftmost
            for &c in &waiting[1..] {
                state.clear_rail(c);
            }
            events.push(LayoutEvent::Merge {
                from_cols: waiting.clone(),
                active: state.active_snapshot(),
            });
        }

        // Emit node event
        events.push(LayoutEvent::Node {
            col,
            node,
            active: state.active_snapshot(),
        });

        // Update rails for parents
        if parents.is_empty() {
            // Root node - emit termination event, then clear rail
            events.push(LayoutEvent::Terminate {
                col,
                active: state.active_snapshot(),
            });
            state.clear_rail(col);
        } else {
            // First parent inherits this rail
            state.set_rail(col, Some(parents[0]));

            // Additional parents need rails
            if parents.len() > 1 {
                let mut parent_cols = vec![col];

                for &parent in &parents[1..] {
                    // Check if parent already has a rail assigned
                    let parent_col = if let Some(existing) = state.find_rail_for(parent) {
                        existing
                    } else {
                        // Assign new rail, preferring adjacent columns
                        let new_col = state.find_or_create_empty_rail_from(col + 1);
                        state.set_rail(new_col, Some(parent));
                        new_col
                    };
                    parent_cols.push(parent_col);
                }

                events.push(LayoutEvent::Fork {
                    from_col: col,
                    to_cols: parent_cols,
                    active: state.active_snapshot(),
                });
            }
        }
    }

    events
}

/// Phase 2: Render events to ASCII
fn render_events<F>(graph: &StepGraph, events: &[LayoutEvent], mut get_title: F) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let mut lines = Vec::new();

    for event in events {
        match event {
            LayoutEvent::Node { col, node, active } => {
                let step = &graph[*node];
                let title = match step {
                    Step::Pick(Pick { id, .. }) => get_title(*id),
                    _ => None,
                };
                lines.push(render_node_line(*col, step, active.as_slice(), title));
            }
            LayoutEvent::Fork {
                from_col,
                to_cols,
                active,
            } => {
                lines.push(render_fork_line(*from_col, to_cols, active));
            }
            LayoutEvent::Merge { from_cols, active } => {
                lines.push(render_merge_line(from_cols, active));
            }
            LayoutEvent::Terminate { col, active } => {
                lines.push(render_terminate_line(*col, active));
            }
        }
    }

    lines.join("\n")
}

/// Render a node line
fn render_node_line(col: usize, step: &Step, active: &[bool], title: Option<String>) -> String {
    let width = active.len().max(col + 1);
    let mut cells: Vec<[char; 2]> = vec![[' ', ' ']; width];

    for (c, &is_active) in active.iter().enumerate() {
        if c == col {
            cells[c] = [step.to_symbol(), ' '];
        } else if is_active {
            cells[c] = [chars::VERT, ' '];
        }
    }

    let grid: String = cells.iter().flat_map(|c| c.iter()).collect();
    format!("{} {}", grid.trim_end(), format_step(step, title))
}

/// Render a fork line
fn render_fork_line(from_col: usize, to_cols: &[usize], active: &[bool]) -> String {
    if to_cols.len() <= 1 {
        return String::new();
    }

    let max_col = *to_cols.iter().max().unwrap();
    let width = active.len().max(max_col + 1);
    let mut cells: Vec<[char; 2]> = vec![[' ', ' ']; width];

    // The diverging columns (skip first which continues straight)
    let diverging: HashSet<usize> = to_cols.iter().skip(1).copied().collect();

    for (c, cell) in cells.iter_mut().enumerate().take(width) {
        if c == from_col {
            // Fork origin
            *cell = [chars::VERT_RIGHT, chars::HORIZ];
        } else if c > from_col && c < max_col {
            if diverging.contains(&c) {
                *cell = [chars::FORK_DOWN, chars::HORIZ];
            } else if active.get(c).copied().unwrap_or(false) {
                *cell = [chars::CROSS, chars::HORIZ];
            } else {
                *cell = [chars::HORIZ, chars::HORIZ];
            }
        } else if c == max_col && diverging.contains(&c) {
            *cell = [chars::CORNER_DR, ' '];
        } else if active.get(c).copied().unwrap_or(false) {
            *cell = [chars::VERT, ' '];
        }
    }

    let grid: String = cells.iter().flat_map(|c| c.iter()).collect();
    grid.trim_end().to_string()
}

/// Render a merge line
fn render_merge_line(from_cols: &[usize], active: &[bool]) -> String {
    if from_cols.len() <= 1 {
        return String::new();
    }

    let min_col = *from_cols.iter().min().unwrap();
    let max_col = *from_cols.iter().max().unwrap();
    let width = active.len().max(max_col + 1);
    let mut cells: Vec<[char; 2]> = vec![[' ', ' ']; width];

    let merging: HashSet<usize> = from_cols.iter().copied().collect();

    for (c, cell) in cells.iter_mut().enumerate().take(width) {
        if c == min_col {
            // Merge target (leftmost)
            *cell = [chars::VERT_RIGHT, chars::HORIZ];
        } else if c > min_col && c < max_col {
            if merging.contains(&c) {
                *cell = [chars::MERGE_UP, chars::HORIZ];
            } else if active.get(c).copied().unwrap_or(false) {
                *cell = [chars::CROSS, chars::HORIZ];
            } else {
                *cell = [chars::HORIZ, chars::HORIZ];
            }
        } else if c == max_col {
            *cell = [chars::CORNER_UR, ' '];
        } else if active.get(c).copied().unwrap_or(false) {
            *cell = [chars::VERT, ' '];
        }
    }

    let grid: String = cells.iter().flat_map(|c| c.iter()).collect();
    grid.trim_end().to_string()
}

/// Render a termination line (branch with no parents)
fn render_terminate_line(col: usize, active: &[bool]) -> String {
    let width = active.len().max(col + 1);
    let mut cells: Vec<[char; 2]> = vec![[' ', ' ']; width];

    for (c, &is_active) in active.iter().enumerate() {
        if c == col {
            cells[c] = [chars::TERM_UP, ' '];
        } else if is_active {
            cells[c] = [chars::VERT, ' '];
        }
    }

    let grid: String = cells.iter().flat_map(|c| c.iter()).collect();
    grid.trim_end().to_string()
}

/// Main entry point: render graph as ASCII
pub(crate) fn render_ascii_graph<F>(graph: &StepGraph, get_title: F) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let heads = find_heads(graph);
    let order = topological_order(graph, &heads);

    if order.is_empty() {
        return String::new();
    }

    let events = compute_layout(graph, &order);
    render_events(graph, &events, get_title)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn make_pick(hex: &str) -> Step {
        Step::Pick(Pick::new_pick(gix::ObjectId::from_str(hex).unwrap()))
    }

    fn make_ref(name: &str) -> Step {
        Step::Reference {
            refname: gix::refs::FullName::try_from(format!("refs/heads/{name}")).unwrap(),
        }
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ● 1111111
        ● 2222222
        ● 3333333
        ◌ no-op
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─╮
        ● │ aaaaaaa
        │ ● bbbbbbb
        ├─╯
        ● ccccccc
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─╮
        ● │ │ aaaaaaa
        │ ● │ bbbbbbb
        │ │ ● ccccccc
        ├─┴─╯
        ● ddddddd
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─╮
        ● │ fffffff
        ├─╪─┬─╮
        ● │ │ │ 1111111
        │ │ ● │ 2222222
        │ │ │ ● 3333333
        │ ● │ │ bbbbbbb
        ├─┴─┴─╯
        ● ccccccc
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─┬─╮
        ● │ │ │ aaaaaaa
        │ ● │ │ bbbbbbb
        │ │ ● │ ccccccc
        │ │ │ ● ddddddd
        ├─┴─┴─╯
        ● eeeeeee
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─╮
        ● │ a1a1a1a
        ● │ a2a2a2a
        ● │ a3a3a3a
        │ ● bbbbbbb
        ├─╯
        ● ccccccc
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─╮
        ● │ bbbbbbb
        ├─╪─╮
        ● │ │ ddddddd
        │ │ ● eeeeeee
        │ ● │ ccccccc
        ├─┴─╯
        ● fffffff
        ╵
        ");
    }

    #[test]
    fn extension_pushes_multiple_branches() {
        // M forks to F,B,C where F then forks to X,Y,Z
        // B and C should both be pushed right before F's fork
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─╮
        ● │ │ fffffff
        ├─╪─╪─┬─╮
        ● │ │ │ │ 1111111
        │ │ │ ● │ 2222222
        │ │ │ │ ● 3333333
        │ ● │ │ │ bbbbbbb
        │ │ ● │ │ ccccccc
        ├─┴─┴─┴─╯
        ● ddddddd
        ╵
        ");
    }

    #[test]
    fn fork_with_shared_parent_relocates_parallel_branch() {
        // Tests relocation when a fork has a parent shared with another branch.
        // B (at col 1) must be relocated before D can fork to E and F.
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─╮
        ● │ │ aaaaaaa
        ● │ │ ddddddd
        ├─╪─╪─╮
        ● │ │ │ eeeeeee
        │ ● │ │ bbbbbbb
        │ │ ● │ ccccccc
        │ │ ├─╯
        │ │ ● fffffff
        ├─┴─╯
        ● 0000000
        ╵
        ");
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
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─╮
        ● │ │ aaaaaaa
        ● │ │ ddddddd
        ├─╪─╪─╮
        ● │ │ │ eeeeeee
        │ ● │ │ bbbbbbb
        │ │ ● │ ccccccc
        │ ├─┴─╯
        │ ● 9999999
        ├─╯
        ● fffffff
        ╵
        ");
    }

    #[test]
    fn wide_fork_relocates_multiple_branches_with_crossing() {
        // A 3-way fork that requires relocating parallel branches.
        // Tests extension line crossing when multiple branches are pushed right.
        //
        //      M
        //     /|\
        //    A B C
        //    |   |
        //    D   |    <- B stays parallel, D continues from A
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

        // C -> shared (establishes shared at column 2)
        add_edge(&mut graph, c, shared, 0);

        // D forks to E, F, shared - but shared is already at col 2
        add_edge(&mut graph, d, e, 0);
        add_edge(&mut graph, d, f, 1);
        add_edge(&mut graph, d, shared, 2);

        // B, E, F, shared all merge to base
        // add_edge(&mut graph, b, base, 0);
        add_edge(&mut graph, e, base, 0);
        add_edge(&mut graph, f, base, 0);
        add_edge(&mut graph, shared, base, 0);

        let output = render_ascii_graph(&graph, |_| None);
        insta::assert_snapshot!(output, @"
        ◎ refs/heads/main
        ├─┬─╮
        ● │ │ aaaaaaa
        ● │ │ ddddddd
        ├─╪─╪─┬─╮
        ● │ │ │ │ eeeeeee
        │ │ │ ● │ fffffff
        │ ● │ │ │ bbbbbbb
        │ ╵ │ │ │
        │   ● │ │ ccccccc
        │   ├─╪─╯
        │   ● │ 1111111
        ├───┴─╯
        ● 0000000
        ╵
        ");
    }
}
