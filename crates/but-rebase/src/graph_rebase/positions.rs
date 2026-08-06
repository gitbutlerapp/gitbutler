//! GitButler's extension, the read side: everything in this module is about structure
//! vanilla git cannot represent — group order, carries, statements — and the checks that
//! hold it honest. The vanilla reads live elsewhere, on the record and the store:
//! `RefState::on` is the fact (name to commit), and `EditorStore::resolve_to_commit` /
//! `positioned_on` are the joins that read it without touching this module's table.
//!
//! Where each reference sits, stored as position data rather than as graph parent entries.
//!
//! A commit carries parent entries; a reference carries
//! None. Instead every reference stands in the layout table (`EditorStore::layout`):
//! per stored key, an ordered list of groups (`RefGroup`), each a bottom→top run of references
//! sharing one [`GroupCarry`]. A position reads back through the editor's accessors: `on`
//! ([`EditorStore::positioned_on`], the stored key it stands on — a commit, or its tombstone
//! after deletion; [`resolve_to_commit`] follows it down), `below` ([`EditorStore::below_of`], the
//! reference directly underneath; `None` = on the commit), and `ambiguous`
//! ([`EditorStore::ambiguous_of`], this position is a merge).
//!
//! The module rule: reads and checks only — every function here takes `&EditorStore`.
//! Anything that mutates the store (takes `&mut`) is a layout write and belongs in
//! `ref_ops`, whatever vocabulary its contract is written in.
//!
//! Keeping references out of the parent entry graph is deliberate: a parent entry running through a reference
//! would make it bear connectivity it shouldn't — gluing a commit's history onto whatever else
//! the reference happens to touch.
//!
//! Vocabulary used throughout this module and `ref_ops`:
//! - **parent entry** — one entry of a commit's parent list, named from the child side as
//!   `(child, parent number)`, since parent lists live in their child. A commit's incoming
//!   entries are its children's entries pointing at it.
//! - **enters through** — an incoming entry of a commit enters through a reference when it
//!   descends into that reference's position (see [`entering`]). This distinguishes
//!   co-located references and commits out which merge group a reference belongs to.
//! - **group** — references stacked on one commit, ordered by their below walk ([`ref_depth`]).
//!   Groups are shallow in practice (≤3 observed).
//! - **carry** — a group's claim over incoming entries
//!   ([`GroupCarry`]): `None` (a root group, nothing descends into it), `All` (every entry
//!   into the commit, present and future), or `Entries` (exactly the listed ones).
//! - **statement** — one listed entry of an `Entries` carry, naming a parent entry by its
//!   stable id: it follows the entry through renumbering and re-pointing, and dies with it —
//!   an unrelated entry later reusing the same coordinates cannot revive it.
//! - **rider** — a group member standing above another reference. Riders keep their
//!   entering statements verbatim when the ground moves, which is why surgery must move the
//!   underlying entries with them or the graph silently bypasses an interposed commit.
//! - **settle** (`ref_ops::settle_group_lower`) — re-anchor a split-off lower group slice
//!   onto the entry that now enters it.
//! - **land** (`ref_ops::land_stack_above`) — hang a carried stack above a known top
//!   reference, so it follows that top through later moves.
//! - **mint** — a workspace-parent entry that exists only in the stack declaration (an
//!   empty lane), never written as real ancestry; see `EditorStore::ws_minted_parents`.

use crate::graph_rebase::commits::{CommitIndex, ParentEntry};
use crate::graph_rebase::store::GroupCarry;
use crate::graph_rebase::store::RefIndex;
use crate::graph_rebase::{EditorIndex, EditorStore};

/// A commit's incoming parent entries as sorted `(child, parent number)` pairs. The groups on the commit
/// divide these among themselves (their [`GroupCarry`]); [`entering`] reads one group's share.
pub(crate) fn live_children_of(store: &EditorStore, commit: CommitIndex) -> Vec<ParentEntry> {
    store
        .children_of(commit)
        .iter()
        .copied()
        .filter(|&ParentEntry { child, .. }| store.is_commit(child))
        .collect()
}

/// The parent entries currently entering through the reference at `entry`: the group's own carry
/// statement (kept aligned by the parent number mutators), ordered and filtered by the resolved commit's
/// live parent entries so a stale carry parent entry never reaches a consumer.
pub(crate) fn entering(store: &EditorStore, entry: impl Into<EditorIndex>) -> Vec<ParentEntry> {
    let Some(entry) = entry.into().as_ref() else {
        return Vec::new();
    };
    let Some(on) = store.positioned_on(entry) else {
        return Vec::new();
    };
    let Some(carry) = store.carry_of(entry) else {
        return Vec::new();
    };
    let entries = match store.resolve_to_commit(on) {
        Some(commit) => live_children_of(store, commit),
        None => Vec::new(),
    };
    match carry {
        GroupCarry::None => Vec::new(),
        GroupCarry::All => entries,
        GroupCarry::Entries(stated) => entries
            .into_iter()
            .filter(|&ParentEntry { child, number }| {
                store
                    .commits
                    .entry_id_at(child, number)
                    .is_some_and(|id| stated.contains(&id))
            })
            .collect(),
    }
}

/// The members of `ref_node`'s group — every reference with the same resolved commit and the
/// same (derived) entering parent entries.
pub(crate) fn group_members(
    store: &EditorStore,
    ref_node: impl Into<EditorIndex>,
) -> Vec<RefIndex> {
    let Some(ref_node) = ref_node.into().as_ref() else {
        return vec![];
    };
    if !store.is_positioned(ref_node) {
        return vec![];
    }
    let commit = store.resolve_to_commit(ref_node);
    let entering_here = entering(store, ref_node);
    store
        .positioned_refs()
        .filter(|&entry| {
            entering(store, entry) == entering_here && store.resolve_to_commit(entry) == commit
        })
        .collect()
}

/// Does `entry` enter a positioned group that resolves to `commit` — i.e. does it reach
/// the commit through a reference's group rather than plainly?
pub(crate) fn enters_group_resolving_to(
    store: &EditorStore,
    entry: ParentEntry,
    commit: CommitIndex,
) -> bool {
    store
        .positioned_refs()
        .any(|r| entering(store, r).contains(&entry) && store.resolve_to_commit(r) == Some(commit))
}

/// Every reference whose stored `on`, followed through tombstones, resolves to `commit`.
/// Order is unspecified (ascending entry id).
pub(crate) fn refs_resolving_to(store: &EditorStore, commit: CommitIndex) -> Vec<RefIndex> {
    store
        .positioned_refs()
        .filter(|&entry| store.resolve_to_commit(entry) == Some(commit))
        .collect()
}

/// The references reachable from `start`, given the commit set it reached. When `start` is
/// itself a reference, it counts too.
pub(crate) fn refs_reachable_with(
    store: &EditorStore,
    start: EditorIndex,
    commits: &std::collections::HashSet<CommitIndex>,
) -> Vec<RefIndex> {
    // Match by commit id as well as entry: a graph can hold the same commit twice (in a
    // stack and in the target's history), and a reference counts as reached when its commit
    // was reached under either entry. Deleting a branch that merges back in relies on this.
    let reached_ids: std::collections::HashSet<gix::ObjectId> = commits
        .iter()
        .filter_map(|entry| store.commit_id(*entry))
        .collect();
    let mut out = Vec::new();
    for entry in store.positioned_refs() {
        // Node-based reachability, commit-equivalent across duplicate groups.
        let commit_reached = store.resolve_to_commit(entry).is_some_and(|commit| {
            commits.contains(&commit)
                || store
                    .commit_id(commit)
                    .is_some_and(|id| reached_ids.contains(&id))
        });
        if commit_reached || EditorIndex::from(entry) == start {
            out.push(entry);
        }
    }
    out
}

/// The reference's depth above its commit — the length of its below walk (0 = directly on
/// the commit). This IS the rank: order among co-located references is adjacency, not a number.
pub(crate) fn ref_depth(store: &EditorStore, entry: impl Into<EditorIndex>) -> usize {
    let Some(entry) = entry.into().as_ref() else {
        return 0;
    };
    let mut depth = 0usize;
    let mut cursor = store.below_of(entry);
    while let Some(b) = cursor {
        depth += 1;
        if depth > 10_000 {
            debug_assert!(false, "below walk cycle at ref {entry}");
            return depth;
        }
        cursor = store.below_of(b);
    }
    depth
}

/// A group about to be entered by a new parent entry, captured before that parent entry exists so
/// `ref_ops::apply_group_join` never reads a half-updated store.
pub(crate) struct GroupJoin {
    /// The joining members: the reference and the group-mates its below walk rests on —
    /// each with its position captured (`on`, `below`, `ambiguous`). Root groups (no
    /// entering parent entries) at one commit are distinct siblings, so only the reference itself
    /// joins.
    pub(crate) members: Vec<(RefIndex, CommitIndex, Option<RefIndex>, bool)>,
    /// The parent entries entering the group at capture time.
    pub(crate) entering: Vec<ParentEntry>,
}

/// Capture `ref_node`'s group for a coming join — call before the joining parent entry is added.
pub(crate) fn prepare_group_join(store: &EditorStore, ref_node: RefIndex) -> GroupJoin {
    let capture = |entry: RefIndex| {
        store
            .positioned_on(entry)
            .map(|on| (entry, on, store.below_of(entry), store.ambiguous_of(entry)))
    };
    let Some(captured) = capture(ref_node) else {
        return GroupJoin {
            members: Vec::new(),
            entering: Vec::new(),
        };
    };
    let is_root = matches!(store.carry_of(ref_node), Some(GroupCarry::None));
    let members = if is_root {
        vec![captured]
    } else {
        // Walk the below walk keeping members of this group — the physical stack may pass
        // through other groups' refs.
        let group = group_members(store, ref_node);
        let mut members = vec![captured];
        let mut cursor = store.below_of(ref_node);
        while let Some(b) = cursor {
            if !store.is_positioned(b) {
                break;
            }
            if group.contains(&b) {
                members.extend(capture(b));
            }
            cursor = store.below_of(b);
        }
        members
    };
    GroupJoin {
        members,
        entering: entering(store, ref_node),
    }
}

/// Every reference has a well-formed position, and positions are unique wherever order
/// matters topologically — within groups entered by a child parent entry (non-empty
/// [`entering`]). Several root groups above one commit are fine: they have no meaningful
/// order, and display sorts them by name.
///
/// Wired at editor creation and at rebase entry, so every graph shape the suite produces —
/// including post-mutation shapes — continuously validates the position model.
pub(crate) fn assert_positions_total(store: &EditorStore) -> anyhow::Result<()> {
    assert_below_wellformed(store)?;
    assert_table_annotates_on(store)?;
    type OrderedPositionKey = (Option<CommitIndex>, Vec<ParentEntry>, usize);
    let mut seen: std::collections::HashMap<OrderedPositionKey, RefIndex> = Default::default();
    for entry in store.references().map(|(entry, _, _)| entry) {
        // No stored position is only legitimate for unborn refs (no commit below at creation).
        if !store.is_positioned(entry) {
            continue;
        }
        let entering = entering(store, entry);
        if entering.is_empty() {
            continue;
        }
        let commit = store.resolve_to_commit(entry);
        let rank = ref_depth(store, entry);
        if let Some(previous) = seen.insert((commit, entering.clone(), rank), entry) {
            let name = |entry: RefIndex| match store.reference(entry.into()) {
                Some((refname, _)) => refname.to_string(),
                None => "removed".to_string(),
            };
            let groups = commit.and_then(|p| store.groups_at_for_debug(p));
            anyhow::bail!(
                "BUG: references {previous} ({}) and {entry} ({}) collide at position \
                 (commit {commit:?}, entering {entering:?}, rank {rank})\ngroups: {groups:#?}",
                name(previous),
                name(entry)
            );
        }
    }
    Ok(())
}

/// Every stored `below` of a live reference names a positioned reference resolving to the same
/// commit, and the below walk is acyclic. Tombstoned refs keep their stored position for
/// retention reads but are spliced out of the physical stack, so only live refs are graded.
/// The extension's table annotates the vanilla fact, never contradicts it: the site key
/// the (slow) full-table scan finds for every reference equals the record's `on` —
/// `None` on both sides for the unplaced. `locate` trusts the fact, so this clause is
/// what licenses it; under the sparse-overlay design this is the rule that inverts into
/// "absent from the table means vanilla". Agreement holds by construction — every
/// writer of `on` (`extract`, `place`, `insert_groups`) writes fact and annotation
/// together — so this check exists to catch a writer that forgets, not a caller that
/// skipped a step.
fn assert_table_annotates_on(store: &EditorStore) -> anyhow::Result<()> {
    for (entry, name, key) in store.ref_positions_for_check() {
        let scanned = store
            .locate_by_scan_for_check(name.as_ref())
            .map(|(k, ..)| k);
        if key != scanned {
            anyhow::bail!(
                "table annotation contradicts the vanilla fact for {name} ({entry}): record says {key:?}, table says {scanned:?}"
            );
        }
    }
    Ok(())
}

fn assert_below_wellformed(store: &EditorStore) -> anyhow::Result<()> {
    let name = |entry: RefIndex| match store.reference(entry.into()) {
        Some((refname, _)) => refname.to_string(),
        None => "removed".to_string(),
    };
    for entry in store.positioned_refs() {
        if !store.is_reference(entry) {
            continue;
        }
        let commit = store.resolve_to_commit(entry);
        let mut depth = 0usize;
        let mut cursor = store.below_of(entry);
        while let Some(b) = cursor {
            if !store.is_positioned(b) {
                anyhow::bail!("BUG: ref {entry}: below {b} is not a positioned reference");
            }
            if store.resolve_to_commit(b) != commit {
                anyhow::bail!(
                    "BUG: ref {entry} ({}): below {b} ({}) resolves to a different commit",
                    name(entry),
                    name(b)
                );
            }
            depth += 1;
            if depth > 10_000 {
                anyhow::bail!("BUG: ref {entry}: below walk cycle");
            }
            cursor = store.below_of(b);
        }
    }
    Ok(())
}
