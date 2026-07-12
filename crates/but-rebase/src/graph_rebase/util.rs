//! Utilities around the commit graph for internal use.

use std::collections::HashSet;

use crate::graph_rebase::graph_editor::PickIndex;
use crate::graph_rebase::{EditorIndex, GraphEditor};

/// Pruned depth-first search for `target`'s commit parents in parent order, descending through
/// non-commit steps.
///
/// A parent edge that carries a reference group (the edge is a stored entry of a group
/// positioned at its pick) yields to any plain parent resolving to the same pick, and only the
/// first of several carrying edges survives — when a reference path and a direct path reach
/// the same pick, that pick is listed once. Plain duplicate parents are all kept
/// (dup-parents workspace commits).
pub(crate) fn collect_ordered_parents(
    graph: &GraphEditor,
    target: impl Into<EditorIndex>,
) -> Vec<PickIndex> {
    let target = target.into();
    // The parent numbers whose edge is a stored edge entry of some positioned group. An edge in a
    // group's share always enters the pick the group is positioned on, so matching the parent number's
    // parent again is redundant — one pass over the refs covers every parent number.
    let carried_numbers: HashSet<usize> = graph
        .positioned_refs()
        .flat_map(|entry| crate::graph_rebase::positions::edges_through(graph, entry))
        .filter(|&(child, _)| EditorIndex::from(child) == target)
        .map(|(_, parent_number)| parent_number)
        .collect();
    let carries_group = |parent_number: usize, parent: PickIndex| {
        graph.is_pick(parent) && carried_numbers.contains(&parent_number)
    };
    let ordered_parents = graph.parents(target);
    let plain_targets: HashSet<PickIndex> = ordered_parents
        .iter()
        .enumerate()
        .filter(|&(parent_number, &parent)| {
            graph.is_pick(parent) && !carries_group(parent_number, parent)
        })
        .map(|(_, &parent)| parent)
        .collect();
    let mut emitted_carrying = HashSet::new();

    let mut potential: Vec<(PickIndex, bool)> = ordered_parents
        .iter()
        .enumerate()
        .rev()
        .map(|(parent_number, &parent)| (parent, carries_group(parent_number, parent)))
        .collect();
    let mut seen = potential
        .iter()
        .map(|(t, _)| *t)
        .collect::<HashSet<PickIndex>>();

    let mut parents = vec![];

    while let Some((entry, carrying)) = potential.pop() {
        if graph.is_pick(entry) {
            if carrying && (plain_targets.contains(&entry) || !emitted_carrying.insert(entry)) {
                continue;
            }
            parents.push(entry);
            // Don't pursue the children
            continue;
        };

        for &parent in graph.parents(entry).iter().rev() {
            if seen.insert(parent) {
                potential.push((parent, false));
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

        use crate::graph_rebase::{GraphEditor, Pick, util::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = GraphEditor::default();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Some(Pick::new_pick(a_id)));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Some(Pick::new_pick(b_id)));
            // Second parent - is a tombstone, so it flattens to its own parents
            let c = graph.add_node(None);
            // Second parent's first parent
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Some(Pick::new_pick(d_id)));
            // Second parent's second parent
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Some(Pick::new_pick(e_id)));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Some(Pick::new_pick(f_id)));

            // A's parents
            graph.push_parent(a, b);
            graph.push_parent(a, c);
            graph.push_parent(a, f);

            // C's parents
            graph.push_parent(c, d);
            graph.push_parent(c, e);

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[b, d, e, f]);

            Ok(())
        }
    }
}
