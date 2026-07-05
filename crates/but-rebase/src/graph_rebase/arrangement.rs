//! The name-keyed arrangement table — the seed of the position model's end-state.
//!
//! Everything a [`RefPosition`](crate::graph_rebase::step_graph::RefPosition) records is keyed
//! by graph coordinates (node ids, parent slots) that churn under mutation, which is why positions
//! need incremental maintenance (`rewrite_approach_leg`, `apply_chain_join`, the preserve-vs-reclassify
//! flag). The intended replacement keys the same information by REF NAMES, which mutation never
//! churns: per anchor commit, an ordered list of lanes, each an ordered list of ref names — exactly
//! the shape of workspace metadata (stack order, branch order). Anchor, rank, and approach then
//! become DERIVED, projection-style, from the table + live pick edges.
//!
//! This module provides two things:
//!
//! 1. The OP API ([`place_ref`] and friends): mutation sites speak position INTENTS
//!    ([`StackSlot`]) instead of authoring `(anchor, below, lane)` triples by hand. Implemented
//!    atop the stored anchors today; when the store swaps to the name-keyed table, only these
//!    ops' internals change.
//! 2. A corpus census (env `BUT_ARRANGE_CENSUS`, called from `debug_assert_positions_total`):
//!    extract the table from today's stored positions, re-derive every position from it, and
//!    compare. Divergences enumerate precisely where the name-keyed model needs a better rule —
//!    or where information is genuinely not order-derivable. Verdict so far: zero divergences
//!    corpus-wide.

use std::collections::HashMap;

use crate::graph_rebase::positions::{self, legs_into_pick};
use crate::graph_rebase::step_graph::{LaneCarry, RefPosition};
use crate::graph_rebase::{StepGraph, StepGraphIndex};

/// A position in a commit's reference stack, named by intent.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StackSlot {
    /// Directly above this reference in its chain: mates that sat on it re-hang onto the
    /// newcomer.
    Above(StepGraphIndex),
    /// At this reference's position: the newcomer takes its below, and it re-hangs onto the
    /// newcomer.
    Below(StepGraphIndex),
    /// The bottom of the pick's whole stack (carrying all its legs — "the branch here");
    /// every reference that sat on the pick itself re-hangs onto the newcomer.
    Bottom(StepGraphIndex),
    /// The top of the chain the leg `(child, parent-slot)` carries into `pick`.
    LaneTop {
        /// The commit the lane's chain is anchored on.
        pick: StepGraphIndex,
        /// The child edge whose lane the reference stacks onto.
        leg: (StepGraphIndex, usize),
    },
    /// A fresh root above `pick`: nothing descends into it, no other position moves.
    Root(StepGraphIndex),
}

/// Place the reference at `node` into `slot`, shifting other positions as the slot demands.
/// The node must not currently occupy a position that should move with it (this is the FRESH
/// placement op; moving an existing reference is a different intent).
pub(crate) fn place_ref(graph: &mut StepGraph, node: StepGraphIndex, slot: StackSlot) {
    match slot {
        StackSlot::Above(target) => {
            if graph.position_of(target).is_none() {
                return;
            }
            // Same-lane members that sat directly on the target now sit on the
            // interposed node; cross-lane members on the target keep it (they branch).
            let rehang: Vec<_> = positions::chain_members(graph, target)
                .into_iter()
                .filter(|(mate, m)| *mate != node && m.below == Some(target))
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            graph.join_lane_of(node, target, Some(target));
        }
        StackSlot::Below(target) => {
            let Some(stored) = graph.position_of(target) else {
                return;
            };
            let below = stored.below;
            graph.join_lane_of(node, target, below);
            graph.set_below(target, Some(node));
        }
        StackSlot::Bottom(pick) => {
            let approach = positions::legs_into_pick(graph, pick);
            // Members that sat on the pick itself now sit on the new bottom.
            let rehang: Vec<_> = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && stored.below.is_none()
                        && positions::resolve_to_pick(graph, stored.anchor) == Some(pick)
                })
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            graph.set_position(node, pick, &approach, approach.len() > 1, None);
        }
        StackSlot::LaneTop { pick, leg } => {
            let approach = vec![leg];
            let top = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && positions::resolve_to_pick(graph, stored.anchor) == Some(pick)
                        && positions::ref_approach(graph, *mate) == approach
                })
                .map(|(mate, _)| mate)
                .max_by_key(|&mate| (positions::ref_depth(graph, mate), mate));
            graph.set_position(node, pick, &approach, false, top);
        }
        StackSlot::Root(pick) => {
            graph.set_position(node, pick, &[], false, None);
        }
    }
}

/// Move the reference at `node` into `slot`, taking its approaching legs along.
///
/// Unlike [`place_ref`] (fresh placement), the reference already holds a position: the legs
/// that approached it follow it when it is their sole carrier (their edges move onto the new
/// anchor and merge into the slot's approach), and — when moving above another reference —
/// the chain members now entered through the moved reference share the merged approach.
pub(crate) fn move_ref(graph: &mut StepGraph, node: StepGraphIndex, slot: StackSlot) {
    let Some(moving) = graph.position_of(node) else {
        return;
    };
    let moving_approach = positions::ref_approach(graph, node);
    // Sole carrier = nothing in the chain sits below the mover. Measured before any shuffling
    // (the shuffles never change which members those are).
    let moving_depth = positions::ref_depth(graph, node);
    let sole_carrier = !positions::chain_members(graph, node)
        .into_iter()
        .any(|(mate, _)| mate != node && positions::ref_depth(graph, mate) < moving_depth);
    // The mover vacates its old spot: members stacked directly on it settle onto what it sat
    // on.
    splice_out(graph, node, moving.below);
    // Each slot yields the new position as (anchor, approach, below); the moving reference's
    // legs are merged into the approach below and the whole thing classified once (all leg
    // edges are moved by then, so `place` sees the complete legs). Each arm hangs the mover
    // on its new below FIRST, so re-hanging mates onto it never leaves a transient below-cycle
    // through its stale pointer.
    let (anchor, mut approach, below) = match slot {
        StackSlot::Above(target) => {
            let Some(t_stored) = graph.position_of(target) else {
                return;
            };
            graph.set_below(node, Some(target));
            // Same-lane members that sat directly on the target now sit on the mover.
            let rehang: Vec<_> = positions::chain_members(graph, target)
                .into_iter()
                .filter(|(mate, m)| *mate != node && m.below == Some(target))
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            (
                t_stored.anchor,
                positions::ref_approach(graph, target),
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
                t_stored.anchor,
                positions::ref_approach(graph, target),
                t_stored.below,
            )
        }
        StackSlot::Bottom(pick) => {
            // The bottom position at the pick; refs that sat on the pick itself re-hang onto
            // the mover. Only the moved reference's own legs approach it there.
            graph.set_below(node, None);
            let rehang: Vec<_> = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && stored.below.is_none()
                        && positions::resolve_to_pick(graph, stored.anchor) == Some(pick)
                })
                .map(|(mate, _)| mate)
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(node));
            }
            (pick, Vec::new(), None)
        }
        StackSlot::LaneTop { pick, leg } => {
            let approach = vec![leg];
            let top = graph
                .positioned_refs()
                .filter(|(mate, stored)| {
                    *mate != node
                        && positions::resolve_to_pick(graph, stored.anchor) == Some(pick)
                        && positions::ref_approach(graph, *mate) == approach
                })
                .map(|(mate, _)| mate)
                .max_by_key(|&mate| (positions::ref_depth(graph, mate), mate));
            graph.set_below(node, top);
            (pick, approach, top)
        }
        StackSlot::Root(pick) => {
            graph.set_below(node, None);
            (pick, Vec::new(), None)
        }
    };
    // The legs that approached the reference follow it (node-era edges pointed at the
    // reference itself), entering the chain at its new position — but only when it was their
    // sole carrier: chain members staying behind keep their approach.
    let old_anchor_pick = positions::resolve_to_pick(graph, moving.anchor);
    let new_anchor_pick = positions::resolve_to_pick(graph, anchor);
    if sole_carrier && let (Some(old_pick), Some(new_pick)) = (old_anchor_pick, new_anchor_pick) {
        for &(leg, leg_slot) in &moving_approach {
            if old_pick != new_pick && graph.parents(leg).get(leg_slot) == Some(&old_pick) {
                graph.replace_parent(leg, leg_slot, new_pick);
            }
            if !approach.contains(&(leg, leg_slot)) {
                approach.push((leg, leg_slot));
            }
        }
        approach.sort();
        // Members below in the joined chain are now approached through the moved reference:
        // they share the merged entry set.
        if let StackSlot::Above(target) = slot
            && graph.position_of(target).is_some()
        {
            let t_depth = positions::ref_depth(graph, target);
            let mates: Vec<_> = positions::chain_members(graph, target)
                .into_iter()
                .filter(|(mate, _)| *mate != node && positions::ref_depth(graph, *mate) <= t_depth)
                .collect();
            for (mate, m) in mates {
                graph.set_position(mate, m.anchor, &approach, approach.len() > 1, m.below);
            }
        }
    }
    graph.set_position(node, anchor, &approach, approach.len() > 1, below);
}

/// Splice `node` out of its physical stack: members sitting directly on it re-hang onto
/// `onto` (what it sat on). Everything above closes the gap by construction — depth is
/// derived from the below-chain.
pub(crate) fn splice_out(
    graph: &mut StepGraph,
    node: StepGraphIndex,
    onto: Option<StepGraphIndex>,
) {
    let dependents: Vec<_> = graph
        .positioned_refs()
        .filter(|(mate, stored)| *mate != node && stored.below == Some(node))
        .map(|(mate, _)| mate)
        .collect();
    for mate in dependents {
        graph.set_below(mate, onto);
    }
}

/// The member holding the depth directly below `depth` on `anchor` (resolved), excluding
/// `exclude` — the mate a landing reference at `depth` sits on, lowest node id on a tie.
fn mate_below_depth(
    graph: &StepGraph,
    exclude: StepGraphIndex,
    anchor: StepGraphIndex,
    depth: usize,
) -> Option<StepGraphIndex> {
    if depth == 0 {
        return None;
    }
    let pick = positions::resolve_to_pick(graph, anchor)?;
    graph
        .positioned_refs()
        .filter(|(mate, stored)| {
            *mate != exclude
                && positions::resolve_to_pick(graph, stored.anchor) == Some(pick)
                && positions::ref_depth(graph, *mate) + 1 == depth
        })
        .map(|(mate, _)| mate)
        .min()
}

/// Re-point the reference at `node` at the commit `new_anchor` — `git update-ref`, spoken as
/// a position move. Its approaching legs follow it (their edges move onto the new anchor),
/// chain members stacked above move with it, and members below lose their approach (they
/// become roots at the old anchor). An anchorless reference is placed as a fresh root; a
/// reference already resolving there just refreshes its stored anchor.
pub(crate) fn repoint_ref(graph: &mut StepGraph, node: StepGraphIndex, new_anchor: StepGraphIndex) {
    let Some(stored) = graph.position_of(node) else {
        place_ref(graph, node, StackSlot::Root(new_anchor));
        return;
    };
    match positions::resolve_to_pick(graph, stored.anchor) {
        Some(old_anchor) if old_anchor != new_anchor => {
            // Snapshot the reference's legs before moving their edges (the derived approach
            // tracks live edges), so it can be re-placed against `new_anchor`'s final legs.
            let approach = positions::ref_approach(graph, node);
            for &(leg, slot) in &approach {
                if graph.parents(leg).get(slot) == Some(&old_anchor) {
                    graph.replace_parent(leg, slot, new_anchor);
                }
            }
            // Its old below stays behind; at the destination the reference sits on whatever
            // holds the depth below it there — or lands directly on the pick when that stack
            // doesn't exist (its carried mates follow through their below-chains).
            let below =
                mate_below_depth(graph, node, new_anchor, positions::ref_depth(graph, node));
            // Carried = the below-subtree stacked on the reference. Depth-tied siblings and the
            // below-chain underneath are NOT carried — they stay at the old anchor, though chain
            // mates lose their approach (their legs move with `node`) and become roots there.
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
            let mates: Vec<_> = positions::chain_members(graph, node)
                .into_iter()
                .filter(|(mate, _)| *mate != node && !carried.contains(mate))
                .collect();
            for (mate, member) in mates {
                graph.set_position(mate, member.anchor, &[], false, member.below);
            }
            for &mate in &carried[1..] {
                graph.rekey_position(mate, new_anchor);
            }
            // The reference's legs moved with it; re-classify its lane against `new_anchor`'s
            // final legs (its old `Lane` slot may not exist there).
            graph.set_position(node, new_anchor, &approach, stored.ambiguous, below);
        }
        _ => {
            graph.rekey_position(node, new_anchor);
        }
    }
}

/// Remove the reference at `node` from its chain: members above close the gap and the
/// reference becomes a root at its current anchor — nothing descends into it any more. With
/// `drop_legs` the pick edges that approached its position are removed outright; otherwise
/// they stay on the anchor for a follow-up reconnect to rewire.
pub(crate) fn unhook_ref(graph: &mut StepGraph, node: StepGraphIndex, drop_legs: bool) {
    let Some(unhooked) = graph.position_of(node) else {
        return;
    };
    // The chain closes past the unhooked reference: mates that sat on it settle onto what it
    // sat on, becoming its sibling branch (the unhooked ref keeps its spot).
    let rehang: Vec<_> = positions::chain_members(graph, node)
        .into_iter()
        .filter(|(mate, m)| *mate != node && m.below == Some(node))
        .map(|(mate, _)| mate)
        .collect();
    for mate in rehang {
        graph.set_below(mate, unhooked.below);
    }
    if drop_legs && let Some(anchor) = positions::resolve_to_pick(graph, unhooked.anchor) {
        let mut legs = positions::ref_approach(graph, node);
        legs.sort_unstable();
        // Descending slots per leg: a removal shifts only the slots above it, so every
        // pending (leg, slot) name below stays exact.
        for (leg, slot) in legs.into_iter().rev() {
            if graph.parents(leg).get(slot) == Some(&anchor) {
                graph.remove_parent(leg, slot);
            }
        }
    }
    graph.set_position(node, unhooked.anchor, &[], false, unhooked.below);
}

/// Move the stack slice led by `lead_ref` — it and its below-subtree in its lane on
/// `source_pick` — onto `dest_anchor`: the lead lands at the bottom, each member is
/// re-classified against its own legs at the destination (they come along), and stored
/// ambiguity is preserved.
pub(crate) fn transfer_stack(
    graph: &mut StepGraph,
    lead_ref: StepGraphIndex,
    source_pick: StepGraphIndex,
    dest_anchor: StepGraphIndex,
) {
    if graph.position_of(lead_ref).is_none() {
        return;
    }
    let lane = positions::ref_approach(graph, lead_ref);
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
                    && positions::resolve_to_pick(graph, stored.anchor) == Some(source_pick)
                    && positions::ref_approach(graph, *node) == lane
            })
            .map(|(node, _)| node)
            .collect();
        moves.extend(dependents);
    }
    for node in moves {
        let Some(stored) = graph.position_of(node) else {
            continue;
        };
        let approach = positions::ref_approach(graph, node);
        // The lead lands at the bottom of the destination (its old below stays behind);
        // the rest of the slice keeps its internal stacking.
        let below = (node != lead_ref).then_some(stored.below).flatten();
        graph.set_position(node, dest_anchor, &approach, stored.ambiguous, below);
    }
}

/// Carry the slice of `lane` on `source_pick` strictly above depth `above_depth` onto
/// `dest_anchor` verbatim — same depths, same kinds; only the anchor key changes. The
/// delimiter position below the slice stays behind. `lane`/`above_depth` are caller-captured
/// (pre-mutation) coordinates rather than live derivations.
pub(crate) fn carry_stack_above(
    graph: &mut StepGraph,
    source_pick: StepGraphIndex,
    lane: &[(StepGraphIndex, usize)],
    above_depth: usize,
    dest_anchor: StepGraphIndex,
) {
    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|(node, stored)| {
            positions::resolve_to_pick(graph, stored.anchor) == Some(source_pick)
                && positions::ref_approach(graph, *node) == lane
                && positions::ref_depth(graph, *node) > above_depth
        })
        .map(|(node, _)| node)
        .collect();
    for &node in &moves {
        graph.rekey_position(node, dest_anchor);
    }
    // The slice bottom sat on the delimiter left behind; at the destination (depths carried
    // verbatim) it sits on whatever holds the depth below it there.
    for &node in &moves {
        if let Some(stored) = graph.position_of(node)
            && stored.below.is_some_and(|b| !moves.contains(&b))
        {
            let mate =
                mate_below_depth(graph, node, dest_anchor, positions::ref_depth(graph, node));
            graph.set_below(node, mate);
        }
    }
}

/// Stack every reference on `source_pick` above `top` (a reference on another pick), the
/// whole tower re-placed behind `bridge_anchor`'s full incoming leg set — the bridged legs
/// that now descend into the joined chain. Returns false (leaving the graph untouched) when
/// `top` holds no position.
pub(crate) fn land_stack_above(
    graph: &mut StepGraph,
    source_pick: StepGraphIndex,
    top: StepGraphIndex,
    bridge_anchor: StepGraphIndex,
) -> bool {
    let Some(top_stored) = graph.position_of(top) else {
        return false;
    };
    let bridge = positions::legs_into_pick(graph, bridge_anchor);
    let top_depth = positions::ref_depth(graph, top);
    graph.set_position(
        top,
        top_stored.anchor,
        &bridge,
        bridge.len() > 1,
        top_stored.below,
    );

    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|(_, stored)| positions::resolve_to_pick(graph, stored.anchor) == Some(source_pick))
        .map(|(node, stored)| (node, positions::ref_depth(graph, node), stored.below))
        .collect();
    for (node, depth, below) in moves {
        // The tower's internal stacking is preserved; its bottom members (they sat on the
        // source pick) now sit on whatever holds the depth below their landing spot — `top`
        // itself when it lives on the bridge anchor, its stand-in there otherwise.
        let below =
            below.or_else(|| mate_below_depth(graph, node, bridge_anchor, depth + top_depth + 1));
        graph.set_position(node, bridge_anchor, &bridge, bridge.len() > 1, below);
    }
    true
}

/// Re-key every reference whose anchor no longer resolves (it sat on removed picks) onto
/// `new_anchor`, positions carried verbatim — the ruled dangling semantics: the position
/// follows where the commit's place went, the approach stays.
pub(crate) fn readopt_dangling_refs(graph: &mut StepGraph, new_anchor: StepGraphIndex) {
    let dangling: Vec<_> = graph
        .positioned_refs()
        .filter(|(_, stored)| positions::resolve_to_pick(graph, stored.anchor).is_none())
        .collect();
    for (node, _) in dangling {
        graph.rekey_position(node, new_anchor);
    }
}

/// Which side of `at_ref` a chain split leaves with the lower part.
pub(crate) enum SplitBoundary {
    /// Members strictly above the ref move up; the ref stays with the lower part.
    Above,
    /// The ref and members above it move up; only members below stay.
    At,
}

/// The result of splitting a chain around an interposed pick.
pub(crate) struct ChainSplit {
    /// The members left behind, with their pre-split anchors — settle them with
    /// [`settle_chain_lower`] once the leg entering the lower part is known.
    pub lower: Vec<(StepGraphIndex, RefPosition)>,
    /// Whether any member moved onto the upper anchor. When none did, `at_ref` was the top
    /// of its chain, so the chain's carried legs belong to the caller's new pick.
    pub moved_any: bool,
}

/// Split the chain at `at_ref` around a pick interposed into it: members on the upper side
/// of `boundary` re-key onto `upper_anchor` with the boundary member landing at the bottom
/// (approach kinds carried verbatim), and the lower members are returned untouched for the
/// caller to settle.
pub(crate) fn split_chain(
    graph: &mut StepGraph,
    at_ref: StepGraphIndex,
    boundary: SplitBoundary,
    upper_anchor: StepGraphIndex,
) -> ChainSplit {
    if graph.position_of(at_ref).is_none() {
        return ChainSplit {
            lower: Vec::new(),
            moved_any: false,
        };
    }
    let members = positions::chain_members(graph, at_ref);
    // The upper side is the below-subtree on the moving side of the boundary — depth-tied
    // siblings from other stacks can share the chain's approach but hang elsewhere and stay.
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
            graph.rekey_position(node, upper_anchor);
            if member.below.is_none_or(|b| !moved.contains(&b)) {
                // The boundary member lands at the bottom of the upper anchor; its old
                // below stays on the lower side of the split.
                boundary_below = member.below;
                graph.set_below(node, None);
            }
        } else {
            lower.push((node, member));
        }
    }
    // References stacked on the moved slice but not moving with it (cross-lane roots, e.g. a
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
    ChainSplit {
        lower,
        moved_any: !moved.is_empty(),
    }
}

/// Settle the lower part of a split chain: each member keeps its anchor and stacking but is
/// now approached through `leg` — the edge descending from the interposed pick.
pub(crate) fn settle_chain_lower(
    graph: &mut StepGraph,
    lower: &[(StepGraphIndex, RefPosition)],
    leg: (StepGraphIndex, usize),
) {
    for (node, member) in lower {
        graph.set_position(*node, member.anchor, &[leg], false, member.below);
    }
}

/// One lane of a co-located group: refs bottom-up. `rank`/`ambiguous` are carried verbatim in
/// this v1 (rank is only topology-defined inside carrying chains; root-sibling order is table
/// data by design).
#[derive(Debug, Clone)]
struct Lane {
    refs: Vec<(gix::refs::FullName, usize, bool)>,
    carry: LaneCarry,
}

/// The lanes of every anchor pick that has references on it.
struct Arrangement {
    groups: HashMap<StepGraphIndex, Vec<Lane>>,
}

/// Extract the arrangement from the CURRENT stored positions, recording anomalies that the
/// name-keyed model must care about (duplicate names, unanchored refs, non-contiguous chain
/// ranks, non-consecutive lane legs).
fn extract(graph: &StepGraph, notes: &mut Vec<String>) -> Arrangement {
    let mut seen_names: HashMap<gix::refs::FullName, StepGraphIndex> = HashMap::new();
    // (anchor, approach) -> members
    type ChainKey = (StepGraphIndex, Vec<(StepGraphIndex, usize)>);
    let mut chains: HashMap<ChainKey, Vec<(gix::refs::FullName, usize, bool)>> = HashMap::new();
    for (node, refname, _) in graph.references() {
        if let Some(previous) = seen_names.insert(refname.clone(), node) {
            notes.push(format!("DUPNAME {refname:?} nodes {previous} and {node}"));
        }
        let Some(stored) = graph.position_of(node) else {
            continue; // no stored anchor: unborn, exempt like the standing assert
        };
        let Some(anchor) = positions::resolve_to_pick(graph, stored.anchor) else {
            notes.push(format!("UNANCHORED {refname:?}"));
            continue;
        };
        chains
            .entry((anchor, positions::ref_approach(graph, node)))
            .or_default()
            .push((
                refname.clone(),
                positions::ref_depth(graph, node),
                stored.ambiguous,
            ));
    }

    type ApproachedLane = (Vec<(StepGraphIndex, usize)>, Lane);
    let mut groups: HashMap<StepGraphIndex, Vec<ApproachedLane>> = HashMap::new();
    for ((anchor, approach), mut members) in chains {
        members.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let legs = legs_into_pick(graph, anchor);
        let carry = if approach.is_empty() {
            LaneCarry::None
        } else if approach == legs {
            LaneCarry::All
        } else {
            // The consumption model needs each lane's legs to be a consecutive run of the
            // anchor's sorted legs; measure violations instead of assuming.
            if let Some(start) = legs.iter().position(|l| Some(l) == approach.first()) {
                if legs[start..].len() < approach.len()
                    || legs[start..start + approach.len()] != approach[..]
                {
                    notes.push(format!(
                        "NONCONSECUTIVE anchor {anchor} approach {approach:?} legs {legs:?}"
                    ));
                }
            } else {
                notes.push(format!(
                    "APPROACH-NOT-IN-LEGS anchor {anchor} approach {approach:?} legs {legs:?}"
                ));
            }
            LaneCarry::Count(approach.len())
        };
        if carry != LaneCarry::None {
            let ranks: Vec<usize> = members.iter().map(|m| m.1).collect();
            if ranks.iter().copied().ne(0..members.len()) {
                notes.push(format!(
                    "RANK-GAP anchor {anchor} ranks {ranks:?} (carrying chain)"
                ));
            }
        }
        groups.entry(anchor).or_default().push((
            approach,
            Lane {
                refs: members,
                carry,
            },
        ));
    }

    // Lane order within a group: carrying lanes by their first leg (the parent-array order the
    // materialization writes), shared `All` lanes next, root lanes last by name. This is the
    // order a name-keyed table would persist.
    let groups = groups
        .into_iter()
        .map(|(anchor, mut lanes)| {
            lanes.sort_by(|(approach_a, lane_a), (approach_b, lane_b)| {
                let class = |lane: &Lane| match lane.carry {
                    LaneCarry::Count(_) => 0,
                    LaneCarry::All => 1,
                    LaneCarry::None => 2,
                };
                class(lane_a)
                    .cmp(&class(lane_b))
                    .then_with(|| approach_a.cmp(approach_b))
                    .then_with(|| lane_a.refs.cmp(&lane_b.refs))
            });
            (anchor, lanes.into_iter().map(|(_, lane)| lane).collect())
        })
        .collect();
    Arrangement { groups }
}

/// A derived position: anchor, rank, approach, ambiguous.
type DerivedPosition = (StepGraphIndex, usize, Vec<(StepGraphIndex, usize)>, bool);

/// Re-derive every reference's position from the arrangement + live edges: `Count` lanes consume
/// the anchor's sorted legs in lane order, `All` lanes take them all, `None` lanes take none.
fn derive(
    graph: &StepGraph,
    arrangement: &Arrangement,
    notes: &mut Vec<String>,
) -> HashMap<gix::refs::FullName, DerivedPosition> {
    let mut out = HashMap::new();
    for (&anchor, lanes) in &arrangement.groups {
        let legs = legs_into_pick(graph, anchor);
        let mut consumed = 0usize;
        for lane in lanes {
            let approach = match lane.carry {
                LaneCarry::None => Vec::new(),
                LaneCarry::All => legs.clone(),
                LaneCarry::Count(n) => {
                    let run = legs
                        .get(consumed..consumed + n)
                        .map(<[_]>::to_vec)
                        .unwrap_or_default();
                    consumed += n;
                    run
                }
            };
            for (name, rank, ambiguous) in &lane.refs {
                out.insert(name.clone(), (anchor, *rank, approach.clone(), *ambiguous));
            }
        }
        if consumed > 0 && consumed != legs.len() {
            notes.push(format!(
                "UNCONSUMED-LEGS anchor {anchor} consumed {consumed} of {}",
                legs.len()
            ));
        }
    }
    out
}

/// Round-trip the current graph through the name-keyed arrangement and report every divergence
/// and anomaly. Empty result = this graph's positions are fully order-derivable.
fn census(graph: &StepGraph) -> Vec<String> {
    let mut notes = Vec::new();
    let arrangement = extract(graph, &mut notes);
    // Is rank a contiguous, duplicate-free 0..n stack index per anchor across ALL lanes?
    // (The forest store derives rank from one global list per anchor if so.)
    for (anchor, lanes) in &arrangement.groups {
        let mut ranks: Vec<usize> = lanes
            .iter()
            .flat_map(|lane| lane.refs.iter().map(|(_, rank, _)| *rank))
            .collect();
        ranks.sort_unstable();
        if !ranks.iter().copied().eq(0..ranks.len()) {
            notes.push(format!("GLOBAL-RANK anchor {anchor} ranks {ranks:?}"));
        }
    }
    let derived = derive(graph, &arrangement, &mut notes);
    for (node, refname, _) in graph.references() {
        let Some(stored) = graph.position_of(node) else {
            continue;
        };
        let Some(anchor) = positions::resolve_to_pick(graph, stored.anchor) else {
            continue;
        };
        let rank = positions::ref_depth(graph, node);
        let approach = positions::ref_approach(graph, node);
        match derived.get(refname) {
            Some((d_anchor, d_rank, d_approach, d_ambiguous)) => {
                if (*d_anchor, *d_rank, d_approach, *d_ambiguous)
                    != (anchor, rank, &approach, stored.ambiguous)
                {
                    notes.push(format!(
                        "DIVERGE {refname:?} stored=({anchor},{rank},{approach:?},{}) derived=({d_anchor},{d_rank},{d_approach:?},{d_ambiguous})",
                        stored.ambiguous
                    ));
                }
            }
            None => notes.push(format!("MISSING {refname:?}")),
        }
    }
    notes
}

/// Env-gated corpus probe: when `BUT_ARRANGE_CENSUS` names a file, append this graph's census
/// findings (and a `GRAPHS` counter line) to it. Capture-proof, like the earlier census tooling.
pub(crate) fn census_to_file(graph: &StepGraph) {
    let Ok(path) = std::env::var("BUT_ARRANGE_CENSUS") else {
        return;
    };
    let notes = census(graph);
    use std::io::Write as _;
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let refs = graph.references().count();
    let _ = writeln!(file, "GRAPH refs={refs} findings={}", notes.len());
    for note in notes {
        let _ = writeln!(file, "{note}");
    }
}
