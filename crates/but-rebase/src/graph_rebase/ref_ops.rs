//! The reference-op API: mutation sites say where a reference should end up
//! ([`StackPlace`]) and let [`place_ref`] and friends work out the `(on, below, group)`
//! details.
//!
//! These ops read and write the layout table: per pick, an ordered list of groups of ref
//! NAMES — the shape of workspace metadata — with pick, rank, and entering edges DERIVED
//! from the table plus live pick edges (`positioned_on`, `ref_depth`, `edges_through`).
//! Names never churn under graph mutation, which is what keeps the table stable while
//! commits are rewritten around it.

use crate::graph_rebase::GraphEditor;
use crate::graph_rebase::graph_editor::{Edge, PickIndex, RefIndex};
use crate::graph_rebase::positions;

/// A position in a commit's reference stack, named by intent.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StackPlace {
    /// Directly above this reference in its group: mates that sat on it re-hang onto the
    /// newcomer.
    Above(RefIndex),
    /// At this reference's position: the newcomer takes its below, and it re-hangs onto the
    /// newcomer.
    Below(RefIndex),
    /// The bottom of the pick's whole stack (carrying all its edges — "the branch here");
    /// every reference that sat on the pick itself re-hangs onto the newcomer.
    Bottom(PickIndex),
    /// The top of the group the edge `(child, parent number)` carries into `pick`.
    GroupTop {
        /// The commit the group sits on.
        pick: PickIndex,
        /// The child edge whose group the reference stacks onto.
        edge: (PickIndex, usize),
    },
    /// A fresh root above `pick`: nothing descends into it, no other position moves.
    Root(PickIndex),
}

/// Re-hang the references sitting directly ON `pick` (no `below` of their own) onto
/// `moving`, which is taking the pick-bottom spot; `moving` itself is left alone.
fn rehang_pick_bottom(graph: &mut GraphEditor, pick: PickIndex, moving: RefIndex) {
    let rehang: Vec<_> = graph
        .positioned_refs()
        .filter(|&mate| {
            mate != moving
                && graph.below_of(mate).is_none()
                && positions::resolve_to_pick(graph, mate) == Some(pick)
        })
        .collect();
    for mate in rehang {
        graph.set_below(mate, Some(moving));
    }
}

/// The topmost reference on `pick` entered through exactly `entering` — the one a
/// group-top placement stacks above; `moving` itself is not a candidate.
fn group_top_at(
    graph: &GraphEditor,
    pick: PickIndex,
    entering: &[Edge],
    moving: RefIndex,
) -> Option<RefIndex> {
    graph
        .positioned_refs()
        .filter(|&mate| {
            mate != moving
                && positions::resolve_to_pick(graph, mate) == Some(pick)
                && positions::edges_through(graph, mate) == entering
        })
        .max_by_key(|&mate| (positions::ref_depth(graph, mate), mate))
}

/// Place the reference at `entry` into `place`, shifting other positions as the placement demands.
/// Fresh placement only — moving an existing reference is [`move_ref`].
pub(crate) fn place_ref(graph: &mut GraphEditor, entry: RefIndex, place: StackPlace) {
    match place {
        StackPlace::Above(target) => {
            if !graph.is_positioned(target) {
                return;
            }
            // Same-group members that sat directly on the target now sit on the
            // interposed entry; cross-group members on the target keep it (they branch).
            let rehang: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|&mate| mate != entry && graph.below_of(mate) == Some(target))
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(entry));
            }
            graph.join_group_of(entry, target, Some(target));
        }
        StackPlace::Below(target) => {
            if !graph.is_positioned(target) {
                return;
            }
            let below = graph.below_of(target);
            graph.join_group_of(entry, target, below);
            graph.set_below(target, Some(entry));
        }
        StackPlace::Bottom(pick) => {
            let entering = positions::edges_into(graph, pick);
            // Members that sat on the pick itself now sit on the new bottom.
            rehang_pick_bottom(graph, pick, entry);
            graph.set_position(entry, pick, &entering, entering.len() > 1, None);
        }
        StackPlace::GroupTop { pick, edge } => {
            let entering = vec![edge];
            let top = group_top_at(graph, pick, &entering, entry);
            graph.set_position(entry, pick, &entering, false, top);
        }
        StackPlace::Root(pick) => {
            graph.set_position(entry, pick, &[], false, None);
        }
    }
}

/// Move the reference at `entry` into `place`. Unlike [`place_ref`], the reference already
/// holds a position: when it is the sole carrier of its entering edges, those follow it
/// (re-pointed onto the new pick and merged into the destination's entering set), and — when
/// moving above another reference — the members now entered through it share the merged set.
pub(crate) fn move_ref(graph: &mut GraphEditor, entry: RefIndex, place: StackPlace) {
    let Some(moving_on) = graph.positioned_on(entry) else {
        return;
    };
    let moving_edges = positions::edges_through(graph, entry);
    // Sole carrier = nothing in the group sits below the mover. Measured before any shuffling
    // (the shuffles never change which members those are).
    let moving_depth = positions::ref_depth(graph, entry);
    let sole_carrier = !positions::group_members(graph, entry)
        .into_iter()
        .any(|mate| mate != entry && positions::ref_depth(graph, mate) < moving_depth);
    // The mover vacates its old spot: members stacked directly on it settle onto what it sat
    // on.
    graph.splice(entry);
    // Each arm yields (on, entering, below); the mover's edges merge into `entering` below,
    // after re-pointing, so the final `set_position` sees the complete set. Each arm hangs the
    // mover on its new below FIRST so re-hanging mates never creates a transient below-cycle
    // through its stale pointer.
    let (on, mut entering, below) = match place {
        StackPlace::Above(target) => {
            let Some(t_on) = graph.positioned_on(target) else {
                return;
            };
            graph.set_below(entry, Some(target));
            // Same-group members that sat directly on the target now sit on the mover.
            let rehang: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|&mate| mate != entry && graph.below_of(mate) == Some(target))
                .collect();
            for mate in rehang {
                graph.set_below(mate, Some(entry));
            }
            (t_on, positions::edges_through(graph, target), Some(target))
        }
        StackPlace::Below(target) => {
            let Some(t_on) = graph.positioned_on(target) else {
                return;
            };
            let t_below = graph.below_of(target);
            graph.set_below(entry, t_below);
            graph.set_below(target, Some(entry));
            (t_on, positions::edges_through(graph, target), t_below)
        }
        StackPlace::Bottom(pick) => {
            // Refs that sat on the pick itself re-hang onto the mover.
            graph.set_below(entry, None);
            rehang_pick_bottom(graph, pick, entry);
            (pick, Vec::new(), None)
        }
        StackPlace::GroupTop { pick, edge } => {
            let entering = vec![edge];
            let top = group_top_at(graph, pick, &entering, entry);
            graph.set_below(entry, top);
            (pick, entering, top)
        }
        StackPlace::Root(pick) => {
            graph.set_below(entry, None);
            (pick, Vec::new(), None)
        }
    };
    // The edges follow the mover only when it was their sole carrier: group members staying
    // behind keep their entering edges.
    let old_resolved = positions::resolve_to_pick(graph, moving_on);
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
        if let StackPlace::Above(target) = place
            && graph.is_positioned(target)
        {
            let t_depth = positions::ref_depth(graph, target);
            let mates: Vec<_> = positions::group_members(graph, target)
                .into_iter()
                .filter(|&mate| mate != entry && positions::ref_depth(graph, mate) <= t_depth)
                .filter_map(|mate| {
                    graph
                        .positioned_on(mate)
                        .map(|on| (mate, on, graph.below_of(mate)))
                })
                .collect();
            for (mate, m_on, m_below) in mates {
                graph.set_position(mate, m_on, &entering, entering.len() > 1, m_below);
            }
        }
    }
    graph.set_position(entry, on, &entering, entering.len() > 1, below);
}

/// Re-point the captured `edges` from `from` onto `to`, keeping each edge's parent number so its
/// group statement follows the name. Edges already rewired elsewhere are left alone.
pub(crate) fn redirect_edges(
    graph: &mut GraphEditor,
    edges: &[Edge],
    from: PickIndex,
    to: PickIndex,
) {
    if from == to {
        return;
    }
    for &(child, parent_number) in edges {
        if graph.parents(child).get(parent_number) == Some(&from) {
            graph.replace_parent(child, parent_number, to);
        }
    }
}

/// The mate a reference landing at `depth` on `on` (resolved) sits on — the member at
/// `depth - 1`, excluding `exclude`, lowest entry id on a tie.
fn mate_below_depth(
    graph: &GraphEditor,
    exclude: RefIndex,
    on: PickIndex,
    depth: usize,
) -> Option<RefIndex> {
    if depth == 0 {
        return None;
    }
    let pick = positions::resolve_to_pick(graph, on)?;
    graph
        .positioned_refs()
        .filter(|&mate| {
            mate != exclude
                && positions::resolve_to_pick(graph, mate) == Some(pick)
                && positions::ref_depth(graph, mate) + 1 == depth
        })
        .min()
}

/// Re-point the reference at `entry` at the commit `onto` — `git update-ref` as a position
/// move. Its entering edges and the members stacked above move with it; members below lose
/// their entering edges and become roots at the old pick. An unplaced reference is placed as
/// a fresh root; one already resolving there just refreshes its stored `on`.
pub(crate) fn repoint_ref(graph: &mut GraphEditor, entry: RefIndex, onto: PickIndex) {
    if !graph.is_positioned(entry) {
        place_ref(graph, entry, StackPlace::Root(onto));
        return;
    }
    match positions::resolve_to_pick(graph, entry) {
        Some(old_pick) if old_pick != onto => {
            // Snapshot the entering edges before re-pointing them — the derived read tracks
            // live edges.
            let entering = positions::edges_through(graph, entry);
            redirect_edges(graph, &entering, old_pick, onto);
            // At the destination the reference sits on whatever holds the depth below it
            // there, or directly on the pick when that stack doesn't exist.
            let below = mate_below_depth(graph, entry, onto, positions::ref_depth(graph, entry));
            // Carried = the below-subtree stacked on the reference. Depth-tied siblings and
            // the walk underneath stay at the old pick, though group mates lose their entering
            // edges (those move with `entry`) and become roots there.
            let mut carried = vec![entry];
            let mut i = 0;
            while i < carried.len() {
                let current = carried[i];
                i += 1;
                let dependents: Vec<_> = graph
                    .positioned_refs()
                    .filter(|&mate| {
                        graph.below_of(mate) == Some(current)
                            && !carried.contains(&mate)
                            && graph.is_reference(mate)
                    })
                    .collect();
                carried.extend(dependents);
            }
            let mates: Vec<_> = positions::group_members(graph, entry)
                .into_iter()
                .filter(|&mate| mate != entry && !carried.contains(&mate))
                .filter_map(|mate| {
                    graph
                        .positioned_on(mate)
                        .map(|on| (mate, on, graph.below_of(mate)))
                })
                .collect();
            for (mate, m_on, m_below) in mates {
                graph.set_position(mate, m_on, &[], false, m_below);
            }
            for &mate in &carried[1..] {
                graph.rekey_position(mate, onto);
            }
            // Re-classify against `onto`'s final edges — the old `Edges` statement may not
            // exist there.
            let ambiguous = graph.ambiguous_of(entry);
            graph.set_position(entry, onto, &entering, ambiguous, below);
        }
        _ => {
            graph.rekey_position(entry, onto);
        }
    }
}

/// Remove the reference at `entry` from its group: members above close the gap and it becomes
/// a root at its current pick. With `drop_edges` the edges that entered through it are removed
/// outright; otherwise they stay on the pick for a follow-up reconnect to rewire.
pub(crate) fn unhook_ref(graph: &mut GraphEditor, entry: RefIndex, drop_edges: bool) {
    let Some(unhooked_on) = graph.positioned_on(entry) else {
        return;
    };
    let unhooked_below = graph.below_of(entry);
    // Mates that sat on the unhooked reference settle onto what it sat on.
    let rehang: Vec<_> = positions::group_members(graph, entry)
        .into_iter()
        .filter(|&mate| mate != entry && graph.below_of(mate) == Some(entry))
        .collect();
    for mate in rehang {
        graph.set_below(mate, unhooked_below);
    }
    if drop_edges && let Some(pick) = positions::resolve_to_pick(graph, entry) {
        let mut edges = positions::edges_through(graph, entry);
        edges.sort_unstable();
        // Descending parent numbers per child: a removal shifts only the parent numbers above it, so every
        // pending `(child, parent number)` name below stays exact.
        for (child, parent_number) in edges.into_iter().rev() {
            if graph.parents(child).get(parent_number) == Some(&pick) {
                graph.remove_parent(child, parent_number);
            }
        }
    }
    graph.set_position(entry, unhooked_on, &[], false, unhooked_below);
}

/// Move the stack slice led by `lead_ref` — it and its below-subtree in its group on
/// `source_pick` — onto `dest`: the lead lands at the bottom, each member re-classifies
/// against its own edges at the destination, and stored ambiguity is preserved.
pub(crate) fn transfer_stack(
    graph: &mut GraphEditor,
    lead_ref: RefIndex,
    source_pick: PickIndex,
    dest: PickIndex,
) {
    if !graph.is_positioned(lead_ref) {
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
            .filter(|&entry| {
                graph.below_of(entry) == Some(current)
                    && !moves.contains(&entry)
                    && positions::resolve_to_pick(graph, entry) == Some(source_pick)
                    && positions::edges_through(graph, entry) == lead_entering
            })
            .collect();
        moves.extend(dependents);
    }
    for entry in moves {
        if !graph.is_positioned(entry) {
            continue;
        }
        let entering = positions::edges_through(graph, entry);
        // The lead's old below stays behind; the rest keeps its internal stacking.
        let below = (entry != lead_ref).then(|| graph.below_of(entry)).flatten();
        let ambiguous = graph.ambiguous_of(entry);
        graph.set_position(entry, dest, &entering, ambiguous, below);
    }
}

/// Carry the slice of the group identified by `entering` on `source_pick` strictly above
/// depth `above_depth` onto `dest` verbatim — same depths, same kinds; only the `on` key
/// changes. The delimiter below the slice stays behind. `entering`/`above_depth` are
/// caller-captured (pre-mutation) coordinates, not live derivations.
pub(crate) fn carry_stack_above(
    graph: &mut GraphEditor,
    source_pick: PickIndex,
    entering: &[(PickIndex, usize)],
    above_depth: usize,
    dest: PickIndex,
) {
    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|&entry| {
            positions::resolve_to_pick(graph, entry) == Some(source_pick)
                && positions::edges_through(graph, entry) == entering
                && positions::ref_depth(graph, entry) > above_depth
        })
        .collect();
    for &entry in &moves {
        graph.rekey_position(entry, dest);
    }
    // The slice bottom sat on the delimiter left behind; at the destination it sits on
    // whatever holds the depth below it there.
    for &entry in &moves {
        if graph.below_of(entry).is_some_and(|b| !moves.contains(&b)) {
            let mate = mate_below_depth(graph, entry, dest, positions::ref_depth(graph, entry));
            graph.set_below(entry, mate);
        }
    }
}

/// Stack every reference on `source_pick` above `top` (a reference on another pick), the
/// whole tower re-placed behind `bridge_pick`'s full incoming edge set — the bridged edges
/// now descending into the joined group. Returns false (graph untouched) when `top` holds
/// no position.
pub(crate) fn land_stack_above(
    graph: &mut GraphEditor,
    source_pick: PickIndex,
    top: RefIndex,
    bridge_pick: PickIndex,
) -> bool {
    let Some(top_on) = graph.positioned_on(top) else {
        return false;
    };
    let bridge = positions::edges_into(graph, bridge_pick);
    let top_depth = positions::ref_depth(graph, top);
    let top_below = graph.below_of(top);
    graph.set_position(top, top_on, &bridge, bridge.len() > 1, top_below);

    let moves: Vec<_> = graph
        .positioned_refs()
        .filter(|&entry| positions::resolve_to_pick(graph, entry) == Some(source_pick))
        .map(|entry| {
            (
                entry,
                positions::ref_depth(graph, entry),
                graph.below_of(entry),
            )
        })
        .collect();
    for (entry, depth, below) in moves {
        // Bottom members (they sat on the source pick) now sit on whatever holds the depth
        // below their landing spot — `top` when it lives on the bridge pick, its stand-in
        // there otherwise.
        let below =
            below.or_else(|| mate_below_depth(graph, entry, bridge_pick, depth + top_depth + 1));
        graph.set_position(entry, bridge_pick, &bridge, bridge.len() > 1, below);
    }
    true
}

/// Re-key every reference whose `on` no longer resolves (it sat on removed picks) onto
/// `onto`, positions carried verbatim — dangling references follow where the commit's
/// place went; their entering edges stay.
pub(crate) fn readopt_dangling_refs(graph: &mut GraphEditor, onto: PickIndex) {
    let dangling: Vec<_> = graph
        .positioned_refs()
        .filter(|&entry| positions::resolve_to_pick(graph, entry).is_none())
        .collect();
    for entry in dangling {
        graph.rekey_position(entry, onto);
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
    /// The members left behind, with their pre-split `(on, below)` — settle them with
    /// [`settle_group_lower`] once the edge entering the lower part is known.
    pub lower: Vec<(RefIndex, PickIndex, Option<RefIndex>)>,
    /// Whether any member moved onto the upper entry. When none did, `at_ref` was the top
    /// of its group, so the group's carried edges belong to the caller's new pick.
    pub moved_any: bool,
}

/// Split the group at `at_ref` around a pick interposed into it: members on the upper side of
/// `boundary` re-key onto `upper` (carry kinds verbatim, boundary member at the bottom); the
/// lower members are returned untouched for the caller to settle.
pub(crate) fn split_group(
    graph: &mut GraphEditor,
    at_ref: RefIndex,
    boundary: SplitBoundary,
    upper: PickIndex,
) -> GroupSplit {
    if !graph.is_positioned(at_ref) {
        return GroupSplit {
            lower: Vec::new(),
            moved_any: false,
        };
    }
    let members: Vec<_> = positions::group_members(graph, at_ref)
        .into_iter()
        .filter_map(|entry| {
            graph
                .positioned_on(entry)
                .map(|on| (entry, on, graph.below_of(entry)))
        })
        .collect();
    // The upper side is the below-subtree on the moving side — depth-tied siblings from other
    // stacks can share the group's entering edges but hang elsewhere and stay.
    let mut moved = vec![at_ref];
    let mut i = 0;
    while i < moved.len() {
        let current = moved[i];
        i += 1;
        let dependents: Vec<_> = members
            .iter()
            .filter(|(entry, _, below)| *below == Some(current) && !moved.contains(entry))
            .map(|(entry, ..)| *entry)
            .collect();
        moved.extend(dependents);
    }
    if matches!(boundary, SplitBoundary::Above) {
        moved.remove(0);
    }
    let mut lower = Vec::new();
    let mut boundary_below = None;
    for (entry, on, below) in members {
        if moved.contains(&entry) {
            graph.rekey_position(entry, upper);
            if below.is_none_or(|b| !moved.contains(&b)) {
                // The boundary member lands at the bottom; its old below stays on the
                // lower side.
                boundary_below = below;
                graph.set_below(entry, None);
            }
        } else {
            lower.push((entry, on, below));
        }
    }
    // References stacked on the moved slice but not moving with it (cross-group roots, e.g. a
    // remote above the moved tip) settle onto what the slice sat on.
    let stranded: Vec<_> = graph
        .positioned_refs()
        .filter(|&entry| {
            !moved.contains(&entry) && graph.below_of(entry).is_some_and(|b| moved.contains(&b))
        })
        .collect();
    for entry in stranded {
        graph.set_below(entry, boundary_below);
    }
    GroupSplit {
        lower,
        moved_any: !moved.is_empty(),
    }
}

/// Settle the lower part of a split group: each member keeps its `on` and stacking but is now
/// entered through `edge` — the edge descending from the interposed pick.
pub(crate) fn settle_group_lower(
    graph: &mut GraphEditor,
    lower: &[(RefIndex, PickIndex, Option<RefIndex>)],
    edge: (PickIndex, usize),
) {
    for &(entry, on, below) in lower {
        graph.set_position(entry, on, &[edge], false, below);
    }
}
