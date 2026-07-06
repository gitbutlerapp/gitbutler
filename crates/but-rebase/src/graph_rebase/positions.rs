//! Where each reference sits, stored as position data rather than as graph edges.
//!
//! A commit ([`Step::Pick`]) carries parent edges; a reference ([`Step::Reference`]) carries
//! NONE. Instead every reference stands in the ARRANGEMENT TABLE (`EditorGraph::arrangement`):
//! per stored key, an ordered list of groups (`RefGroup`), each an ordered bottom→top run of
//! references sharing one [`Carry`] — none of the pick's incoming edges, all of them, or an
//! explicit edge list. A reference's position is read back as a [`RefPosition`] view:
//!
//! - `on` — the node the reference points at (followed through tombstones to a live pick);
//! - `below`  — the reference directly underneath in the physical stack (`None` = on the pick):
//!   the previous member in its group, or the group's attach;
//! - `ambiguous` — whether more than one thing converged here (i.e. this position is a merge).
//!
//! Keeping references out of the edge graph is deliberate: an edge running THROUGH a reference
//! node would make the reference bear connectivity it shouldn't — gluing a commit's history onto
//! whatever else the reference happens to touch. The functions here read and maintain positions.
//!
//! Vocabulary used throughout this module:
//! - **edge** — a parent edge of the commit graph, named from its child side as
//!   `(child node, parent-slot)` — the canonical identity, since edges live in parent arrays.
//!   A commit has one parent edge per parent; a pick's INCOMING edges are its children's
//!   edges pointing at it.
//! - **enters through** — an incoming edge of a pick ENTERS THROUGH a reference when it
//!   descends into that reference's position (see [`edges_through`]). This is what
//!   distinguishes co-located references and picks out which merge group a reference belongs to.
//! - **group** — references stacked on one pick, ordered by their below walk ([`ref_depth`]).
//!   Groups are shallow in practice (≤3 observed).

use crate::graph_rebase::editor_graph::{Carry, RefPosition};
use crate::graph_rebase::{EditorGraph, EditorGraphIndex};

/// The reference's depth above its pick — the length of its below walk (0 = directly on
/// the pick). This IS the rank: order among co-located references is adjacency, not a number.
pub(crate) fn ref_depth(graph: &EditorGraph, node: EditorGraphIndex) -> usize {
    let mut depth = 0usize;
    let mut cursor = graph.position_of(node).and_then(|s| s.below);
    while let Some(b) = cursor {
        depth += 1;
        if depth > 10_000 {
            debug_assert!(false, "below walk cycle at ref {node}");
            return depth;
        }
        cursor = graph.position_of(b).and_then(|s| s.below);
    }
    depth
}

/// The edges currently entering through the reference at `node` — the DIRECT carry read: the
/// node's group carries its own edge statement, kept aligned by the slot mutators
/// (`EditorGraph::remove_parent` / `insert_parent` / `replace_parent`), ordered and filtered by
/// the resolved pick's live edges so a stale carry edge never reaches a consumer.
pub(crate) fn edges_through(
    graph: &EditorGraph,
    node: EditorGraphIndex,
) -> Vec<(EditorGraphIndex, usize)> {
    let Some(stored) = graph.position_of(node) else {
        return Vec::new();
    };
    let Some(carry) = graph.carry_of(node) else {
        return Vec::new();
    };
    let edges = match resolve_to_pick(graph, stored.on) {
        Some(pick) => edges_into(graph, pick),
        None => Vec::new(),
    };
    match carry {
        Carry::None => Vec::new(),
        Carry::All => edges,
        Carry::Edges(stated) => edges
            .into_iter()
            .filter(|edge| stated.contains(edge))
            .collect(),
    }
}

/// Every reference that RESOLVES to `pick` — its stored `on`, followed through tombstones,
/// ends at it. Order is unspecified (ascending node id), like the node-walking predecessor.
pub(crate) fn refs_resolving_to(
    graph: &EditorGraph,
    pick: EditorGraphIndex,
) -> Vec<EditorGraphIndex> {
    graph
        .positioned_refs()
        .filter_map(|(node, stored)| {
            (resolve_to_pick(graph, stored.on) == Some(pick)).then_some(node)
        })
        .collect()
}

/// The standing collapse invariant: every reference in the graph has a well-formed position,
/// and positions are unique wherever order is topologically meaningful — i.e. within groups
/// entered by a child edge (a non-empty [`edges_through`]). Parallel ROOT groups above one pick are legitimate
/// unordered siblings (found by this very assert on its first corpus run): with nothing above
/// them, their relative order is not defined by topology — the collapse orders them like the
/// passive set (by name).
///
/// Wired at editor creation AND at rebase entry, so every graph shape the suite produces —
/// including post-mutation shapes — continuously validates the position model.
pub(crate) fn debug_assert_positions_total(graph: &EditorGraph) {
    if !cfg!(debug_assertions) {
        return;
    }
    debug_assert_below_wellformed(graph);
    type OrderedPositionKey = (
        Option<EditorGraphIndex>,
        Vec<(EditorGraphIndex, usize)>,
        usize,
    );
    let mut seen: std::collections::HashMap<OrderedPositionKey, EditorGraphIndex> =
        Default::default();
    for node in graph.references().map(|(node, _, _)| node) {
        // A reference without a stored position is only legitimate when the graph holds no
        // pick below it at creation (unborn); it resolves to nothing.
        let Some(stored) = graph.position_of(node) else {
            continue;
        };
        let entering = edges_through(graph, node);
        if entering.is_empty() {
            continue;
        }
        let pick = resolve_to_pick(graph, stored.on);
        let rank = ref_depth(graph, node);
        if let Some(previous) = seen.insert((pick, entering.clone(), rank), node) {
            debug_assert!(
                false,
                "reference nodes {previous} and {node} collide at position \
                 (pick {pick:?}, entering {entering:?}, rank {rank})"
            );
        }
    }
}

/// Every stored `below` of a LIVE reference names a positioned reference resolving to the SAME
/// pick, and the below walk is acyclic. Tombstoned refs keep their stored position for
/// retention reads but are spliced out of the physical stack, so only live refs are graded.
fn debug_assert_below_wellformed(graph: &EditorGraph) {
    let name = |node: EditorGraphIndex| match graph.reference(node) {
        Some((refname, _)) => refname.to_string(),
        None => format!("{:?}", graph.step_view(node)),
    };
    for (node, stored) in graph.positioned_refs() {
        if !graph.is_reference(node) {
            continue;
        }
        let pick = resolve_to_pick(graph, stored.on);
        let mut depth = 0usize;
        let mut cursor = stored.below;
        while let Some(b) = cursor {
            let Some(mate) = graph.position_of(b) else {
                debug_assert!(false, "ref {node}: below {b} is not a positioned reference");
                return;
            };
            debug_assert_eq!(
                resolve_to_pick(graph, mate.on),
                pick,
                "ref {node} ({}): below {b} ({}) resolves to a different pick",
                name(node),
                name(b)
            );
            depth += 1;
            debug_assert!(depth <= 10_000, "ref {node}: below walk cycle");
            if depth > 10_000 {
                return;
            }
            cursor = mate.below;
        }
    }
}

/// The references the node-era traversal from `start` would have walked through, given the
/// PICK set it reached: a group is entered when one of its edges was visited (the edge from
/// edge to group top), and when `start` is itself a reference, it and its group below count.
pub(crate) fn refs_reachable_with(
    graph: &EditorGraph,
    start: EditorGraphIndex,
    picks: &std::collections::HashSet<EditorGraphIndex>,
) -> Vec<EditorGraphIndex> {
    // Reached commits by ID as well as node: a graph can hold one commit twice (a stack group
    // and a target group), and the node era's shared reference nodes made reachability
    // commit-equivalent across such groups.
    let reached_ids: std::collections::HashSet<gix::ObjectId> = picks
        .iter()
        .filter_map(|node| graph.commit_id(*node))
        .collect();
    let mut out = Vec::new();
    for (node, stored) in graph.positioned_refs() {
        // A group whose pick is reached lies on reached history — pick-based
        // reachability, exactly what the node-era walk through interposed reference nodes
        // computed (and the ruling the merge-bypass deletion rests on).
        let pick_reached = resolve_to_pick(graph, stored.on).is_some_and(|pick| {
            picks.contains(&pick)
                || graph
                    .commit_id(pick)
                    .is_some_and(|id| reached_ids.contains(&id))
        });
        if pick_reached || node == start {
            out.push(node);
        }
    }
    out
}

/// A group about to be entered by a new edge, captured BEFORE that edge exists — while
/// the store is still consistent — so [`apply_group_join`] never reads a half-updated store.
pub(crate) struct GroupJoin {
    /// The joining members: the reference and the group-mates its below walk rests on. Root
    /// groups (no entering edges) at one pick are distinct siblings, so only the reference
    /// itself joins.
    members: Vec<(EditorGraphIndex, RefPosition)>,
    /// The edges entering the group at capture time.
    entering: Vec<(EditorGraphIndex, usize)>,
}

/// Capture `ref_node`'s group for a coming join — call BEFORE the joining edge is added.
pub(crate) fn prepare_group_join(graph: &EditorGraph, ref_node: EditorGraphIndex) -> GroupJoin {
    let Some(stored) = graph.position_of(ref_node) else {
        return GroupJoin {
            members: Vec::new(),
            entering: Vec::new(),
        };
    };
    let is_root = matches!(graph.carry_of(ref_node), Some(Carry::None));
    let members = if is_root {
        vec![(ref_node, stored.clone())]
    } else {
        // The reference plus the group-mates underneath it: walk the below walk, keeping
        // members of this group (the physical stack may pass through other groups' refs).
        let group = group_members(graph, ref_node);
        let mut members = vec![(ref_node, stored.clone())];
        let mut cursor = stored.below;
        while let Some(b) = cursor {
            let Some(m) = graph.position_of(b) else {
                break;
            };
            if group.iter().any(|(node, _)| *node == b) {
                members.push((b, m.clone()));
            }
            cursor = m.below;
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
    graph: &mut EditorGraph,
    join: &GroupJoin,
    edge: (EditorGraphIndex, usize),
) {
    for (node, member) in &join.members {
        let mut entering = join.entering.clone();
        if !entering.contains(&edge) {
            entering.push(edge);
        }
        let ambiguous = member.ambiguous || entering.len() > 1;
        graph.set_position(*node, member.on, &entering, ambiguous, member.below);
    }
}

/// Move every reference resolving to `from_pick` onto `to_pick`.
///
/// With `reclassify` false the kind is PRESERVED (an `All` group top follows onto `to_pick`
/// and derives its edges there — the bridged edge set a deletion's re-point restores, robust to the
/// reconnect renumbering the edge's slot). With `reclassify` true the ref's current derived edges
/// are re-classified against `to_pick`'s edges, so a ref sliding onto a dup-parent MERGE base splits
/// into the `Edges` group its edge occupies. `ambiguous` is preserved. NOTE: preserve-vs-reclassify is
/// per-situation, not cleanly per-caller — each call site picks based on whether the edge set
/// should survive the move or be re-derived at the destination.
pub(crate) fn reposition_refs(
    graph: &mut EditorGraph,
    from_pick: EditorGraphIndex,
    to_pick: EditorGraphIndex,
    reclassify: bool,
) {
    let moves: Vec<_> = graph
        .positioned_refs()
        .filter_map(|(node, stored)| {
            (resolve_to_pick(graph, stored.on) == Some(from_pick)).then_some((node, stored))
        })
        .collect();
    for (node, stored) in moves {
        if reclassify {
            let entering = edges_through(graph, node);
            graph.set_position(node, to_pick, &entering, stored.ambiguous, stored.below);
        } else {
            graph.rekey_position(node, to_pick);
        }
    }
}

/// The members of `ref_node`'s group — every reference with the same resolved pick and the
/// same (derived) entering edges — with their stored positions.
pub(crate) fn group_members(
    graph: &EditorGraph,
    ref_node: EditorGraphIndex,
) -> Vec<(
    EditorGraphIndex,
    crate::graph_rebase::editor_graph::RefPosition,
)> {
    let Some(stored) = graph.position_of(ref_node) else {
        return vec![];
    };
    let pick = resolve_to_pick(graph, stored.on);
    let entering = edges_through(graph, ref_node);
    graph
        .positioned_refs()
        .filter_map(|(node, other)| {
            (edges_through(graph, node) == entering && resolve_to_pick(graph, other.on) == pick)
                .then(|| (node, other.clone()))
        })
        .collect()
}

/// A pick's incoming edges — its children's parent edges pointing at it, as
/// `(child, parent-slot)` pairs, sorted. The groups on the pick divide these among
/// themselves (their [`Carry`]); [`edges_through`] reads one group's share.
pub(crate) fn edges_into(
    graph: &EditorGraph,
    pick: EditorGraphIndex,
) -> Vec<(EditorGraphIndex, usize)> {
    graph
        .incoming_edges(pick)
        .into_iter()
        .filter(|&(child, _)| graph.is_pick(child))
        .collect()
}

/// Resolve `node` to the current pick it stands for: a pick resolves to itself, a tombstone
/// follows its (preserved) first edge downward, and a reference resolves via its stored
/// position — dead references via their RETAINED position, the retention pointer stale
/// selectors normalize through (unborn refs carry none and resolve to nothing).
pub(crate) fn resolve_to_pick(
    graph: &EditorGraph,
    node: EditorGraphIndex,
) -> Option<EditorGraphIndex> {
    let mut cursor = node;
    for _ in 0..10_000 {
        if graph.is_pick(cursor) {
            return Some(cursor);
        }
        // A reference resolves via its stored position; a tombstone follows its preserved
        // first edge downward.
        cursor = match graph.position_of(cursor) {
            Some(stored) => stored.on,
            None => graph.parents(cursor).first().copied()?,
        };
    }
    None
}
