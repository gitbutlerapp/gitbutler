//! The editor-grade ref layout, derived from the FINISHED segment graph: every surfaced
//! reference in segment order with its position over the commit graph, plus the entrypoint's
//! reach (mutability), HEAD ordinals, and the workspace commit's stack slots.
//!
//! This is the position derivation the rebase editor historically ran itself (reverse-
//! engineering segment topology at creation time); authored here once per build and stored on
//! the commit graph's [`RefArrangement`](crate::ref_arrangement::RefArrangement), the editor
//! becomes a pure table consumer. The derivation still READS segments — it retires with the
//! segment graph once a commit-graph native authoring exists.

use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::ref_arrangement::{PositionedRef, RefPosition, RefPositions};

#[derive(Clone, Copy, PartialEq, Eq)]
enum IrStep {
    /// Index into `ref_table`.
    Ref(usize),
    /// Index into `commit_table`.
    Commit(usize),
    /// The placeholder for a segment with neither name nor commits.
    None,
}

/// Derive the layout from the finished `graph`. `repo` detects the managed entrypoint commit
/// (by message), which takes its STACK slots instead of its real parents.
///
/// The derivation mirrors the retired segment walk's semantics exactly, on a throwaway IR:
/// per-segment runs (segment ref, then per commit its refs then the commit), rank-ordered
/// inter-segment edges, the parent fixup (a commit whose group-flattened parents disagree
/// with its raw parent list is rewired directly, bypassing groups — the ws commit and
/// partially-traversed commits keep their wiring), position derivation, and the strip's
/// slot compaction.
pub(crate) fn ref_positions(graph: &crate::Graph, repo: &gix::Repository) -> Result<RefPositions> {
    let entrypoint = graph.entrypoint()?;

    let mut reachable_segments = HashSet::new();
    graph.visit_all_segments_including_start_until(
        entrypoint.segment.id,
        crate::Direction::Outgoing,
        |segment| !reachable_segments.insert(segment.id),
    );

    let workspace_commit_id = graph.managed_entrypoint_commit(repo)?.map(|c| c.id);

    // IR build: one run of nodes per segment, in `graph.segments()` order — which fixes the
    // stored ref order (and with it the editor's reference table and render sibling order).
    let mut nodes: Vec<IrStep> = Vec::new();
    let mut parents: Vec<Vec<usize>> = Vec::new();
    let mut ref_table: Vec<(gix::refs::FullName, bool)> = Vec::new();
    let mut commit_table: Vec<(gix::ObjectId, Vec<gix::ObjectId>)> = Vec::new();
    let mut commit_node = HashMap::<gix::ObjectId, usize>::new();
    let mut reachable_commits = Vec::new();
    let mut head_refs = Vec::new();
    let mut runs = Vec::new();

    for sid in graph.segments() {
        let segment = &graph[sid];
        let reachable = reachable_segments.contains(&sid);
        let mut run: Vec<usize> = vec![];
        let push = |nodes: &mut Vec<IrStep>, parents: &mut Vec<Vec<usize>>, step| {
            nodes.push(step);
            parents.push(vec![]);
            nodes.len() - 1
        };

        if let Some(reference) = segment.ref_name() {
            if Some(reference) == entrypoint.segment.ref_name() {
                head_refs.push(ref_table.len());
            }
            ref_table.push((reference.to_owned(), reachable));
            let n = push(&mut nodes, &mut parents, IrStep::Ref(ref_table.len() - 1));
            run.push(n);
        }
        for commit in &segment.commits {
            if reachable {
                reachable_commits.push(commit.id);
            }
            for r in &commit.refs {
                ref_table.push((r.ref_name.clone(), reachable));
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
    // in the source commit's parent array, commit-less edges after them in edge order, ranks
    // compacted by push order.
    let parents_by_commit: HashMap<gix::ObjectId, &[gix::ObjectId]> = commit_table
        .iter()
        .map(|(id, parent_ids)| (*id, parent_ids.as_slice()))
        .collect();
    let first_node_of_segment: HashMap<crate::SegmentIndex, usize> = runs
        .iter()
        .map(|(sid, run)| (*sid, *run.first().expect("every run has a node")))
        .collect();
    for (sid, run) in &runs {
        let source = *run.last().expect("every run has a node");
        let mut empty_branch_count = 0usize;
        let mut ranked_targets = Vec::new();
        for edge in graph.edges_directed(*sid, crate::Direction::Outgoing) {
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

    // The fixup: flatten a commit's group parents in slot order; on disagreement with the
    // RAW parent list, rewire directly to present commits (groups lose their edges). The ws
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

    // Positions from the (post-fixup, pre-strip) topology: descend first-edges for `on` and
    // below, ascend for the entering edges and the convergence signal.
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
        on: usize,
        below: Option<usize>,
        ambiguous: bool,
        entering: Vec<(usize, usize)>,
    }
    let mut positions = HashMap::<usize, DerivedPosition>::new();
    for &(ref_node, _) in &ref_nodes {
        let mut cursor = ref_node;
        let mut on = None;
        let mut below = None;
        for _ in 0..10_000 {
            let Some(&next) = parents[cursor].first() else {
                break;
            };
            if is_commit(next) {
                on = Some(next);
                break;
            }
            if matches!(nodes[next], IrStep::Ref(_)) && below.is_none() {
                below = Some(next);
            }
            cursor = next;
        }
        let Some(on) = on else {
            continue; // unborn: no stored position
        };
        let mut cursor = ref_node;
        let mut entering_edges = Vec::new();
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
                entering_edges = picks;
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
                on,
                below,
                ambiguous,
                entering: entering_edges,
            },
        );
    }

    // The strip's slot compaction: resolve each commit's parent entries to commits (dropping
    // unborn groups), record the vacated slots so the captured entering edges can be renamed,
    // and keep the ws commit's resolved STACK slots (one per stack, so empty stacks
    // over one base yield duplicate entries the real commit does not have).
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
    let mut ws_stack_slots: Option<(gix::ObjectId, Vec<gix::ObjectId>)> = None;
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
        if Some(*id) == workspace_commit_id {
            ws_stack_slots = Some((*id, resolved));
        }
    }
    for position in positions.values_mut() {
        for (edge_source, slot) in position.entering.iter_mut() {
            *slot -= dropped
                .iter()
                .filter(|(source, vacated)| source == edge_source && vacated < slot)
                .count();
        }
    }

    // The stored shape: ref-table order, node handles translated to ref ordinals and commit
    // ids, entering edges sorted.
    let node_of_ref: HashMap<usize, usize> = ref_nodes.iter().map(|&(n, r)| (r, n)).collect();
    let refs = ref_table
        .into_iter()
        .enumerate()
        .map(|(r, (name, reachable))| {
            let position = positions.get(&node_of_ref[&r]).map(|position| {
                let IrStep::Commit(c) = nodes[position.on] else {
                    unreachable!("positions sit on commits");
                };
                let below = position.below.map(|b| {
                    let IrStep::Ref(br) = nodes[b] else {
                        unreachable!("below entries are refs");
                    };
                    br
                });
                let mut entering: Vec<(gix::ObjectId, usize)> = position
                    .entering
                    .iter()
                    .map(|&(child, slot)| {
                        let IrStep::Commit(c) = nodes[child] else {
                            unreachable!("entering edges come from commits");
                        };
                        (commit_table[c].0, slot)
                    })
                    .collect();
                entering.sort_unstable();
                RefPosition {
                    on: commit_table[c].0,
                    below,
                    entering,
                    ambiguous: position.ambiguous,
                }
            });
            PositionedRef {
                name,
                reachable,
                position,
            }
        })
        .collect();

    reachable_commits.sort();
    Ok(RefPositions {
        refs,
        ws_stack_slots,
        head_refs,
        reachable_commits,
    })
}
