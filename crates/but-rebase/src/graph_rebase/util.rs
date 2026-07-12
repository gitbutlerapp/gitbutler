//! Utilities around the commit graph for internal use.

use crate::graph_rebase::commits::ParentEntry;
use std::collections::HashSet;

use crate::graph_rebase::commits::CommitIndex;
use crate::graph_rebase::{EditorIndex, EditorStore};

/// Pruned depth-first search for `target`'s commit parents in parent order, descending through
/// non-commit steps.
///
/// A parent entry that carries a reference group (the parent entry enters through a group
/// positioned at its commit) yields to any plain parent resolving to the same commit, and only the
/// first of several carrying parent entries survives — when a reference path and a direct path reach
/// the same commit, that commit is listed once. Plain duplicate parents are all kept
/// (dup-parents workspace commits).
pub(crate) fn collect_ordered_parents(
    store: &EditorStore,
    target: impl Into<EditorIndex>,
) -> Vec<CommitIndex> {
    collect_ordered_parents_with_indices(store, target)
        .into_iter()
        .map(|(parent, _)| parent)
        .collect()
}

/// As [`collect_ordered_parents`], but each parent keeps the target's parent slot it came from —
/// `None` when it was flattened out of a tombstone and so has no parent index of its own. Callers that
/// must ask something about a specific parent index need this: the emitted order skips and flattens, so an
/// emitted position is not a parent index, and indexing one by the other silently misattributes.
pub(crate) fn collect_ordered_parents_with_indices(
    store: &EditorStore,
    target: impl Into<EditorIndex>,
) -> Vec<(CommitIndex, Option<usize>)> {
    let target = target.into();
    // The parent numbers whose entry is stored as entering some positioned group. An entry in a
    // group's share always enters the commit the group is positioned on, so matching the parent number's
    // parent again is redundant — one pass over the refs covers every parent number.
    let carried_numbers: HashSet<usize> = store
        .positioned_refs()
        .flat_map(|entry| crate::graph_rebase::positions::entering(store, entry))
        .filter(|&ParentEntry { child, .. }| EditorIndex::from(child) == target)
        .map(|entry| entry.number)
        .collect();
    let carries_group = |parent_number: usize, parent: CommitIndex| {
        store.is_commit(parent) && carried_numbers.contains(&parent_number)
    };
    let ordered_parents = store.parents(target);
    let plain_targets: HashSet<CommitIndex> = ordered_parents
        .iter()
        .enumerate()
        .filter(|&(parent_number, &parent)| {
            store.is_commit(parent) && !carries_group(parent_number, parent)
        })
        .map(|(_, &parent)| parent)
        .collect();
    let mut emitted_carrying = HashSet::new();

    let mut potential: Vec<(CommitIndex, bool, Option<usize>)> = ordered_parents
        .iter()
        .enumerate()
        .rev()
        .map(|(parent_number, &parent)| {
            (
                parent,
                carries_group(parent_number, parent),
                Some(parent_number),
            )
        })
        .collect();
    let mut seen = potential
        .iter()
        .map(|(t, _, _)| *t)
        .collect::<HashSet<CommitIndex>>();

    let mut parents = vec![];

    while let Some((entry, carrying, index)) = potential.pop() {
        if store.is_commit(entry) {
            if carrying && (plain_targets.contains(&entry) || !emitted_carrying.insert(entry)) {
                continue;
            }
            parents.push((entry, index));
            // Don't pursue the children
            continue;
        };

        for &parent in store.parents(entry).iter().rev() {
            if seen.insert(parent) {
                potential.push((parent, false, None));
            }
        }
    }

    parents
}

#[cfg(test)]
mod test {
    mod collect_ordered_parents {
        use std::str::FromStr as _;

        use anyhow::Result;

        use crate::graph_rebase::{CommitSpec, EditorStore, util::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut store = EditorStore::default();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = store.commits.add_commit(CommitSpec::new(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = store.commits.add_commit(CommitSpec::new(b_id));
            // Second parent - is a tombstone, so it flattens to its own parents
            let c = store.commits.add_tombstone();
            // Second parent's first parent
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = store.commits.add_commit(CommitSpec::new(d_id));
            // Second parent's second parent
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = store.commits.add_commit(CommitSpec::new(e_id));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = store.commits.add_commit(CommitSpec::new(f_id));

            // A's parents
            store.commits.push_parent(a, b);
            store.commits.push_parent(a, c);
            store.commits.push_parent(a, f);

            // C's parents
            store.commits.push_parent(c, d);
            store.commits.push_parent(c, e);

            let parents = collect_ordered_parents(&store, a);
            assert_eq!(&parents, &[b, d, e, f]);

            Ok(())
        }
    }
}
