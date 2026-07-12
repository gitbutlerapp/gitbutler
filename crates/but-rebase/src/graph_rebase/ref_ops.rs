//! GitButler's extension, the write side: every function here mutates structure
//! vanilla git cannot represent. A plain-git editor would keep only one resident:
//! [`repoint_ref`] is `git branch -f` in extension clothing — vanilla semantics,
//! implemented through the position machinery like every other degenerate case.
//! Its only caller is the parent-surgery wiring, and no repoint verb exists at all:
//! position-following retired the need — refs follow rewrites by standing still, and
//! deliberate movement is `move_reference`. The rest of the module a plain-git editor
//! would not need.
//!
//! The reference-op API: mutation sites say where a reference should end up
//! ([`RefPlace`]) and let [`place_ref`] and friends work out the `(on, below, group)`
//! details.
//!
//! These ops read and write the layout table: per commit, an ordered list of groups of ref
//! names — the shape of workspace metadata — with commit, rank, and entering parent entries derived
//! from the table plus live commit parent entries (`positioned_on`, `ref_depth`, `entering`).
//! Names never churn under graph mutation, which is what keeps the table stable while
//! commits are rewritten around it.
//!
//! The module rule, the mirror of `positions`': every layout write lives here — each
//! function takes `&mut EditorStore`. The reads and the checks live in `positions`.
//!
//! # The rider rules
//!
//! Which references follow which surgery is product policy, not mechanics — stated here
//! because this module's writes implement it:
//!
//! - The vanilla default costs zero code: refs stay put while ids rewrite underneath.
//! - Interposing a commit above another lifts every ref standing there onto the
//!   newcomer ([`reposition_refs`], [`Carry::Preserve`]).
//! - Moving a range leaves ordinary refs behind on the healed lineage; a worktree's
//!   checked-out branch follows the commit its worktree stands on
//!   ([`reposition_refs_except`]'s `keep_seated`).
//! - Removing a commit lands the refs standing on it on the heal target.

use crate::graph_rebase::EditorStore;
use crate::graph_rebase::commits::{CommitIndex, ParentEntry};
use crate::graph_rebase::positions;
use crate::graph_rebase::store::RefIndex;

/// A destination for a reference among the groups on a commit, named by intent.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RefPlace {
    /// Directly above this reference in its group: mates that sat on it re-hang onto the
    /// newcomer.
    Above(RefIndex),
    /// At this reference's position: the newcomer takes its below, and it re-hangs onto the
    /// newcomer.
    Below(RefIndex),
    /// The bottom of the commit's whole stack (carrying all its parent entries — "the branch here");
    /// every reference that sat on the commit itself re-hangs onto the newcomer.
    Bottom(CommitIndex),
    /// The top of the group the parent entry `(child, parent number)` carries into `commit`.
    GroupTop {
        /// The commit the group sits on.
        commit: CommitIndex,
        /// The child parent entry whose group the reference stacks onto.
        entry: ParentEntry,
    },
    /// A fresh root above `commit`: nothing descends into it, no other position moves.
    Root(CommitIndex),
}

/// Re-hang the references sitting directly ON `commit` (no `below` of their own) onto
/// `moving`, which is taking the commit-bottom spot; `moving` itself is left alone.
fn rehang_bottom(store: &mut EditorStore, commit: CommitIndex, moving: RefIndex) {
    let rehang: Vec<_> = store
        .positioned_refs()
        .filter(|&mate| {
            mate != moving
                && store.below_of(mate).is_none()
                && store.resolve_to_commit(mate) == Some(commit)
        })
        .collect();
    for mate in rehang {
        store.set_below(mate, Some(moving));
    }
}

/// The topmost reference on `commit` entered through exactly `entering` — the one a
/// group-top placement stacks above; `moving` itself is not a candidate.
fn group_top_at(
    store: &EditorStore,
    commit: CommitIndex,
    entering: &[ParentEntry],
    moving: RefIndex,
) -> Option<RefIndex> {
    store
        .positioned_refs()
        .filter(|&mate| {
            mate != moving
                && store.resolve_to_commit(mate) == Some(commit)
                && positions::entering(store, mate) == entering
        })
        .max_by_key(|&mate| (positions::ref_depth(store, mate), mate))
}

/// Place the reference at `entry` into `place`, shifting other positions as the placement demands.
/// Fresh placement only — moving an existing reference is [`move_ref`].
pub(crate) fn place_ref(store: &mut EditorStore, entry: RefIndex, place: RefPlace) {
    match place {
        RefPlace::Above(target) => {
            if !store.is_positioned(target) {
                return;
            }
            // Same-group members that sat directly on the target now sit on the
            // interposed entry; cross-group members on the target keep it (they branch).
            let rehang: Vec<_> = positions::group_members(store, target)
                .into_iter()
                .filter(|&mate| mate != entry && store.below_of(mate) == Some(target))
                .collect();
            for mate in rehang {
                store.set_below(mate, Some(entry));
            }
            store.join_group_of(entry, target, Some(target));
        }
        RefPlace::Below(target) => {
            if !store.is_positioned(target) {
                return;
            }
            let below = store.below_of(target);
            store.join_group_of(entry, target, below);
            store.set_below(target, Some(entry));
        }
        RefPlace::Bottom(commit) => {
            let entering = positions::live_children_of(store, commit);
            // Members that sat on the commit itself now sit on the new bottom.
            rehang_bottom(store, commit, entry);
            store.set_position(entry, commit, &entering, entering.len() > 1, None);
        }
        RefPlace::GroupTop {
            commit,
            entry: entering,
        } => {
            let entering = vec![entering];
            let top = group_top_at(store, commit, &entering, entry);
            store.set_position(entry, commit, &entering, false, top);
        }
        RefPlace::Root(commit) => {
            store.set_position(entry, commit, &[], false, None);
        }
    }
}

/// Move the reference at `entry` into `place`. Unlike [`place_ref`], the reference already
/// holds a position: when it is the sole carrier of its entering parent entries, those follow it
/// (re-pointed onto the new commit and merged into the destination's entering set), and — when
/// moving above another reference — the members now entered through it share the merged set.
pub(crate) fn move_ref(store: &mut EditorStore, entry: RefIndex, place: RefPlace) {
    let Some(moving_on) = store.positioned_on(entry) else {
        return;
    };
    let moving_edges = positions::entering(store, entry);
    // Sole carrier = nothing in the group sits below the mover. Measured before any shuffling
    // (the shuffles never change which members those are).
    let moving_depth = positions::ref_depth(store, entry);
    let sole_carrier = !positions::group_members(store, entry)
        .into_iter()
        .any(|mate| mate != entry && positions::ref_depth(store, mate) < moving_depth);
    // The mover vacates its old spot: members stacked directly on it settle onto what it sat
    // on.
    store.splice(entry);
    // Each arm yields (on, entering, below); the mover's parent entries merge into `entering` below,
    // after re-pointing, so the final `set_position` sees the complete set. Each arm hangs the
    // mover on its new below first so re-hanging mates never creates a transient below-cycle
    // through its stale pointer.
    let (on, mut entering, below) = match place {
        RefPlace::Above(target) => {
            let Some(t_on) = store.positioned_on(target) else {
                return;
            };
            store.set_below(entry, Some(target));
            // Same-group members that sat directly on the target now sit on the mover.
            let rehang: Vec<_> = positions::group_members(store, target)
                .into_iter()
                .filter(|&mate| mate != entry && store.below_of(mate) == Some(target))
                .collect();
            for mate in rehang {
                store.set_below(mate, Some(entry));
            }
            (t_on, positions::entering(store, target), Some(target))
        }
        RefPlace::Below(target) => {
            let Some(t_on) = store.positioned_on(target) else {
                return;
            };
            let t_below = store.below_of(target);
            store.set_below(entry, t_below);
            store.set_below(target, Some(entry));
            (t_on, positions::entering(store, target), t_below)
        }
        RefPlace::Bottom(commit) => {
            // Refs that sat on the commit itself re-hang onto the mover.
            store.set_below(entry, None);
            rehang_bottom(store, commit, entry);
            (commit, Vec::new(), None)
        }
        RefPlace::GroupTop {
            commit,
            entry: entering,
        } => {
            let entering = vec![entering];
            let top = group_top_at(store, commit, &entering, entry);
            store.set_below(entry, top);
            (commit, entering, top)
        }
        RefPlace::Root(commit) => {
            store.set_below(entry, None);
            (commit, Vec::new(), None)
        }
    };
    // The parent entries follow the mover only when it was their sole carrier: group members staying
    // behind keep their entering parent entries.
    let old_resolved = store.resolve_to_commit(moving_on);
    let new_resolved = store.resolve_to_commit(on);
    if sole_carrier && let (Some(old_pick), Some(new)) = (old_resolved, new_resolved) {
        redirect_entries(store, &moving_edges, old_pick, new);
        for &entry in &moving_edges {
            if !entering.contains(&entry) {
                entering.push(entry);
            }
        }
        entering.sort();
        // Members below in the joined group are now entered through the moved reference:
        // they share the merged entry set.
        if let RefPlace::Above(target) = place
            && store.is_positioned(target)
        {
            let t_depth = positions::ref_depth(store, target);
            let mates: Vec<_> = positions::group_members(store, target)
                .into_iter()
                .filter(|&mate| mate != entry && positions::ref_depth(store, mate) <= t_depth)
                .filter_map(|mate| {
                    store
                        .positioned_on(mate)
                        .map(|on| (mate, on, store.below_of(mate)))
                })
                .collect();
            for (mate, m_on, m_below) in mates {
                store.set_position(mate, m_on, &entering, entering.len() > 1, m_below);
            }
        }
    }
    store.set_position(entry, on, &entering, entering.len() > 1, below);
}

/// Re-point the captured `parent entries` from `from` onto `to`. Each parent entry keeps its parent number, so the
/// caller's captured coordinates stay exact, and its stable id, so groups stated on it follow it to
/// its new target. Entries already rewired elsewhere are left alone.
pub(crate) fn redirect_entries(
    store: &mut EditorStore,
    entries: &[ParentEntry],
    from: CommitIndex,
    to: CommitIndex,
) {
    if from == to {
        return;
    }
    for &ParentEntry {
        child,
        number: parent_number,
    } in entries
    {
        if store.parents(child).get(parent_number) == Some(&from) {
            store.commits.replace_parent(child, parent_number, to);
        }
    }
}

/// The mate a reference landing at `depth` on `on` (resolved) sits on — the member at
/// `depth - 1`, excluding `exclude`, lowest entry id on a tie.
fn mate_below_depth(
    store: &EditorStore,
    exclude: RefIndex,
    on: CommitIndex,
    depth: usize,
) -> Option<RefIndex> {
    if depth == 0 {
        return None;
    }
    let commit = store.resolve_to_commit(on)?;
    store
        .positioned_refs()
        .filter(|&mate| {
            mate != exclude
                && store.resolve_to_commit(mate) == Some(commit)
                && positions::ref_depth(store, mate) + 1 == depth
        })
        .min()
}

/// Re-point the reference at `entry` at the commit `onto` — `git update-ref` as a position
/// move. Its entering parent entries and the members stacked above move with it; members below lose
/// their entering parent entries and become roots at the old commit. An unplaced reference is placed as
/// a fresh root; one already resolving there just refreshes its stored `on`.
pub(crate) fn repoint_ref(store: &mut EditorStore, entry: RefIndex, onto: CommitIndex) {
    if !store.is_positioned(entry) {
        place_ref(store, entry, RefPlace::Root(onto));
        return;
    }
    match store.resolve_to_commit(entry) {
        Some(old_pick) if old_pick != onto => {
            // Snapshot the entering parent entries before re-pointing them — the derived read tracks
            // live parent entries.
            let entering = positions::entering(store, entry);
            redirect_entries(store, &entering, old_pick, onto);
            // At the destination the reference sits on whatever holds the depth below it
            // there, or directly on the commit when that stack doesn't exist.
            let below = mate_below_depth(store, entry, onto, positions::ref_depth(store, entry));
            // Carried = the below-subtree stacked on the reference. Depth-tied siblings and
            // the walk underneath stay at the old commit, though group mates lose their entering
            // parent entries (those move with `entry`) and become roots there.
            let mut carried = vec![entry];
            let mut i = 0;
            while i < carried.len() {
                let current = carried[i];
                i += 1;
                let dependents: Vec<_> = store
                    .positioned_refs()
                    .filter(|&mate| {
                        store.below_of(mate) == Some(current)
                            && !carried.contains(&mate)
                            && store.is_reference(mate)
                    })
                    .collect();
                carried.extend(dependents);
            }
            let mates: Vec<_> = positions::group_members(store, entry)
                .into_iter()
                .filter(|&mate| mate != entry && !carried.contains(&mate))
                .filter_map(|mate| {
                    store
                        .positioned_on(mate)
                        .map(|on| (mate, on, store.below_of(mate)))
                })
                .collect();
            for (mate, m_on, m_below) in mates {
                store.set_position(mate, m_on, &[], false, m_below);
            }
            for &mate in &carried[1..] {
                store.rekey_position(mate, onto);
            }
            // Re-classify against `onto`'s final parent entries — the old `Entries` statement may not
            // exist there.
            let ambiguous = store.ambiguous_of(entry);
            store.set_position(entry, onto, &entering, ambiguous, below);
        }
        _ => {
            store.rekey_position(entry, onto);
        }
    }
}

/// Remove the reference at `entry` from its group: members above close the gap and it becomes
/// a root at its current commit. With `drop_edges` the parent entries that entered through it are removed
/// outright; otherwise they stay on the commit for a follow-up reconnect to rewire.
pub(crate) fn unhook_ref(store: &mut EditorStore, entry: RefIndex, drop_edges: bool) {
    let Some(unhooked_on) = store.positioned_on(entry) else {
        return;
    };
    let unhooked_below = store.below_of(entry);
    // Mates that sat on the unhooked reference settle onto what it sat on.
    let rehang: Vec<_> = positions::group_members(store, entry)
        .into_iter()
        .filter(|&mate| mate != entry && store.below_of(mate) == Some(entry))
        .collect();
    for mate in rehang {
        store.set_below(mate, unhooked_below);
    }
    if drop_edges && let Some(commit) = store.resolve_to_commit(entry) {
        let mut entries = positions::entering(store, entry);
        entries.sort_unstable();
        // Descending parent numbers per child: a removal shifts only the parent numbers above it, so every
        // pending `(child, parent number)` name below stays exact.
        for ParentEntry {
            child,
            number: parent_number,
        } in entries.into_iter().rev()
        {
            if store.parents(child).get(parent_number) == Some(&commit) {
                store.remove_parent(child, parent_number);
            }
        }
    }
    store.set_position(entry, unhooked_on, &[], false, unhooked_below);
}

/// Move the stack slice led by `lead_ref` — it and its below-subtree in its group on
/// `source_pick` — onto `dest`: the lead lands at the bottom, each member re-classifies
/// against its own parent entries at the destination, and stored ambiguity is preserved.
pub(crate) fn transfer_stack(
    store: &mut EditorStore,
    lead_ref: RefIndex,
    source_pick: CommitIndex,
    dest: CommitIndex,
) {
    if !store.is_positioned(lead_ref) {
        return;
    }
    let lead_entering = positions::entering(store, lead_ref);
    let mut moves = vec![lead_ref];
    let mut i = 0;
    while i < moves.len() {
        let current = moves[i];
        i += 1;
        let dependents: Vec<_> = store
            .positioned_refs()
            .filter(|&entry| {
                store.below_of(entry) == Some(current)
                    && !moves.contains(&entry)
                    && store.resolve_to_commit(entry) == Some(source_pick)
                    && positions::entering(store, entry) == lead_entering
            })
            .collect();
        moves.extend(dependents);
    }
    for entry in moves {
        if !store.is_positioned(entry) {
            continue;
        }
        let entering = positions::entering(store, entry);
        // The lead's old below stays behind; the rest keeps its internal stacking.
        let below = (entry != lead_ref).then(|| store.below_of(entry)).flatten();
        let ambiguous = store.ambiguous_of(entry);
        store.set_position(entry, dest, &entering, ambiguous, below);
    }
}

/// Carry the slice of the group identified by `entering` on `source_pick` strictly above
/// depth `above_depth` onto `dest` verbatim — same depths, same kinds; only the `on` key
/// changes. The delimiter below the slice stays behind. `entering`/`above_depth` are
/// caller-captured (pre-mutation) coordinates, not live derivations.
pub(crate) fn carry_stack_above(
    store: &mut EditorStore,
    source_pick: CommitIndex,
    entering: &[ParentEntry],
    above_depth: usize,
    dest: CommitIndex,
) {
    let moves: Vec<_> = store
        .positioned_refs()
        .filter(|&entry| {
            store.resolve_to_commit(entry) == Some(source_pick)
                && positions::entering(store, entry) == entering
                && positions::ref_depth(store, entry) > above_depth
        })
        .collect();
    for &entry in &moves {
        store.rekey_position(entry, dest);
    }
    // The slice bottom sat on the delimiter left behind; at the destination it sits on
    // whatever holds the depth below it there.
    for &entry in &moves {
        if store.below_of(entry).is_some_and(|b| !moves.contains(&b)) {
            let mate = mate_below_depth(store, entry, dest, positions::ref_depth(store, entry));
            store.set_below(entry, mate);
        }
    }
}

/// Stack every reference on `source_pick` above `top` (a reference on another commit), the
/// whole tower re-placed behind `bridge_pick`'s full incoming parent entry set — the bridged parent entries
/// now descending into the joined group. Returns false (graph untouched) when `top` holds
/// no position.
pub(crate) fn land_stack_above(
    store: &mut EditorStore,
    source_pick: CommitIndex,
    top: RefIndex,
    bridge_pick: CommitIndex,
) -> bool {
    let Some(top_on) = store.positioned_on(top) else {
        return false;
    };
    let bridge = positions::live_children_of(store, bridge_pick);
    let top_depth = positions::ref_depth(store, top);
    let top_below = store.below_of(top);
    store.set_position(top, top_on, &bridge, bridge.len() > 1, top_below);

    let mut moves: Vec<_> = store
        .positioned_refs()
        .filter(|&entry| store.resolve_to_commit(entry) == Some(source_pick))
        .map(|entry| {
            (
                entry,
                positions::ref_depth(store, entry),
                store.below_of(entry),
            )
        })
        .collect();
    // Land bottom-up: each member's captured below is then already in place, and a
    // bottom member's mate lookup can't commit a tower member that hasn't landed yet —
    // out-of-order landing let a bottom ref hang onto its own upper neighbour and
    // closed the below chain into a contradiction (fuzz seed 55).
    moves.sort_by_key(|&(_, depth, _)| depth);
    for (entry, depth, below) in moves {
        // Bottom members (they sat on the source commit) now sit on whatever holds the depth
        // below their landing spot — `top` when it lives on the bridge commit, its stand-in
        // there otherwise.
        let below =
            below.or_else(|| mate_below_depth(store, entry, bridge_pick, depth + top_depth + 1));
        store.set_position(entry, bridge_pick, &bridge, bridge.len() > 1, below);
    }
    true
}

/// Re-key every reference whose `on` no longer resolves (it sat on removed commits) onto
/// `onto`, positions carried verbatim — dangling references follow where the commit's
/// place went; their entering parent entries stay.
pub(crate) fn readopt_dangling_refs(store: &mut EditorStore, onto: CommitIndex) {
    let dangling: Vec<_> = store
        .positioned_refs()
        .filter(|&entry| store.resolve_to_commit(entry).is_none())
        .collect();
    for entry in dangling {
        store.rekey_position(entry, onto);
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

/// The result of splitting a group around an interposed commit.
pub(crate) struct GroupSplit {
    /// The members left behind, with their pre-split `(on, below)` — settle them with
    /// [`settle_group_lower`] once the parent entry entering the lower part is known.
    pub lower: Vec<(RefIndex, CommitIndex, Option<RefIndex>)>,
    /// The moved member that landed at the bottom of the upper side (below cleared). The
    /// caller re-hangs it on the upper commit's chain top after its parent entry surgery settles —
    /// only then do carries resolve against final parent entries.
    pub boundary: Option<RefIndex>,
    /// Every member that moved onto the upper entry (the boundary and its riders).
    pub moved: Vec<RefIndex>,
}

/// After a split whose moved slice landed on `landing_commit`: the slice may land where
/// the range's own ref chain already stands — hang the split boundary on that chain's
/// top so both form one stack. This must run after the entering entries are redirected,
/// and it is timing that keeps it alive, not statement staleness: the split places the
/// slice before the graph parent entries settle, so no fold at placement can unify the
/// resolve-equal chains — deleting this under stable parent entry ids still collides
/// (A/B 2026-07-23: mixed-ops seed 41 + managed-moves seed 48, rank-0 position
/// collisions).
pub(crate) fn rehang_split_boundary(
    store: &mut EditorStore,
    split: &GroupSplit,
    landing_commit: CommitIndex,
) {
    let Some(boundary) = split.boundary else {
        return;
    };
    let top_below = store
        .positioned_refs()
        .filter(|&entry| store.is_reference(entry))
        .filter(|entry| !split.moved.contains(entry))
        .filter(|&entry| store.resolve_to_commit(entry) == Some(landing_commit))
        .filter(|&entry| {
            // Never hang the boundary onto a chain that already stands ON it — that
            // closes the below chain into a contradiction and two references share one
            // rank (fuzz seed 55: the moved slice's own members re-hung onto the
            // boundary are not valid tops for it).
            let mut cursor = Some(entry);
            let mut hops = 0usize;
            while let Some(c) = cursor {
                if c == boundary {
                    return false;
                }
                hops += 1;
                if hops > 10_000 {
                    break;
                }
                cursor = store.below_of(c);
            }
            true
        })
        .max_by_key(|&entry| positions::ref_depth(store, entry));
    if let Some(top) = top_below {
        store.set_below(boundary, Some(top));
    }
}

/// Split the group at `at_ref` around a commit interposed into it: members on the upper side of
/// `boundary` re-key onto `upper` (carry kinds verbatim, boundary member at the bottom); the
/// lower members are returned untouched for the caller to settle.
pub(crate) fn split_group(
    store: &mut EditorStore,
    at_ref: RefIndex,
    boundary: SplitBoundary,
    upper: CommitIndex,
) -> GroupSplit {
    if !store.is_positioned(at_ref) {
        return GroupSplit {
            lower: Vec::new(),
            boundary: None,
            moved: Vec::new(),
        };
    }
    let members: Vec<_> = positions::group_members(store, at_ref)
        .into_iter()
        .filter_map(|entry| {
            store
                .positioned_on(entry)
                .map(|on| (entry, on, store.below_of(entry)))
        })
        .collect();
    // The upper side is the below-subtree on the moving side — depth-tied siblings from other
    // stacks can share the group's entering parent entries but hang elsewhere and stay.
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
    let mut boundary = None;
    for (entry, on, below) in members {
        if moved.contains(&entry) {
            store.rekey_position(entry, upper);
            if below.is_none_or(|b| !moved.contains(&b)) {
                // The boundary member lands at the bottom; its old below stays on the
                // lower side.
                boundary_below = below;
                boundary = Some(entry);
                store.set_below(entry, None);
            }
        } else {
            lower.push((entry, on, below));
        }
    }
    // References stacked on the moved slice but not moving with it (cross-group roots, e.g. a
    // remote above the moved tip) settle onto what the slice sat on.
    let stranded: Vec<_> = store
        .positioned_refs()
        .filter(|&entry| {
            !moved.contains(&entry) && store.below_of(entry).is_some_and(|b| moved.contains(&b))
        })
        .collect();
    for entry in stranded {
        store.set_below(entry, boundary_below);
    }
    GroupSplit {
        lower,
        boundary,
        moved,
    }
}

/// Settle the lower part of a split group: each member keeps its `on` and stacking but is now
/// entered through `parent entry` — the parent entry descending from the interposed commit.
pub(crate) fn settle_group_lower(
    store: &mut EditorStore,
    lower: &[(RefIndex, CommitIndex, Option<RefIndex>)],
    entering: ParentEntry,
) {
    for &(entry, on, below) in lower {
        store.set_position(entry, on, &[entering], false, below);
    }
}

/// The new parent entry enters the captured group: every member gains it among its
/// entering entries, classified against the commit's now-complete parent list — call right
/// after the parent is added (capture with `positions::prepare_group_join` before). An
/// `All` group stays `All`; an `Entries` group gains the entry; a Root descends.
pub(crate) fn apply_group_join(
    store: &mut EditorStore,
    join: &positions::GroupJoin,
    entering_entry: ParentEntry,
) {
    for &(entry, on, below, was_ambiguous) in &join.members {
        let mut entering = join.entering.clone();
        if !entering.contains(&entering_entry) {
            entering.push(entering_entry);
        }
        let ambiguous = was_ambiguous || entering.len() > 1;
        store.set_position(entry, on, &entering, ambiguous, below);
    }
}

/// What happens to a moved reference's group-carry at its destination — the one
/// decision [`reposition_refs`] asks of its caller. It never affects where the
/// reference ends up: both variants land it on the destination commit, and the rebase
/// derives ref targets from position alone. The carry is the layer git cannot
/// represent — which parent entries enter through the group — and the choice is about
/// keeping that statement honest across the move. Per-situation, not cleanly
/// per-caller: preserve when the move keeps the statement true, re-derive when the
/// move itself invalidates it. `ambiguous` is preserved either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Carry {
    /// The record moves verbatim: an `All` group derives its parent entries afresh at
    /// the destination, robust to a reconnect renumbering parent numbers.
    Preserve,
    /// The currently derived parent entries are re-classified against the
    /// destination's, so a ref sliding onto a dup-parent merge base splits into the
    /// `Entries` group its parent entry occupies.
    Reclassify,
}

/// Move every reference resolving to `from_commit` onto `to_commit`; `carry` says what
/// their group-carry does there.
///
/// One statement per reference: placement writes the annotation and the
/// vanilla fact (`on`) together, so there is no separate vanilla write to remember —
/// and no window in which the two could disagree.
pub(crate) fn reposition_refs(
    store: &mut EditorStore,
    from_commit: CommitIndex,
    to_commit: CommitIndex,
    carry: Carry,
) {
    reposition_refs_except(store, from_commit, to_commit, carry, &[]);
}

/// [`reposition_refs`], except that references in `keep_seated` hold their position —
/// a worktree's checked-out branch follows the commit its worktree stands on, while
/// ordinary references stay behind in the lineage.
pub(crate) fn reposition_refs_except(
    store: &mut EditorStore,
    from_commit: CommitIndex,
    to_commit: CommitIndex,
    carry: Carry,
    keep_seated: &[RefIndex],
) {
    let moves = positions::refs_resolving_to(store, from_commit);
    for entry in moves {
        if keep_seated.contains(&entry) {
            continue;
        }
        match carry {
            Carry::Reclassify => {
                let entering = positions::entering(store, entry);
                let (ambiguous, below) = (store.ambiguous_of(entry), store.below_of(entry));
                store.set_position(entry, to_commit, &entering, ambiguous, below);
            }
            Carry::Preserve => store.rekey_position(entry, to_commit),
        }
    }
}
