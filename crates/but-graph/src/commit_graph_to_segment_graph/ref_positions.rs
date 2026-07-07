//! The editor-grade ref layout: every surfaced reference in segment order with its position
//! over the commit graph, plus the entrypoint's reach (mutability), HEAD ordinals, and the
//! workspace commit's chain slots.
//!
//! This is the position derivation the rebase editor historically ran itself (reverse-
//! engineering segment topology at creation time); authored here once per build and stored on
//! the commit graph's [`RefArrangement`](crate::ref_arrangement::RefArrangement), the editor
//! becomes a pure table consumer.
//!
//! The derivation is split: [`positions_from_ir`] computes the layout from a graph-agnostic
//! [`Ir`] — segments as linear runs of data, in minting order — and a front-end authors the
//! IR. The production front-end is the commit-graph native store ([`author_positions`]); the
//! segment-walk front-end ([`ref_positions`]) remains as the debug oracle.

use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result};

use crate::ref_arrangement::{PositionedRef, RefPosition, RefPositions};

/// One commit of an [`IrRun`]: its raw parents and the passive refs riding on it.
struct IrCommit {
    id: gix::ObjectId,
    parent_ids: Vec<gix::ObjectId>,
    refs: Vec<gix::refs::FullName>,
}

/// One minted segment as a linear run: its naming ref, its commits (top → bottom), and its
/// outgoing edges as target run ordinals in FINAL rank order (real parents by their index in
/// the source commit's parent array, commit-less edges after them in edge order).
struct IrRun {
    name: Option<gix::refs::FullName>,
    commits: Vec<IrCommit>,
    targets: Vec<usize>,
}

/// The position IR: every segment as a run of data, in minting order — which fixes the stored
/// ref order (and with it the editor's reference table and render sibling order).
struct Ir {
    runs: Vec<IrRun>,
    /// The entrypoint's run: the root of the reach computation, and its name marks the HEAD
    /// ordinals.
    entrypoint_run: usize,
    /// The managed entrypoint commit, which takes its resolved CHAIN slots instead of its
    /// real parents.
    workspace_commit_id: Option<gix::ObjectId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IrStep {
    /// Index into `ref_table`.
    Ref(usize),
    /// Index into `commits`.
    Commit(usize),
    /// The placeholder for a run with neither name nor commits.
    None,
}

/// THE DEMOTED WALK: derive the layout from the finished `graph` — the debug oracle for
/// [`author_positions`]. `repo` detects the managed entrypoint commit (by message).
pub(super) fn ref_positions(graph: &crate::Graph, repo: &gix::Repository) -> Result<RefPositions> {
    Ok(positions_from_ir(ir_from_segment_graph(graph, repo)?))
}

/// Author the stored layout from the native `store`. The entrypoint run ordinal and the
/// managed workspace commit are graph/repo decisions, not store decisions — read here.
pub(super) fn author_positions(
    store: &super::position_ir::NativeStore,
    graph: &crate::Graph,
    repo: &gix::Repository,
) -> Result<RefPositions> {
    let entrypoint = graph.entrypoint()?;
    let workspace_commit_id = graph.managed_entrypoint_commit(repo)?.map(|c| c.id);
    let entrypoint_run = graph
        .segment_ids()
        .position(|sid| sid == entrypoint.segment.id)
        .context("BUG: the entrypoint segment is always live")?;
    Ok(native_ref_positions(
        store,
        entrypoint_run,
        workspace_commit_id,
    ))
}

/// The segment-graph front-end: one run per segment in `graph.segment_ids()` order, edges
/// rank-ordered from the segment connections.
fn ir_from_segment_graph(graph: &crate::Graph, repo: &gix::Repository) -> Result<Ir> {
    let entrypoint = graph.entrypoint()?;
    let workspace_commit_id = graph.managed_entrypoint_commit(repo)?.map(|c| c.id);

    let sids: Vec<crate::SegmentIndex> = graph.segment_ids().collect();
    let run_of_sid: HashMap<crate::SegmentIndex, usize> = sids
        .iter()
        .enumerate()
        .map(|(ri, &sid)| (sid, ri))
        .collect();

    let mut runs: Vec<IrRun> = sids
        .iter()
        .map(|&sid| {
            let segment = &graph[sid];
            IrRun {
                name: segment.ref_name().map(|r| r.to_owned()),
                commits: segment
                    .commits
                    .iter()
                    .map(|commit| IrCommit {
                        id: commit.id,
                        parent_ids: commit.parent_ids.clone(),
                        refs: commit.refs.iter().map(|r| r.ref_name.clone()).collect(),
                    })
                    .collect(),
                targets: Vec::new(),
            }
        })
        .collect();

    let parents_by_commit: HashMap<gix::ObjectId, &[gix::ObjectId]> = runs
        .iter()
        .flat_map(|run| run.commits.iter())
        .map(|c| (c.id, c.parent_ids.as_slice()))
        .collect();
    let mut targets_of_run: Vec<Vec<usize>> = Vec::with_capacity(sids.len());
    for &sid in &sids {
        targets_of_run.push(rank_ordered_targets(
            graph
                .edges_directed(sid, crate::Direction::Outgoing)
                .map(|edge| {
                    (
                        run_of_sid[&edge.target],
                        edge.connection.src_id(),
                        edge.connection.dst_id(),
                    )
                }),
            &parents_by_commit,
        ));
    }
    for (run, targets) in runs.iter_mut().zip(targets_of_run) {
        run.targets = targets;
    }

    Ok(Ir {
        runs,
        entrypoint_run: run_of_sid[&entrypoint.segment.id],
        workspace_commit_id,
    })
}

/// The commit-graph native front-end: the store IS the segment mechanics, so its runs map to
/// IR runs one-to-one. The entrypoint run and the managed workspace commit are decisions made
/// outside the store, passed in by the caller.
fn native_ref_positions(
    store: &super::position_ir::NativeStore,
    entrypoint_run: usize,
    workspace_commit_id: Option<gix::ObjectId>,
) -> RefPositions {
    let mut runs: Vec<IrRun> = store
        .runs
        .iter()
        .map(|run| IrRun {
            name: run.name.clone(),
            commits: run
                .commits
                .iter()
                .map(|commit| IrCommit {
                    id: commit.id,
                    parent_ids: commit.parent_ids.clone(),
                    refs: commit.refs.iter().map(|r| r.ref_name.clone()).collect(),
                })
                .collect(),
            targets: Vec::new(),
        })
        .collect();
    let parents_by_commit: HashMap<gix::ObjectId, &[gix::ObjectId]> = store
        .runs
        .iter()
        .flat_map(|run| run.commits.iter())
        .map(|c| (c.id, c.parent_ids.as_slice()))
        .collect();
    let targets_of_run: Vec<Vec<usize>> = store
        .runs
        .iter()
        .map(|run| {
            rank_ordered_targets(
                run.edges.iter().map(|e| (e.target, e.src_id, e.dst_id)),
                &parents_by_commit,
            )
        })
        .collect();
    for (run, targets) in runs.iter_mut().zip(targets_of_run) {
        run.targets = targets;
    }
    positions_from_ir(Ir {
        runs,
        entrypoint_run,
        workspace_commit_id,
    })
}

/// Rank-order a run's outgoing edges — `(target run, src id, dst id)` in edge order — into
/// final target order: real parents by their index in the source commit's parent array,
/// commit-less edges after them in edge order, ranks compacted by push order.
fn rank_ordered_targets(
    edges: impl Iterator<Item = (usize, Option<gix::ObjectId>, Option<gix::ObjectId>)>,
    parents_by_commit: &HashMap<gix::ObjectId, &[gix::ObjectId]>,
) -> Vec<usize> {
    let mut empty_branch_count = 0usize;
    let mut ranked_targets = Vec::new();
    for (target, src_id, dst_id) in edges {
        let edge_parents = src_id.and_then(|src| parents_by_commit.get(&src).copied());
        let real_parent_index = edge_parents
            .zip(dst_id)
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
    ranked_targets.into_iter().map(|(_, t)| t).collect()
}

/// Compute the layout from the IR: per-run steps (run ref, then per commit its refs then the
/// commit), the parent fixup (a commit whose group-flattened parents disagree with its raw
/// parent list is rewired directly, bypassing groups — the ws commit and partially-traversed
/// commits keep their wiring), position derivation, and the strip's slot compaction.
fn positions_from_ir(ir: Ir) -> RefPositions {
    // Reach: every run the entrypoint's run descends into through the ranked edges.
    let mut reachable_runs = HashSet::new();
    let mut queue = vec![ir.entrypoint_run];
    while let Some(ri) = queue.pop() {
        if reachable_runs.insert(ri) {
            queue.extend(ir.runs[ri].targets.iter().copied());
        }
    }
    let head_name = ir.runs[ir.entrypoint_run].name.clone();

    // Step build: one run of steps per IR run, in IR order.
    let mut steps: Vec<IrStep> = Vec::new();
    let mut parents: Vec<Vec<usize>> = Vec::new();
    let mut ref_table: Vec<(gix::refs::FullName, bool)> = Vec::new();
    let mut commits: Vec<(gix::ObjectId, Vec<gix::ObjectId>)> = Vec::new();
    let mut commit_step = HashMap::<gix::ObjectId, usize>::new();
    let mut reachable_commits = Vec::new();
    let mut head_refs = Vec::new();
    let mut runs = Vec::new();

    for (ri, ir_run) in ir.runs.iter().enumerate() {
        let reachable = reachable_runs.contains(&ri);
        let mut run: Vec<usize> = vec![];
        let push = |steps: &mut Vec<IrStep>, parents: &mut Vec<Vec<usize>>, step| {
            steps.push(step);
            parents.push(vec![]);
            steps.len() - 1
        };

        if let Some(reference) = &ir_run.name {
            if head_name.as_ref() == Some(reference) {
                head_refs.push(ref_table.len());
            }
            ref_table.push((reference.clone(), reachable));
            let n = push(&mut steps, &mut parents, IrStep::Ref(ref_table.len() - 1));
            run.push(n);
        }
        for commit in &ir_run.commits {
            if reachable {
                reachable_commits.push(commit.id);
            }
            for r in &commit.refs {
                ref_table.push((r.clone(), reachable));
                let n = push(&mut steps, &mut parents, IrStep::Ref(ref_table.len() - 1));
                if let Some(&previous) = run.last() {
                    parents[previous].push(n);
                }
                run.push(n);
            }
            commits.push((commit.id, commit.parent_ids.clone()));
            let n = push(&mut steps, &mut parents, IrStep::Commit(commits.len() - 1));
            commit_step.insert(commit.id, n);
            if let Some(&previous) = run.last() {
                parents[previous].push(n);
            }
            run.push(n);
        }
        if run.is_empty() {
            run.push(push(&mut steps, &mut parents, IrStep::None));
        }
        runs.push(run);
    }

    // The ranked edges land on each run's LAST step, pointing at the target run's FIRST step.
    let first_step_of_run: Vec<usize> = runs
        .iter()
        .map(|run| *run.first().expect("every run has a step"))
        .collect();
    for (ri, run) in runs.iter().enumerate() {
        let source = *run.last().expect("every run has a step");
        for &target in &ir.runs[ri].targets {
            parents[source].push(first_step_of_run[target]);
        }
    }

    // The fixup: flatten a commit's group parents in slot order; on disagreement with the
    // RAW parent list, rewire directly to present commits (groups lose their edges). The ws
    // commit and partially-traversed commits keep their segment wiring.
    let commit_ids: HashSet<gix::ObjectId> = commits.iter().map(|(id, _)| *id).collect();
    let flatten = |steps: &[IrStep], parents: &[Vec<usize>], start: usize| {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = parents[start].iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            match steps[n] {
                IrStep::Commit(c) => out.push(c),
                IrStep::Ref(_) | IrStep::None => {
                    stack.extend(parents[n].iter().rev().copied());
                }
            }
        }
        out
    };
    for (id, raw_parents) in &commits {
        if Some(*id) == ir.workspace_commit_id {
            continue;
        }
        let preserved =
            !raw_parents.is_empty() && raw_parents.iter().any(|p| !commit_ids.contains(p));
        if preserved {
            continue;
        }
        let n = commit_step[id];
        let flat_ids: Vec<gix::ObjectId> = flatten(&steps, &parents, n)
            .into_iter()
            .map(|c| commits[c].0)
            .collect();
        if flat_ids == *raw_parents {
            continue;
        }
        parents[n] = raw_parents
            .iter()
            .filter_map(|p| commit_step.get(p).copied())
            .collect();
    }

    // Positions from the (post-fixup, pre-strip) topology: descend first-edges for `on` and
    // below, ascend for the entering edges and the convergence signal.
    let mut incoming: Vec<Vec<(usize, usize)>> = vec![Vec::new(); steps.len()];
    for (child, slots) in parents.iter().enumerate() {
        for (slot, &parent) in slots.iter().enumerate() {
            incoming[parent].push((child, slot));
        }
    }
    let is_commit = |n: usize| matches!(steps[n], IrStep::Commit(_));
    let ref_steps: Vec<(usize, usize)> = steps
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
    for &(ref_step, _) in &ref_steps {
        let mut cursor = ref_step;
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
            if matches!(steps[next], IrStep::Ref(_)) && below.is_none() {
                below = Some(next);
            }
            cursor = next;
        }
        let Some(on) = on else {
            continue; // unborn: no stored position
        };
        let mut cursor = ref_step;
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
            ref_step,
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
    // and keep the ws commit's resolved CHAIN slots (one per chain, so empty chains
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
    let mut ws_chain_slots: Option<(gix::ObjectId, Vec<gix::ObjectId>)> = None;
    for (id, _) in &commits {
        let n = commit_step[id];
        let mut resolved = Vec::with_capacity(parents[n].len());
        for (slot, &parent) in parents[n].iter().enumerate() {
            match resolve(parent) {
                Some(pick) => {
                    let IrStep::Commit(c) = steps[pick] else {
                        unreachable!("resolve returns commits");
                    };
                    resolved.push(commits[c].0);
                }
                None => dropped.push((n, slot)),
            }
        }
        if Some(*id) == ir.workspace_commit_id {
            ws_chain_slots = Some((*id, resolved));
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

    // The stored shape: ref-table order, step handles translated to ref ordinals and commit
    // ids, entering edges sorted.
    let step_of_ref: HashMap<usize, usize> = ref_steps.iter().map(|&(n, r)| (r, n)).collect();
    let refs = ref_table
        .into_iter()
        .enumerate()
        .map(|(r, (name, reachable))| {
            let position = positions.get(&step_of_ref[&r]).map(|position| {
                let IrStep::Commit(c) = steps[position.on] else {
                    unreachable!("positions sit on commits");
                };
                let below = position.below.map(|b| {
                    let IrStep::Ref(br) = steps[b] else {
                        unreachable!("below entries are refs");
                    };
                    br
                });
                let mut entering: Vec<(gix::ObjectId, usize)> = position
                    .entering
                    .iter()
                    .map(|&(child, slot)| {
                        let IrStep::Commit(c) = steps[child] else {
                            unreachable!("entering edges come from commits");
                        };
                        (commits[c].0, slot)
                    })
                    .collect();
                entering.sort_unstable();
                RefPosition {
                    on: commits[c].0,
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
    RefPositions {
        refs,
        ws_chain_slots,
        head_refs,
        reachable_commits,
    }
}
