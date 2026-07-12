//! Where each reference sits, stored as position data rather than as graph edges.
//!
//! A commit ([`Step::Pick`]) carries parent edges; a reference ([`Step::Reference`]) carries
//! NONE. Instead every reference stands in the arrangement table (`GraphEditor::layout`):
//! per stored key, an ordered list of groups (`RefGroup`), each a bottom→top run of references
//! sharing one [`GroupCarry`]. A position reads back as a [`RefPosition`] view: `on` (the entry
//! pointed at, followed through tombstones), `below` (the reference directly underneath;
//! `None` = on the pick), and `ambiguous` (this position is a merge).
//!
//! Keeping references out of the edge graph is deliberate: an edge running THROUGH a reference
//! would make it bear connectivity it shouldn't — gluing a commit's history onto whatever else
//! the reference happens to touch.
//!
//! Vocabulary used throughout this module:
//! - **edge** — a parent edge of the commit graph, named from its child side as
//!   `(child entry, parent number)`, since edges live in parent arrays. A pick's INCOMING edges
//!   are its children's edges pointing at it.
//! - **enters through** — an incoming edge of a pick ENTERS THROUGH a reference when it
//!   descends into that reference's position (see [`edges_through`]). This distinguishes
//!   co-located references and picks out which merge group a reference belongs to.
//! - **group** — references stacked on one pick, ordered by their below walk ([`ref_depth`]).
//!   Groups are shallow in practice (≤3 observed).

use crate::graph_rebase::graph_editor::GroupCarry;
use crate::graph_rebase::graph_editor::{PickIndex, RefIndex};
use crate::graph_rebase::{EditorIndex, GraphEditor};

/// Resolve `entry` to the pick it stands for: a pick is itself, a tombstone follows its
/// preserved first edge downward, a reference goes via its stored position — dead references
/// via their RETAINED position, which stale selectors normalize through (unborn refs carry
/// none and resolve to nothing).
pub(crate) fn resolve_to_pick(
    graph: &GraphEditor,
    entry: impl Into<EditorIndex>,
) -> Option<PickIndex> {
    // A reference resolves via its (retained) stored position; unborn refs carry none and
    // resolve to nothing.
    let mut node = match entry.into() {
        EditorIndex::Pick(i) => PickIndex(i),
        entry @ EditorIndex::Ref(_) => graph.positioned_on(entry.as_ref()?)?,
    };
    // Tombstones descend their preserved first edge.
    for _ in 0..10_000 {
        if graph.is_pick(node) {
            return Some(node);
        }
        node = *graph.parents(node).first()?;
    }
    None
}

/// A pick's incoming edges as sorted `(child, parent number)` pairs. The groups on the pick
/// divide these among themselves (their [`GroupCarry`]); [`edges_through`] reads one group's share.
pub(crate) fn edges_into(graph: &GraphEditor, pick: PickIndex) -> Vec<(PickIndex, usize)> {
    graph
        .incoming_edges(pick)
        .iter()
        .copied()
        .filter(|&(child, _)| graph.is_pick(child))
        .collect()
}

/// The edges currently entering through the reference at `entry`: the group's own carry
/// statement (kept aligned by the parent number mutators), ordered and filtered by the resolved pick's
/// live edges so a stale carry edge never reaches a consumer.
pub(crate) fn edges_through(
    graph: &GraphEditor,
    entry: impl Into<EditorIndex>,
) -> Vec<(PickIndex, usize)> {
    let Some(entry) = entry.into().as_ref() else {
        return Vec::new();
    };
    let Some(on) = graph.positioned_on(entry) else {
        return Vec::new();
    };
    let Some(carry) = graph.carry_of(entry) else {
        return Vec::new();
    };
    let edges = match resolve_to_pick(graph, on) {
        Some(pick) => edges_into(graph, pick),
        None => Vec::new(),
    };
    match carry {
        GroupCarry::None => Vec::new(),
        GroupCarry::All => edges,
        GroupCarry::Edges(stated) => edges
            .into_iter()
            .filter(|edge| stated.contains(edge))
            .collect(),
    }
}

/// The members of `ref_node`'s group — every reference with the same resolved pick and the
/// same (derived) entering edges.
pub(crate) fn group_members(
    graph: &GraphEditor,
    ref_node: impl Into<EditorIndex>,
) -> Vec<RefIndex> {
    let Some(ref_node) = ref_node.into().as_ref() else {
        return vec![];
    };
    if !graph.is_positioned(ref_node) {
        return vec![];
    }
    let pick = resolve_to_pick(graph, ref_node);
    let entering = edges_through(graph, ref_node);
    graph
        .positioned_refs()
        .filter(|&entry| {
            edges_through(graph, entry) == entering && resolve_to_pick(graph, entry) == pick
        })
        .collect()
}

/// Every reference whose stored `on`, followed through tombstones, resolves to `pick`.
/// Order is unspecified (ascending entry id).
pub(crate) fn refs_resolving_to(graph: &GraphEditor, pick: PickIndex) -> Vec<RefIndex> {
    graph
        .positioned_refs()
        .filter(|&entry| resolve_to_pick(graph, entry) == Some(pick))
        .collect()
}

/// The references reachable from `start`, given the PICK set it reached. When `start` is
/// itself a reference, it counts too.
pub(crate) fn refs_reachable_with(
    graph: &GraphEditor,
    start: EditorIndex,
    picks: &std::collections::HashSet<PickIndex>,
) -> Vec<RefIndex> {
    // Match by commit id as well as entry: a graph can hold the same commit twice (in a
    // stack and in the target's history), and a reference counts as reached when its commit
    // was reached under either entry. Deleting a branch that merges back in relies on this.
    let reached_ids: std::collections::HashSet<gix::ObjectId> = picks
        .iter()
        .filter_map(|entry| graph.commit_id(*entry))
        .collect();
    let mut out = Vec::new();
    for entry in graph.positioned_refs() {
        // Pick-based reachability, commit-equivalent across duplicate groups.
        let pick_reached = resolve_to_pick(graph, entry).is_some_and(|pick| {
            picks.contains(&pick)
                || graph
                    .commit_id(pick)
                    .is_some_and(|id| reached_ids.contains(&id))
        });
        if pick_reached || EditorIndex::from(entry) == start {
            out.push(entry);
        }
    }
    out
}

/// The reference's depth above its pick — the length of its below walk (0 = directly on
/// the pick). This IS the rank: order among co-located references is adjacency, not a number.
pub(crate) fn ref_depth(graph: &GraphEditor, entry: impl Into<EditorIndex>) -> usize {
    let Some(entry) = entry.into().as_ref() else {
        return 0;
    };
    let mut depth = 0usize;
    let mut cursor = graph.below_of(entry);
    while let Some(b) = cursor {
        depth += 1;
        if depth > 10_000 {
            debug_assert!(false, "below walk cycle at ref {entry}");
            return depth;
        }
        cursor = graph.below_of(b);
    }
    depth
}

/// A group about to be entered by a new edge, captured BEFORE that edge exists so
/// [`apply_group_join`] never reads a half-updated store.
pub(crate) struct GroupJoin {
    /// The joining members: the reference and the group-mates its below walk rests on —
    /// each with its position captured (`on`, `below`, `ambiguous`). Root groups (no
    /// entering edges) at one pick are distinct siblings, so only the reference itself
    /// joins.
    members: Vec<(RefIndex, PickIndex, Option<RefIndex>, bool)>,
    /// The edges entering the group at capture time.
    entering: Vec<(PickIndex, usize)>,
}

/// Capture `ref_node`'s group for a coming join — call BEFORE the joining edge is added.
pub(crate) fn prepare_group_join(graph: &GraphEditor, ref_node: RefIndex) -> GroupJoin {
    let capture = |entry: RefIndex| {
        graph
            .positioned_on(entry)
            .map(|on| (entry, on, graph.below_of(entry), graph.ambiguous_of(entry)))
    };
    let Some(captured) = capture(ref_node) else {
        return GroupJoin {
            members: Vec::new(),
            entering: Vec::new(),
        };
    };
    let is_root = matches!(graph.carry_of(ref_node), Some(GroupCarry::None));
    let members = if is_root {
        vec![captured]
    } else {
        // Walk the below walk keeping members of this group — the physical stack may pass
        // through other groups' refs.
        let group = group_members(graph, ref_node);
        let mut members = vec![captured];
        let mut cursor = graph.below_of(ref_node);
        while let Some(b) = cursor {
            if !graph.is_positioned(b) {
                break;
            }
            if group.contains(&b) {
                members.extend(capture(b));
            }
            cursor = graph.below_of(b);
        }
        members
    };
    GroupJoin {
        members,
        entering: edges_through(graph, ref_node),
    }
}

/// The new `edge` enters the captured group: every member gains it among its entering edges,
/// classified against the pick's now-complete edges — call right AFTER the edge is added. An
/// `All` group stays `All`; an `Edges` group gains the edge; a Root descends.
pub(crate) fn apply_group_join(
    graph: &mut GraphEditor,
    join: &GroupJoin,
    edge: (PickIndex, usize),
) {
    for &(entry, on, below, was_ambiguous) in &join.members {
        let mut entering = join.entering.clone();
        if !entering.contains(&edge) {
            entering.push(edge);
        }
        let ambiguous = was_ambiguous || entering.len() > 1;
        graph.set_position(entry, on, &entering, ambiguous, below);
    }
}

/// Move every reference resolving to `from_pick` onto `to_pick`.
///
/// With `reclassify` false the carry kind is PRESERVED (an `All` group derives its edges at
/// `to_pick`, robust to the reconnect renumbering parent numbers); with it true the current derived
/// edges are re-classified against `to_pick`'s, so a ref sliding onto a dup-parent MERGE base
/// splits into the `Edges` group its edge occupies. `ambiguous` is preserved.
/// Preserve-vs-reclassify is per-situation, not cleanly per-caller — each site picks based on
/// whether the edge set should survive the move or be re-derived at the destination.
pub(crate) fn reposition_refs(
    graph: &mut GraphEditor,
    from_pick: PickIndex,
    to_pick: PickIndex,
    reclassify: bool,
) {
    let moves = refs_resolving_to(graph, from_pick);
    for entry in moves {
        if reclassify {
            let entering = edges_through(graph, entry);
            let (ambiguous, below) = (graph.ambiguous_of(entry), graph.below_of(entry));
            graph.set_position(entry, to_pick, &entering, ambiguous, below);
        } else {
            graph.rekey_position(entry, to_pick);
        }
    }
}

/// Every reference has a well-formed position, and positions are unique wherever order
/// matters topologically — within groups entered by a child edge (non-empty
/// [`edges_through`]). Several root groups above one pick are fine: they have no meaningful
/// order, and display sorts them by name.
///
/// Wired at editor creation AND at rebase entry, so every graph shape the suite produces —
/// including post-mutation shapes — continuously validates the position model.
pub(crate) fn debug_assert_positions_total(graph: &GraphEditor) {
    if !cfg!(debug_assertions) {
        return;
    }
    debug_assert_below_wellformed(graph);
    type OrderedPositionKey = (Option<PickIndex>, Vec<(PickIndex, usize)>, usize);
    let mut seen: std::collections::HashMap<OrderedPositionKey, RefIndex> = Default::default();
    for entry in graph.references().map(|(entry, _, _)| entry) {
        // No stored position is only legitimate for unborn refs (no pick below at creation).
        if !graph.is_positioned(entry) {
            continue;
        }
        let entering = edges_through(graph, entry);
        if entering.is_empty() {
            continue;
        }
        let pick = resolve_to_pick(graph, entry);
        let rank = ref_depth(graph, entry);
        if let Some(previous) = seen.insert((pick, entering.clone(), rank), entry) {
            debug_assert!(
                false,
                "references {previous} and {entry} collide at position \
                 (pick {pick:?}, entering {entering:?}, rank {rank})"
            );
        }
    }
}

/// Every stored `below` of a LIVE reference names a positioned reference resolving to the SAME
/// pick, and the below walk is acyclic. Tombstoned refs keep their stored position for
/// retention reads but are spliced out of the physical stack, so only live refs are graded.
fn debug_assert_below_wellformed(graph: &GraphEditor) {
    let name = |entry: RefIndex| match graph.reference(entry.into()) {
        Some((refname, _)) => refname.to_string(),
        None => format!("{:?}", graph.step_view(entry.into())),
    };
    for entry in graph.positioned_refs() {
        if !graph.is_reference(entry) {
            continue;
        }
        let pick = resolve_to_pick(graph, entry);
        let mut depth = 0usize;
        let mut cursor = graph.below_of(entry);
        while let Some(b) = cursor {
            if !graph.is_positioned(b) {
                debug_assert!(
                    false,
                    "ref {entry}: below {b} is not a positioned reference"
                );
                return;
            }
            debug_assert_eq!(
                resolve_to_pick(graph, b),
                pick,
                "ref {entry} ({}): below {b} ({}) resolves to a different pick",
                name(entry),
                name(b)
            );
            depth += 1;
            debug_assert!(depth <= 10_000, "ref {entry}: below walk cycle");
            if depth > 10_000 {
                return;
            }
            cursor = graph.below_of(b);
        }
    }
}
