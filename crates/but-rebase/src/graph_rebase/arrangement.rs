//! The name-keyed arrangement table — the seed of the position model's end-state.
//!
//! Everything a [`RefPosition`](crate::graph_rebase::editor_graph::RefPosition) records is keyed
//! by graph coordinates (node ids, parent slots) that churn under mutation, which is why positions
//! need incremental maintenance (`rename_edges`, `apply_group_join`, the preserve-vs-reclassify
//! flag). The intended replacement keys the same information by REF NAMES, which mutation never
//! churns: per pick, an ordered list of groups, each an ordered list of ref names — exactly
//! the shape of workspace metadata (stack order, branch order). Pick, rank, and entering edges
//! then become DERIVED, projection-style, from the table + live pick edges.
//!
//! This module provides the OP API ([`place_ref`] and friends): mutation sites speak position
//! INTENTS ([`StackSlot`]) instead of authoring `(on, below, group)` triples by hand. Implemented
//! atop the stored positions today; when the store swaps to the name-keyed table, only these
//! ops' internals change. (An env-gated corpus census proved the swap viable — every stored
//! position round-trips through the name-keyed table with zero divergences corpus-wide.)

use crate::graph_rebase::editor_graph::{Edge, RefPosition};
use crate::graph_rebase::positions;
use crate::graph_rebase::{EditorGraph, EditorGraphIndex};

/// A position in a commit's reference stack, named by intent.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StackSlot {
    /// Directly above this reference in its group: mates that sat on it re-hang onto the
    /// newcomer.
    Above(EditorGraphIndex),
    /// At this reference's position: the newcomer takes its below, and it re-hangs onto the
    /// newcomer.
    Below(EditorGraphIndex),
    /// The bottom of the pick's whole stack (carrying all its edges — "the branch here");
    /// every reference that sat on the pick itself re-hangs onto the newcomer.
    Bottom(EditorGraphIndex),
    /// The top of the group the edge `(child, parent-slot)` carries into `pick`.
    GroupTop {
        /// The commit the group sits on.
        pick: EditorGraphIndex,
        /// The child edge whose group the reference stacks onto.
        edge: (EditorGraphIndex, usize),
    },
    /// A fresh root above `pick`: nothing descends into it, no other position moves.
    Root(EditorGraphIndex),
}

/// Place the reference at `node` into `slot`, shifting other positions as the slot demands.
/// The node must not currently occupy a position that should move with it (this is the FRESH
/// placement op; moving an existing reference is a different intent).
pub(crate) fn place_ref(graph: &mut EditorGraph, node: EditorGraphIndex, slot: StackSlot) {
    match slot {
        StackSlot::Above(target) => {
            if graph.position_of(target).is_none() {
                return;
            }
            // Same-group members that sat directly on the target now sit on the
            // interposed node; cross-group members on the target keep it (they branch).
            let rehang: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|(mate, m)| *mate != node && m.below == Some(target))
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            graph.join_group_of(node, target, Some(target));
        }
        StackSlot::Below(target) => {
            let Some(stored) = graph.position_of(target) else {
                return;
            };
            let below = stored.below;
            graph.join_group_of(node, target, below);
            graph.set_below(target, Some(node));
        }
        StackSlot::Bottom(pick) => {
            let entering = positions::edges_into(graph, pick);
            // Members that sat on the pick itself now sit on the new bottom.
            let rehang: Vec<_> = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && stored.below.is_none()
                        && positions::resolve_to_pick(graph, stored.on) == Some(pick)
                })
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            graph.set_position(node, pick, &entering, entering.len() > 1, None);
        }
        StackSlot::GroupTop { pick, edge } => {
            let entering = vec![edge];
            let top = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && positions::resolve_to_pick(graph, stored.on) == Some(pick)
                        && positions::edges_through(graph, *mate) == entering
                })
                .map(|(mate, _)| mate)
                .max_by_key(|&mate| (positions::ref_depth(graph, mate), mate));
            graph.set_position(node, pick, &entering, false, top);
        }
        StackSlot::Root(pick) => {
            graph.set_position(node, pick, &[], false, None);
        }
    }
}

/// Move the reference at `node` into `slot`, taking the edges that enter through it along.
///
/// Unlike [`place_ref`] (fresh placement), the reference already holds a position: the edges
/// that entered through it follow it when it is their sole carrier (they re-point onto the new
/// pick and merge into the slot's entering set), and — when moving above another reference —
/// the group members now entered through the moved reference share the merged entry set.
pub(crate) fn move_ref(graph: &mut EditorGraph, node: EditorGraphIndex, slot: StackSlot) {
    let Some(moving) = graph.position_of(node) else {
        return;
    };
    let moving_edges = positions::edges_through(graph, node);
    // Sole carrier = nothing in the group sits below the mover. Measured before any shuffling
    // (the shuffles never change which members those are).
    let moving_depth = positions::ref_depth(graph, node);
    let sole_carrier = !positions::group_members(graph, node)
        .into_iter()
        .any(|(mate, _)| mate != node && positions::ref_depth(graph, mate) < moving_depth);
    // The mover vacates its old spot: members stacked directly on it settle onto what it sat
    // on.
    graph.splice(node);
    // Each slot yields the new position as (on, entering, below); the moving reference's
    // edges are merged into the entering set below and the whole thing classified once (all
    // carried edges are re-pointed by then, so the final `set_position` sees the complete
    // set). Each arm hangs the mover on its new below FIRST, so re-hanging mates onto it
    // never leaves a transient below-cycle through its stale pointer.
    let (on, mut entering, below) = match slot {
        StackSlot::Above(target) => {
            let Some(t_stored) = graph.position_of(target) else {
                return;
            };
            graph.set_below(node, Some(target));
            // Same-group members that sat directly on the target now sit on the mover.
            let rehang: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|(mate, m)| *mate != node && m.below == Some(target))
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            (
                t_stored.on,
                positions::edges_through(graph, target),
                Some(target),
            )
        }
        StackSlot::Below(target) => {
            let Some(t_stored) = graph.position_of(target) else {
                return;
            };
            graph.set_below(node, t_stored.below);
            graph.set_below(target, Some(node));
            (
                t_stored.on,
                positions::edges_through(graph, target),
                t_stored.below,
            )
        }
        StackSlot::Bottom(pick) => {
            // The bottom position at the pick; refs that sat on the pick itself re-hang onto
            // the mover. Only the moved reference's own edges enter through it there.
            graph.set_below(node, None);
            let rehang: Vec<_> = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && stored.below.is_none()
                        && positions::resolve_to_pick(graph, stored.on) == Some(pick)
                })
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            (pick, Vec::new(), None)
        }
        StackSlot::GroupTop { pick, edge } => {
            let entering = vec![edge];
            let top = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && positions::resolve_to_pick(graph, stored.on) == Some(pick)
                        && positions::edges_through(graph, *mate) == entering
                })
                .map(|(mate, _)| mate)
                .max_by_key(|&mate| (positions::ref_depth(graph, mate), mate));
            graph.set_below(node, top);
            (pick, entering, top)
        }
        StackSlot::Root(pick) => {
            graph.set_below(node, None);
            (pick, Vec::new(), None)
        }
    };
    // The edges that entered through the reference follow it (node-era edges pointed at the
    // reference itself), entering the group at its new position — but only when it was their
    // sole carrier: group members staying behind keep their entering edges.
    let old_resolved = positions::resolve_to_pick(graph, moving.on);
    let new_resolved = positions::resolve_to_pick(graph, on);
    if sole_carrier && let (Some(old_pick), Some(new_pick)) = (old_resolved, new_resolved) {
        redirect_edges(graph, &moving_edges, old_pick, new_pick);
        for &edge in &moving_edges {
            if !entering.contains(&edge) {
                entering.push(edge);
            }
        }
        entering.sort();
        // Members below in the joined group are now entered through the moved reference:
        // they share the merged entry set.
        if let StackSlot::Above(target) = slot
            && graph.position_of(target).is_some()
        {
            let t_depth = positions::ref_depth(graph, target);
            let mates: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|(mate, _)| *mate != node && positions::ref_depth(graph, *mate) <= t_depth)
                .collect();
            for (mate, m) in mates {
                graph.set_position(mate, m.on, &entering, entering.len() > 1, m.below);
            }
        }
    }
    graph.set_position(node, on, &entering, entering.len() > 1, below);
}

/// Re-point the captured `edges` from `from` onto `to`: each edge keeps its slot, so its
/// group statement follows the name. Edges already rewired elsewhere are left alone.
pub(crate) fn redirect_edges(
    graph: &mut EditorGraph,
    edges: &[Edge],
    from: EditorGraphIndex,
    to: EditorGraphIndex,
) {
    if from == to {
        return;
    }
    for &(child, slot) in edges {
        if graph.parents(child).get(slot) == Some(&from) {
            graph.replace_parent(child, slot, to);
        }
    }
}

/// The member holding the depth directly below `depth` on `on` (resolved), excluding
/// `exclude` — the mate a landing reference at `depth` sits on, lowest node id on a tie.
fn mate_below_depth(
    graph: &EditorGraph,
    exclude: EditorGraphIndex,
    on: EditorGraphIndex,
    depth: usize,
) -> Option<EditorGraphIndex> {
    if depth == 0 {
        return None;
    }
    let pick = positions::resolve_to_pick(graph, on)?;
    graph
        .positioned_refs()
        .filter(|(mate, stored)| {
            *mate != exclude
                && positions::resolve_to_pick(graph, stored.on) == Some(pick)
                && positions::ref_depth(graph, *mate) + 1 == depth
        })
        .map(|(mate, _)| mate)
        .min()
}

/// Re-point the reference at `node` at the commit `onto` — `git update-ref`, spoken as
/// a position move. The edges that entered through it follow it (they re-point onto the new pick),
/// group members stacked above move with it, and members below lose their entering edges (they
/// become roots at the old pick). An unplaced reference is placed as a fresh root; a
/// reference already resolving there just refreshes its stored `on`.
pub(crate) fn repoint_ref(graph: &mut EditorGraph, node: EditorGraphIndex, onto: EditorGraphIndex) {
    let Some(stored) = graph.position_of(node) else {
        place_ref(graph, node, StackSlot::Root(onto));
        return;
    };
    match positions::resolve_to_pick(graph, stored.on) {
        Some(old_pick) if old_pick != onto => {
            // Snapshot the reference's entering edges before re-pointing them (the derived read
            // tracks live edges), so it can be re-placed against `onto`'s final edges.
            let entering = positions::edges_through(graph, node);
            redirect_edges(graph, &entering, old_pick, onto);
            // Its old below stays behind; at the destination the reference sits on whatever
            // holds the depth below it there — or lands directly on the pick when that stack
            // doesn't exist (its carried mates follow through their below walks).
            let below = mate_below_depth(graph, node, onto, positions::ref_depth(graph, node));
            // Carried = the below-subtree stacked on the reference. Depth-tied siblings and the
            // below walk underneath are NOT carried — they stay at the old pick, though group
            // mates lose their entering edges (they move with `node`) and become roots there.
            let mut carried = vec![node];
            let mut i = 0;
            while i < carried.len() {
                let current = carried[i];
                i += 1;
                let dependents: Vec<_> = graph
                    .positioned_refs()
                    .filter(|(mate, member)| {
                        member.below == Some(current)
                            && !carried.contains(mate)
                            && graph.is_reference(*mate)
                    })
                    .map(|(mate, _)| mate)
                    .collect();
                carried.extend(dependents);
            }
            let mates: Vec<_> = positions::group_members(graph, node)
                .into_iter()
                .filter(|(mate, _)| *mate != node && !carried.contains(mate))
                .collect();
            for (mate, member) in mates {
                graph.set_position(mate, member.on, &[], false, member.below);
            }
            for &mate in &carried[1..] {
                graph.rekey_position(mate, onto);
            }
            // The reference's edges moved with it; re-classify its group against `onto`'s
            // final edges (its old `Edges` statement may not exist there).
            graph.set_position(node, onto, &entering, stored.ambiguous, below);
        }
        _ => {
            graph.rekey_position(node, onto);
        }
    }
}

/// Remove the reference at `node` from its group: members above close the gap and the
/// reference becomes a root at its current pick — nothing descends into it any more. With
/// `drop_edges` the pick edges that entered through its position are removed outright; otherwise
/// they stay on the pick for a follow-up reconnect to rewire.
pub(crate) fn unhook_ref(graph: &mut EditorGraph, node: EditorGraphIndex, drop_edges: bool) {
    let Some(unhooked) = graph.position_of(node) else {
        return;
    };
    // The group closes past the unhooked reference: mates that sat on it settle onto what it
    // sat on, becoming its sibling branch (the unhooked ref keeps its spot).
    let rehang: Vec<_> = positions::group_members(graph, node)
        .into_iter()
        .filter(|(mate, m)| *mate != node && m.below == Some(node))
        .map(|(mate, _)| mate)
        .collect();
    for mate in rehang {
        graph.set_below(mate, unhooked.below);
    }
    if drop_edges && let Some(pick) = positions::resolve_to_pick(graph, unhooked.on) {
        let mut edges = positions::edges_through(graph, node);
        edges.sort_unstable();
        // Descending slots per child: a removal shifts only the slots above it, so every
        // pending `(child, slot)` name below stays exact.
        for (child, slot) in edges.into_iter().rev() {
            if graph.parents(child).get(slot) == Some(&pick) {
                graph.remove_parent(child, slot);
            }
        }
    }
    graph.set_position(node, unhooked.on, &[], false, unhooked.below);
}

/// Move the stack slice led by `lead_ref` — it and its below-subtree in its group on
/// `source_pick` — onto `dest`: the lead lands at the bottom, each member is
/// re-classified against its own edges at the destination (they come along), and stored
/// ambiguity is preserved.
pub(crate) fn transfer_stack(
    graph: &mut EditorGraph,
    lead_ref: EditorGraphIndex,
    source_pick: EditorGraphIndex,
    dest: EditorGraphIndex,
) {
    if graph.position_of(lead_ref).is_none() {
        return;
    }
    let lead_entering = positions::edges_through(graph, lead_ref);
    let mut moves = vec![lead_ref];
    let mut i = 0;
    while i < moves.len() {
        let current = moves[i];
        i += 1;
        let dependents: Vec<_> = graph
            .positioned_refs()
            .filter(|(node, stored)| {
                stored.below == Some(current)
                    && !moves.contains(node)
                    && positions::resolve_to_pick(graph, stored.on) == Some(source_pick)
                    && positions::edges_through(graph, *node) == lead_entering
            })
            .map(|(node, _)| node)
            .collect();
        moves.extend(dependents);
    }
    for node in moves {
        let Some(stored) = graph.position_of(node) else {
            continue;
        };
        let entering = positions::edges_through(graph, node);
        // The lead lands at the bottom of the destination (its old below stays behind);
        // the rest of the slice keeps its internal stacking.
        let below = (node != lead_ref).then_some(stored.below).flatten();
        graph.set_position(node, dest, &entering, stored.ambiguous, below);
    }
}

/// Carry the slice of the group identified by `entering` on `source_pick` strictly above depth `above_depth` onto
/// `dest` verbatim — same depths, same kinds; only the `on` key changes. The
/// delimiter position below the slice stays behind. `entering`/`above_depth` are caller-captured
/// (pre-mutation) coordinates rather than live derivations.
pub(crate) fn carry_stack_above(
    graph: &mut EditorGraph,
    source_pick: EditorGraphIndex,
    entering: &[(EditorGraphIndex, usize)],
    above_depth: usize,
    dest: EditorGraphIndex,
) {
    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|(node, stored)| {
            positions::resolve_to_pick(graph, stored.on) == Some(source_pick)
                && positions::edges_through(graph, *node) == entering
                && positions::ref_depth(graph, *node) > above_depth
        })
        .map(|(node, _)| node)
        .collect();
    for &node in &moves {
        graph.rekey_position(node, dest);
    }
    // The slice bottom sat on the delimiter left behind; at the destination (depths carried
    // verbatim) it sits on whatever holds the depth below it there.
    for &node in &moves {
        if let Some(stored) = graph.position_of(node)
            && stored.below.is_some_and(|b| !moves.contains(&b))
        {
            let mate = mate_below_depth(graph, node, dest, positions::ref_depth(graph, node));
            graph.set_below(node, mate);
        }
    }
}

/// Stack every reference on `source_pick` above `top` (a reference on another pick), the
/// whole tower re-placed behind `bridge_node`'s full incoming edge set — the bridged edges
/// that now descend into the joined group. Returns false (leaving the graph untouched) when
/// `top` holds no position.
pub(crate) fn land_stack_above(
    graph: &mut EditorGraph,
    source_pick: EditorGraphIndex,
    top: EditorGraphIndex,
    bridge_node: EditorGraphIndex,
) -> bool {
    let Some(top_stored) = graph.position_of(top) else {
        return false;
    };
    let bridge = positions::edges_into(graph, bridge_node);
    let top_depth = positions::ref_depth(graph, top);
    graph.set_position(
        top,
        top_stored.on,
        &bridge,
        bridge.len() > 1,
        top_stored.below,
    );

    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|(_, stored)| positions::resolve_to_pick(graph, stored.on) == Some(source_pick))
        .map(|(node, stored)| (node, positions::ref_depth(graph, node), stored.below))
        .collect();
    for (node, depth, below) in moves {
        // The tower's internal stacking is preserved; its bottom members (they sat on the
        // source pick) now sit on whatever holds the depth below their landing spot — `top`
        // itself when it lives on the bridge node, its stand-in there otherwise.
        let below =
            below.or_else(|| mate_below_depth(graph, node, bridge_node, depth + top_depth + 1));
        graph.set_position(node, bridge_node, &bridge, bridge.len() > 1, below);
    }
    true
}

/// Re-key every reference whose `on` no longer resolves (it sat on removed picks) onto
/// `onto`, positions carried verbatim — the ruled dangling semantics: the position
/// follows where the commit's place went, the entering edges stay.
pub(crate) fn readopt_dangling_refs(graph: &mut EditorGraph, onto: EditorGraphIndex) {
    let dangling: Vec<_> = graph
        .positioned_refs()
        .filter(|(_, stored)| positions::resolve_to_pick(graph, stored.on).is_none())
        .collect();
    for (node, _) in dangling {
        graph.rekey_position(node, onto);
    }
}

/// Which side of `at_ref` a group split leaves with the lower part.
#[derive(Clone, Copy)]
pub(crate) enum SplitBoundary {
    /// Members strictly above the ref move up; the ref stays with the lower part.
    Above,
    /// The ref and members above it move up; only members below stay.
    At,
}

/// The result of splitting a group around an interposed pick.
pub(crate) struct GroupSplit {
    /// The members left behind, with their pre-split positions — settle them with
    /// [`settle_group_lower`] once the edge entering the lower part is known.
    pub lower: Vec<(EditorGraphIndex, RefPosition)>,
    /// Whether any member moved onto the upper node. When none did, `at_ref` was the top
    /// of its group, so the group's carried edges belong to the caller's new pick.
    pub moved_any: bool,
}

/// Split the group at `at_ref` around a pick interposed into it: members on the upper side
/// of `boundary` re-key onto `upper` with the boundary member landing at the bottom
/// (carry kinds carried verbatim), and the lower members are returned untouched for the
/// caller to settle.
pub(crate) fn split_group(
    graph: &mut EditorGraph,
    at_ref: EditorGraphIndex,
    boundary: SplitBoundary,
    upper: EditorGraphIndex,
) -> GroupSplit {
    if graph.position_of(at_ref).is_none() {
        return GroupSplit {
            lower: Vec::new(),
            moved_any: false,
        };
    }
    let members = positions::group_members(graph, at_ref);
    // The upper side is the below-subtree on the moving side of the boundary — depth-tied
    // siblings from other stacks can share the group's entering edges but hang elsewhere and stay.
    let mut moved = vec![at_ref];
    let mut i = 0;
    while i < moved.len() {
        let current = moved[i];
        i += 1;
        let dependents: Vec<_> = members
            .iter()
            .filter(|(node, m)| m.below == Some(current) && !moved.contains(node))
            .map(|(node, _)| *node)
            .collect();
        moved.extend(dependents);
    }
    if matches!(boundary, SplitBoundary::Above) {
        moved.remove(0);
    }
    let mut lower = Vec::new();
    let mut boundary_below = None;
    for (node, member) in members {
        if moved.contains(&node) {
            graph.rekey_position(node, upper);
            if member.below.is_none_or(|b| !moved.contains(&b)) {
                // The boundary member lands at the bottom of the upper node; its old
                // below stays on the lower side of the split.
                boundary_below = member.below;
                graph.set_below(node, None);
            }
        } else {
            lower.push((node, member));
        }
    }
    // References stacked on the moved slice but not moving with it (cross-group roots, e.g. a
    // remote above the moved tip) settle onto what the slice sat on: they and everything on
    // top of them close the gap by construction.
    let stranded: Vec<_> = graph
        .positioned_refs()
        .filter(|(node, s)| !moved.contains(node) && s.below.is_some_and(|b| moved.contains(&b)))
        .map(|(node, _)| node)
        .collect();
    for node in stranded {
        graph.set_below(node, boundary_below);
    }
    GroupSplit {
        lower,
        moved_any: !moved.is_empty(),
    }
}

/// Settle the lower part of a split group: each member keeps its `on` and stacking but is
/// now entered through `edge` — the edge descending from the interposed pick.
pub(crate) fn settle_group_lower(
    graph: &mut EditorGraph,
    lower: &[(EditorGraphIndex, RefPosition)],
    edge: (EditorGraphIndex, usize),
) {
    for (node, member) in lower {
        graph.set_position(*node, member.on, &[edge], false, member.below);
    }
}
