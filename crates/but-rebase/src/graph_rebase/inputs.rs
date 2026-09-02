//! Branch selection for the step graph: which branches the rebase operates on, in what order, and
//! which of them the editor may rewrite.
//!
//! [`select_branches`] does a breadth-first visit over a [`BranchGraph`](but_graph::BranchGraph)'s
//! adjacency list from the mutable roots (the entrypoint and any extra mutable refs), then appends
//! every remaining branch as immutable; [`Editor::create_with_opts`](super::Editor::create_with_opts)
//! then builds the step graph directly from that order and the branches themselves.

use std::collections::HashSet;

use anyhow::{Context as _, Result, bail};

/// The outcome of [`select_branches`].
pub(crate) struct SelectedBranches {
    /// The visit order, as indices into [`BranchGraph::branches`](but_graph::BranchGraph::branches).
    /// Every branch is listed exactly once.
    pub order: Vec<usize>,
    /// The connections between branches in order-index space: `(source, target, parent order)`.
    pub connections: Vec<(usize, usize, usize)>,
    /// Per order index, whether the editor may rewrite the branch: `true` for branches reachable
    /// from a mutable root by following parent edges, `false` for every other branch.
    pub mutable: Vec<bool>,
}

/// Select and order the branches for the step graph.
///
/// The editor always contains every branch of the graph. Branches reachable from the entrypoint or
/// from one of `mutable_roots` (following parent edges) are mutable; all others are included for
/// traversal only. A mutable root that names no branch is an error, as callers ask for it
/// explicitly.
pub(crate) fn select_branches<'a>(
    bg: &but_graph::BranchGraph,
    mutable_roots: impl IntoIterator<Item = &'a gix::refs::FullName>,
) -> Result<SelectedBranches> {
    let branches = &bg.branches;
    let ep = branches
        .iter()
        .position(|b| b.is_entrypoint)
        .context("BUG: a BranchGraph always has an entrypoint branch")?;

    let find_by_ref = |name: &gix::refs::FullNameRef| {
        branches.iter().position(|b| {
            b.ref_name.as_ref().map(|rn| rn.as_ref()) == Some(name)
                || b.commits
                    .iter()
                    .any(|c| c.refs.iter().any(|ri| ri.ref_name.as_ref() == name))
        })
    };
    let mut mutable_entrypoints = vec![ep];
    for ref_name in mutable_roots {
        let Some(idx) = find_by_ref(ref_name.as_ref()) else {
            bail!("Failed to find corresponding segment for {ref_name}");
        };
        mutable_entrypoints.push(idx);
    }

    let mut order = vec![];
    let mut seen = HashSet::new();
    let bfs = |start: usize, seen: &mut HashSet<usize>, order: &mut Vec<usize>| {
        let mut queue = std::collections::VecDeque::new();
        if seen.insert(start) {
            queue.push_back(start);
        }
        while let Some(idx) = queue.pop_front() {
            order.push(idx);
            for &(target, _) in &branches[idx].outgoing {
                if seen.insert(target) {
                    queue.push_back(target);
                }
            }
        }
    };
    for start in mutable_entrypoints {
        bfs(start, &mut seen, &mut order);
    }
    // An empty local branch is a leaf the entrypoint visit never reaches (it points down, nothing
    // points at it) yet belongs to the workspace as much as any lane: seed from every remaining
    // empty local branch so it stays mutable and selectable. Only empty ones: a branch with
    // commits belongs to the history that reaches it.
    for (idx, branch) in branches.iter().enumerate() {
        let is_empty_local_branch = branch.commits.is_empty()
            && branch
                .ref_name
                .as_ref()
                .is_some_and(|rn| rn.category() == Some(gix::refs::Category::LocalBranch));
        if is_empty_local_branch && !seen.contains(&idx) {
            bfs(idx, &mut seen, &mut order);
        }
    }
    let mutable_count = order.len();

    // Everything else is part of the editor too, but immutable: the graph is copied verbatim
    // there, so history the workspace merely sees (targets, remotes, other lanes) keeps its
    // identity while still being available for traversal.
    for idx in 0..branches.len() {
        if !seen.contains(&idx) {
            bfs(idx, &mut seen, &mut order);
        }
    }
    let mutable = (0..order.len()).map(|i| i < mutable_count).collect();

    // Edges in `outgoing` address branches by their index in `branches`; relabel each edge into
    // order-index space. Branch indices are dense, so a Vec beats a map.
    let mut order_of_branch = vec![None; branches.len()];
    for (order_idx, &branch_idx) in order.iter().enumerate() {
        order_of_branch[branch_idx] = Some(order_idx);
    }
    let mut connections = vec![];
    for (order_idx, &branch_idx) in order.iter().enumerate() {
        for &(target, parent_order) in &branches[branch_idx].outgoing {
            let Some(target_order) = order_of_branch[target] else {
                continue;
            };
            connections.push((order_idx, target_order, parent_order as usize));
        }
    }

    Ok(SelectedBranches {
        order,
        connections,
        mutable,
    })
}
