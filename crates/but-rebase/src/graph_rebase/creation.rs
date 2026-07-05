use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow, bail};
use but_core::{RefMetadata, commit::SignCommit};

use crate::graph_rebase::{
    Checkout, Editor, EditorGraph, EditorGraphIndex, Pick, RevisionHistory, Selector, Step,
    SuccessfulRebase,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References whose segment should be forced mutable.
    ///
    /// The editor always contains every segment in the workspace graph, with
    /// only those reachable from `HEAD` being mutable. Use this to force a
    /// segment that isn't reachable from `HEAD` to be mutable so it can be
    /// rewritten.
    pub extra_mutable_refs: Vec<gix::refs::FullName>,
}

impl Default for GraphEditorOptions {
    fn default() -> Self {
        Self {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            extra_mutable_refs: vec![],
        }
    }
}

/// Creates an editor out of the workspace graph.
impl<'ws, 'meta, M: RefMetadata> Editor<'ws, 'meta, M> {
    /// Creates an editor out of the workspace graph with the default options.
    pub fn create(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
    ) -> Result<Self> {
        Self::create_with_opts(workspace, meta, repo, &GraphEditorOptions::default())
    }

    /// Creates an editor out of the workspace graph with the specified options.
    pub fn create_with_opts(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<Self> {
        let (graph, references, checkouts) = create_native(workspace, repo, options)?;
        Ok(Self {
            graph,
            initial_references: references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            workspace,
            meta,
        })
    }
}

/// Build the editor graph by ADOPTION: the carried [`but_graph::CommitGraph`] is cloned
/// wholesale into the arena — full commit payloads (flags, refs, generation) survive, which
/// the write-through put-back depends on — then normalized to editor shape (every parent
/// slot present, the ws commit on its stack slots) and dressed with pick settings and the
/// reference positions derived from the segment graph.
///
/// The derivation mirrors the retired segment walk's semantics exactly, on a throwaway IR:
/// per-segment runs (segment ref, then per commit its refs then the commit), rank-ordered
/// inter-segment edges, the parent fixup (a commit whose chain-flattened parents disagree
/// with its raw parent list is rewired directly, bypassing chains — the ws commit and
/// partially-traversed commits keep their wiring), position derivation, and the strip's
/// slot compaction.
fn create_native(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
    options: &GraphEditorOptions,
) -> Result<(EditorGraph, Vec<gix::refs::FullName>, Vec<Checkout>)> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum IrStep {
        /// Index into `ref_table`.
        Ref(usize),
        /// Index into `commit_table`.
        Commit(usize),
        /// The placeholder for a segment with neither name nor commits.
        None,
    }

    let Some(cg) = workspace.graph.commit_graph() else {
        bail!("native creation requires the graph to carry its CommitGraph");
    };
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
    // editor's ref order (and with it render sibling order).
    let mut nodes: Vec<IrStep> = Vec::new();
    let mut parents: Vec<Vec<usize>> = Vec::new();
    let mut ref_table: Vec<(gix::refs::FullName, bool)> = Vec::new();
    let mut commit_table: Vec<(gix::ObjectId, Vec<gix::ObjectId>)> = Vec::new();
    let mut commit_node = HashMap::<gix::ObjectId, usize>::new();
    let mut mutable_commits = HashSet::new();
    let mut head_ref_ordinals = Vec::new();
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
                head_ref_ordinals.push(ref_table.len());
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
    // in the source commit's parent array, commit-less edges after them in edge order, ranks
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
    // RAW parent list, rewire directly to present commits (chains lose their edges). The ws
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
    // unborn chains), record the vacated slots so the captured entering edges can be renamed,
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
    let mut ws_parents: Option<Vec<gix::ObjectId>> = None;
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
            ws_parents = Some(resolved);
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

    // A parent outside the graph means the traversal was partial here: the editor's slots
    // must all be present, so those slots are dropped — the raw parent list is preserved so
    // the rebase keeps the commit's real ancestry.
    let traversal_was_partial_at = |id: gix::ObjectId| {
        let raw_parents = &cg.node(id).expect("iterating graph ids").commit.parent_ids;
        (!raw_parents.is_empty() && raw_parents.iter().any(|p| cg.node(*p).is_none()))
            .then(|| raw_parents.clone())
    };
    let node_of = |id: gix::ObjectId| -> Result<EditorGraphIndex> {
        cg.index_of(id)
            .map(EditorGraphIndex::Node)
            .ok_or_else(|| anyhow!("derived position {id} is not a commit in the graph"))
    };

    let mut arena = cg.clone();
    for (i, id) in cg.commit_ids().enumerate() {
        // The ws commit takes its STACK slots from the derivation (dups and all); everything
        // else keeps the PRESENT parents in slot order — the same presence filter the old
        // segment walk's parent-fixup pass applied.
        if workspace_commit_id == Some(id) {
            let mut targets = Vec::new();
            for parent in ws_parents.as_deref().unwrap_or_default() {
                let Some(target) = cg.index_of(*parent) else {
                    bail!("derived ws parent {parent} is not a commit in the graph");
                };
                targets.push(target);
            }
            arena.set_parents(i, targets);
        } else if traversal_was_partial_at(id).is_some() {
            arena.set_parents(i, cg.present_parent_indices(i));
        }
    }

    let mut editor_graph = EditorGraph::adopt(arena);
    for (i, id) in cg.commit_ids().enumerate() {
        let ix = EditorGraphIndex::Node(i);
        let mut pick = if workspace_commit_id == Some(id) {
            Pick::new_workspace_pick(id)
        } else {
            let mut pick = Pick::new_pick(id);
            pick.sign_commit = options.default_sign_commit;
            pick
        };
        pick.mutable = mutable_commits.contains(&id);
        editor_graph.set_step(ix, Step::Pick(pick));
        if let Some(raw_parents) = traversal_was_partial_at(id) {
            editor_graph.set_preserved_parents(ix, Some(raw_parents));
        }
    }

    // Two passes: refs stack top-down in the derivation (a ref's `below` has a HIGHER
    // ordinal), so every node must exist before positions can name it.
    let ref_ixs: Vec<EditorGraphIndex> = ref_table
        .iter()
        .map(|(name, mutable)| editor_graph.add_reference(name.clone(), *mutable))
        .collect();
    let node_of_ref: HashMap<usize, usize> = ref_nodes.iter().map(|&(n, r)| (r, n)).collect();
    for r in 0..ref_table.len() {
        // Unborn refs (no position) keep no stored position.
        let Some(position) = positions.get(&node_of_ref[&r]) else {
            continue;
        };
        let IrStep::Commit(c) = nodes[position.on] else {
            unreachable!("positions sit on commits");
        };
        let on = node_of(commit_table[c].0)?;
        let below = position.below.map(|b| {
            let IrStep::Ref(br) = nodes[b] else {
                unreachable!("below entries are refs");
            };
            ref_ixs[br]
        });
        let mut edges = Vec::with_capacity(position.entering.len());
        for &(child, slot) in &position.entering {
            let IrStep::Commit(c) = nodes[child] else {
                unreachable!("entering edges come from commits");
            };
            edges.push((commit_table[c].0, slot));
        }
        edges.sort_unstable();
        let entering = edges
            .into_iter()
            .map(|(id, slot)| Ok((node_of(id)?, slot)))
            .collect::<Result<Vec<_>>>()?;
        editor_graph.set_position(ref_ixs[r], on, &entering, position.ambiguous, below);
    }

    let references = ref_table
        .iter()
        .filter(|(_, mutable)| *mutable)
        .map(|(name, _)| name.clone())
        .collect();
    let checkouts = head_ref_ordinals
        .into_iter()
        .map(|r| Checkout::Head {
            selector: Selector { id: ref_ixs[r] },
            merge_base_override: None,
        })
        .collect();

    crate::graph_rebase::positions::debug_assert_positions_total(&editor_graph);
    Ok((editor_graph, references, checkouts))
}

impl<'ws, 'meta, M: RefMetadata> SuccessfulRebase<'ws, 'meta, M> {
    /// Converts a SuccessfulRebase back into another editor for multi-step operations.
    ///
    /// This is the normalization path for callers that want to chain
    /// additional editor-based operations and need the editor graph plus
    /// in-memory repository to agree on ancestry.
    pub fn into_editor(self) -> Editor<'ws, 'meta, M> {
        Editor {
            graph: self.graph,
            initial_references: self.initial_references,
            checkouts: self.checkouts,
            repo: self.repo,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        }
    }
}
