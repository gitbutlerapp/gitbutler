//! The REF-PLACEMENT LEDGER: everything editor creation needs to place references, addressed
//! by COMMIT ID and REF NAME instead of arena indices — the commit-addressed distillation of
//! the segment walk it replaced. [`derive`] builds it straight from the segment graph, and
//! native creation builds the editor graph from the carried `CommitGraph` plus this ledger.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};

/// One reference in canonical (commit/name-addressed) form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedRef {
    /// The full reference name.
    pub name: gix::refs::FullName,
    /// Whether the rebase may move this reference.
    pub mutable: bool,
    /// The commit the reference sits on; `None` for unborn refs (no stored position).
    pub anchor: Option<gix::ObjectId>,
    /// The name of the reference directly underneath in the physical stack.
    pub below: Option<gix::refs::FullName>,
    /// The stored convergence signal (see `RefPosition::ambiguous`).
    pub ambiguous: bool,
    /// The approach legs as `(source commit, parent-slot)`, sorted.
    pub approach: Vec<(gix::ObjectId, usize)>,
}

/// The full ledger: refs in arena order (which IS the segment-walk insertion order — the
/// native build re-adds them in this order to preserve ref indices and render sibling order),
/// plus what creation derives alongside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefPlacements {
    /// All live references in ref-arena order.
    pub refs: Vec<PlacedRef>,
    /// Commits whose picks are mutable (reachable from a mutable entrypoint).
    pub mutable_commits: HashSet<gix::ObjectId>,
    /// The reference names the HEAD checkouts point at, in checkout order.
    pub head_refs: Vec<gix::refs::FullName>,
    /// The managed workspace commit's parent SLOTS — one per workspace lane, so empty lanes
    /// over one base yield duplicate entries the real commit does not have. Lane data, not
    /// commit data: creation deliberately keeps the segment wiring here (the parent-fixup
    /// pass skips the ws commit).
    pub ws_parents: Option<Vec<gix::ObjectId>>,
}

/// Derive the ledger STRAIGHT from the segment graph — no step-graph intermediate. This
/// mirrors the segment walk's semantics exactly, on a throwaway IR: per-segment runs
/// (segment ref, then per commit its refs then the commit), rank-ordered inter-segment
/// edges, the parent fixup (a commit whose chain-flattened parents disagree with its raw
/// parent list is rewired directly, bypassing chains — the ws commit and partially-traversed
/// commits keep their wiring), position derivation, and the strip's slot compaction.
pub(crate) fn derive(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
    options: &crate::graph_rebase::GraphEditorOptions,
) -> Result<RefPlacements> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum IrStep {
        /// Index into `ref_table`.
        Ref(usize),
        /// Index into `commit_table`.
        Commit(usize),
        /// The placeholder for a segment with neither name nor commits.
        None,
    }

    let graph = &workspace.graph;
    let entrypoint = graph.entrypoint()?;

    let mut mutable_entrypoints = vec![entrypoint.segment.id];
    for ref_name in &options.extra_mutable_refs {
        let Some((segment, _)) = graph.segment_and_commit_by_ref_name(ref_name.as_ref()) else {
            bail!("Failed to find corresponding segment for {ref_name}");
        };
        mutable_entrypoints.push(segment.id);
    }
    let mut mutable_segments = HashSet::new();
    for ep in mutable_entrypoints {
        graph.visit_all_segments_including_start_until(
            ep,
            but_graph::Direction::Outgoing,
            |segment| !mutable_segments.insert(segment.id),
        );
    }

    let workspace_commit_id = graph.managed_entrypoint_commit(repo)?.map(|c| c.id);

    // IR build: one run of nodes per segment, in `graph.segments()` order — which fixes the
    // ledger's ref order.
    let mut nodes: Vec<IrStep> = Vec::new();
    let mut parents: Vec<Vec<usize>> = Vec::new();
    let mut ref_table: Vec<(gix::refs::FullName, bool)> = Vec::new();
    let mut commit_table: Vec<(gix::ObjectId, Vec<gix::ObjectId>)> = Vec::new();
    let mut commit_node = HashMap::<gix::ObjectId, usize>::new();
    let mut mutable_commits = HashSet::new();
    let mut head_refs = Vec::new();
    let mut runs = Vec::new();

    for sid in graph.segments() {
        let segment = &graph[sid];
        let mutable = mutable_segments.contains(&sid);
        let mut run: Vec<usize> = vec![];
        let push = |nodes: &mut Vec<IrStep>, parents: &mut Vec<Vec<usize>>, step| {
            nodes.push(step);
            parents.push(vec![]);
            nodes.len() - 1
        };

        if let Some(reference) = segment.ref_name() {
            if Some(reference) == entrypoint.segment.ref_name() {
                head_refs.push(reference.to_owned());
            }
            ref_table.push((reference.to_owned(), mutable));
            let n = push(&mut nodes, &mut parents, IrStep::Ref(ref_table.len() - 1));
            run.push(n);
        }
        for commit in &segment.commits {
            if mutable {
                mutable_commits.insert(commit.id);
            }
            for r in &commit.refs {
                ref_table.push((r.ref_name.clone(), mutable));
                let n = push(&mut nodes, &mut parents, IrStep::Ref(ref_table.len() - 1));
                if let Some(&previous) = run.last() {
                    parents[previous].push(n);
                }
                run.push(n);
            }
            commit_table.push((commit.id, commit.parent_ids.clone()));
            let n = push(
                &mut nodes,
                &mut parents,
                IrStep::Commit(commit_table.len() - 1),
            );
            commit_node.insert(commit.id, n);
            if let Some(&previous) = run.last() {
                parents[previous].push(n);
            }
            run.push(n);
        }
        if run.is_empty() {
            run.push(push(&mut nodes, &mut parents, IrStep::None));
        }
        runs.push((sid, run));
    }

    // Rank-ordered inter-segment edges onto each run's LAST node: real parents by their index
    // in the source commit's parent array, commit-less legs after them in edge order, ranks
    // compacted by push order.
    let parents_by_commit: HashMap<gix::ObjectId, &[gix::ObjectId]> = commit_table
        .iter()
        .map(|(id, parent_ids)| (*id, parent_ids.as_slice()))
        .collect();
    let first_node_of_segment: HashMap<but_graph::SegmentIndex, usize> = runs
        .iter()
        .map(|(sid, run)| (*sid, *run.first().expect("every run has a node")))
        .collect();
    for (sid, run) in &runs {
        let source = *run.last().expect("every run has a node");
        let mut empty_branch_count = 0usize;
        let mut ranked_targets = Vec::new();
        for edge in graph.edges_directed(*sid, but_graph::Direction::Outgoing) {
            let Some(&target) = first_node_of_segment.get(&edge.target()) else {
                continue;
            };
            let edge_parents = edge
                .weight()
                .src_id()
                .and_then(|src| parents_by_commit.get(&src).copied());
            let real_parent_index = edge_parents
                .zip(edge.weight().dst_id())
                .and_then(|(parents, dst)| parents.iter().position(|p| *p == dst));
            let rank = match real_parent_index {
                Some(idx) => idx,
                None => {
                    let o = edge_parents.map_or(0, |p| p.len()) + empty_branch_count;
                    empty_branch_count += 1;
                    o
                }
            };
            ranked_targets.push((rank, target));
        }
        ranked_targets.sort_by_key(|(rank, _)| *rank);
        for (_, target) in ranked_targets {
            parents[source].push(target);
        }
    }

    // The fixup: flatten a commit's chain parents in slot order; on disagreement with the
    // RAW parent list, rewire directly to present commits (chains lose their legs). The ws
    // commit and partially-traversed commits keep their segment wiring.
    let commit_ids: HashSet<gix::ObjectId> = commit_table.iter().map(|(id, _)| *id).collect();
    let flatten = |nodes: &[IrStep], parents: &[Vec<usize>], start: usize| {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = parents[start].iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            match nodes[n] {
                IrStep::Commit(c) => out.push(c),
                IrStep::Ref(_) | IrStep::None => {
                    stack.extend(parents[n].iter().rev().copied());
                }
            }
        }
        out
    };
    for (id, raw_parents) in &commit_table {
        if Some(*id) == workspace_commit_id {
            continue;
        }
        let preserved =
            !raw_parents.is_empty() && raw_parents.iter().any(|p| !commit_ids.contains(p));
        if preserved {
            continue;
        }
        let n = commit_node[id];
        let flat_ids: Vec<gix::ObjectId> = flatten(&nodes, &parents, n)
            .into_iter()
            .map(|c| commit_table[c].0)
            .collect();
        if flat_ids == *raw_parents {
            continue;
        }
        parents[n] = raw_parents
            .iter()
            .filter_map(|p| commit_node.get(p).copied())
            .collect();
    }

    // Positions from the (post-fixup, pre-strip) topology: descend first-edges for anchor and
    // below, ascend for the approach legs and the convergence signal.
    let mut incoming: Vec<Vec<(usize, usize)>> = vec![Vec::new(); nodes.len()];
    for (child, slots) in parents.iter().enumerate() {
        for (slot, &parent) in slots.iter().enumerate() {
            incoming[parent].push((child, slot));
        }
    }
    let is_commit = |n: usize| matches!(nodes[n], IrStep::Commit(_));
    let ref_nodes: Vec<(usize, usize)> = nodes
        .iter()
        .enumerate()
        .filter_map(|(n, step)| match step {
            IrStep::Ref(r) => Some((n, *r)),
            _ => None,
        })
        .collect();
    struct DerivedPosition {
        anchor: usize,
        below: Option<usize>,
        ambiguous: bool,
        approach: Vec<(usize, usize)>,
    }
    let mut positions = HashMap::<usize, DerivedPosition>::new();
    for &(ref_node, _) in &ref_nodes {
        let mut cursor = ref_node;
        let mut anchor = None;
        let mut below = None;
        for _ in 0..10_000 {
            let Some(&next) = parents[cursor].first() else {
                break;
            };
            if is_commit(next) {
                anchor = Some(next);
                break;
            }
            if matches!(nodes[next], IrStep::Ref(_)) && below.is_none() {
                below = Some(next);
            }
            cursor = next;
        }
        let Some(anchor) = anchor else {
            continue; // unborn: no stored position
        };
        let mut cursor = ref_node;
        let mut approach = Vec::new();
        let mut ambiguous = false;
        for _ in 0..10_000 {
            let entering = &incoming[cursor];
            let picks: Vec<_> = entering
                .iter()
                .copied()
                .filter(|&(child, _)| is_commit(child))
                .collect();
            if !picks.is_empty() {
                ambiguous = entering.len() > 1;
                approach = picks;
                break;
            }
            let mut others = entering.iter().filter(|&&(child, _)| !is_commit(child));
            match (others.next(), others.next()) {
                (Some(&(child, _)), None) => cursor = child,
                _ => break,
            }
        }
        positions.insert(
            ref_node,
            DerivedPosition {
                anchor,
                below,
                ambiguous,
                approach,
            },
        );
    }

    // The strip's slot compaction: resolve each commit's parent entries to commits (dropping
    // unborn chains), record the vacated slots, and rename the captured approach legs.
    let resolve = |start: usize| -> Option<usize> {
        let mut cursor = start;
        for _ in 0..10_000 {
            if is_commit(cursor) {
                return Some(cursor);
            }
            cursor = *parents[cursor].first()?;
        }
        None
    };
    let mut dropped: Vec<(usize, usize)> = Vec::new();
    let mut final_parents = HashMap::<gix::ObjectId, Vec<gix::ObjectId>>::new();
    for (id, _) in &commit_table {
        let n = commit_node[id];
        let mut resolved = Vec::with_capacity(parents[n].len());
        for (slot, &parent) in parents[n].iter().enumerate() {
            match resolve(parent) {
                Some(pick) => {
                    let IrStep::Commit(c) = nodes[pick] else {
                        unreachable!("resolve returns commits");
                    };
                    resolved.push(commit_table[c].0);
                }
                None => dropped.push((n, slot)),
            }
        }
        final_parents.insert(*id, resolved);
    }
    for position in positions.values_mut() {
        for (leg_source, slot) in position.approach.iter_mut() {
            *slot -= dropped
                .iter()
                .filter(|(source, vacated)| source == leg_source && vacated < slot)
                .count();
        }
    }

    // Emit, in ref-table order (= the step-graph ref arena order).
    let node_of_ref: HashMap<usize, usize> = ref_nodes.iter().map(|&(n, r)| (r, n)).collect();
    let mut refs = Vec::with_capacity(ref_table.len());
    for (r, (name, mutable)) in ref_table.iter().enumerate() {
        let ref_node = node_of_ref[&r];
        let mut anchor = None;
        let mut below = None;
        let mut ambiguous = false;
        let mut approach = Vec::new();
        if let Some(position) = positions.get(&ref_node) {
            let IrStep::Commit(c) = nodes[position.anchor] else {
                unreachable!("anchors are commits");
            };
            anchor = Some(commit_table[c].0);
            below = position.below.map(|b| {
                let IrStep::Ref(br) = nodes[b] else {
                    unreachable!("below entries are refs");
                };
                ref_table[br].0.clone()
            });
            ambiguous = position.ambiguous;
            for &(child, slot) in &position.approach {
                let IrStep::Commit(c) = nodes[child] else {
                    unreachable!("approach legs come from commits");
                };
                approach.push((commit_table[c].0, slot));
            }
            approach.sort_unstable();
        }
        refs.push(PlacedRef {
            name: name.clone(),
            mutable: *mutable,
            anchor,
            below,
            ambiguous,
            approach,
        });
    }

    let ws_parents = workspace_commit_id.and_then(|id| final_parents.get(&id).cloned());

    Ok(RefPlacements {
        refs,
        mutable_commits,
        head_refs,
        ws_parents,
    })
}
