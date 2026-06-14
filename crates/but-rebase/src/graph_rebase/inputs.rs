//! Branch selection for the step graph: which branches the rebase operates on, and in what order.
//!
//! [`select_branches`] does a breadth-first visit over a [`BranchGraph`](but_graph::BranchGraph)'s
//! adjacency list from the entrypoint and any extra refs; [`Editor::create_with_opts`] then builds
//! the step graph directly from that order and the branches themselves.

use std::collections::HashSet;

use anyhow::{Context as _, Result, bail};

use crate::graph_rebase::ExtraRefMutability;

/// Select and order the branches for the step graph: a breadth-first visit over the
/// [`BranchGraph`](but_graph::BranchGraph) from the entrypoint and any extra refs, seeding empty
/// local branches so they stay selectable.
///
/// Returns the visit `order` (indices into [`BranchGraph::branches`](but_graph::BranchGraph::branches)),
/// the `connections` between selected branches in that order-index space `(source, target, parent
/// order)`, and the references that may be traversed but not mutated.
#[allow(clippy::type_complexity)]
pub(crate) fn select_branches(
    bg: &but_graph::BranchGraph,
    extra_refs: &[crate::graph_rebase::ExtraRef<'_>],
) -> Result<(
    Vec<usize>,
    Vec<(usize, usize, usize)>,
    HashSet<gix::refs::FullName>,
)> {
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
    let mut immutable_entrypoints = vec![];
    for extra_ref in extra_refs {
        let Some(idx) = find_by_ref(extra_ref.ref_name) else {
            bail!(
                "Failed to find corresponding branch for {}",
                extra_ref.ref_name
            );
        };
        if extra_ref.mutability == ExtraRefMutability::Mutable {
            mutable_entrypoints.push(idx);
        } else {
            immutable_entrypoints.push(idx);
        }
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
    let mut immutable_references = HashSet::new();
    for start in immutable_entrypoints {
        let from = order.len();
        bfs(start, &mut seen, &mut order);
        for &idx in &order[from..] {
            immutable_references.extend(branches[idx].ref_name.clone());
            immutable_references.extend(
                branches[idx]
                    .commits
                    .iter()
                    .flat_map(|c| c.refs.iter().map(|ri| ri.ref_name.clone())),
            );
        }
    }

    // An empty branch at the base is a leaf the entrypoint visit never reaches (it points down,
    // nothing points at it) yet must stay selectable — so seed from every remaining empty local
    // branch. Only empty ones: a branch with commits belongs to the lane that reaches it.
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

    // Edges in `outgoing` address branches by their index in `branches`; the visit order is a
    // subset, so relabel each edge into order-index space, dropping edges to branches the visit
    // never selected. Branch indices are dense, so a Vec beats a map.
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

    Ok((order, connections, immutable_references))
}
