//! Utilities around the commit graph for internal use.

use std::collections::HashSet;

use crate::graph_rebase::{EditorGraph, EditorGraphIndex};

/// Pruned depth-first search for `target`'s commit parents in parent order, descending through
/// non-commit steps.
///
/// A parent slot that carries a reference chain (the slot is a stored edge entry of a chain
/// positioned at its pick) yields to any plain slot resolving to the same pick, and only the
/// first of several carrying slots survives — the same collapse the node-era search produced
/// when a ref path and a direct path reached one pick. Plain duplicate slots are all kept
/// (dup-parents workspace commits).
pub(crate) fn collect_ordered_parents(
    graph: &EditorGraph,
    target: EditorGraphIndex,
) -> Vec<EditorGraphIndex> {
    let carries_chain = |slot: usize, parent: EditorGraphIndex| {
        graph.is_pick(parent)
            && graph.positioned_refs().any(|(node, stored)| {
                crate::graph_rebase::positions::edges_through(graph, node).contains(&(target, slot))
                    && crate::graph_rebase::positions::resolve_to_pick(graph, stored.on)
                        == Some(parent)
            })
    };
    let slot_parents = graph.parents(target);
    let plain_targets: HashSet<EditorGraphIndex> = slot_parents
        .iter()
        .enumerate()
        .filter(|&(slot, &parent)| graph.is_pick(parent) && !carries_chain(slot, parent))
        .map(|(_, &parent)| parent)
        .collect();
    let mut emitted_carrying = HashSet::new();

    let mut potential: Vec<(EditorGraphIndex, bool)> = slot_parents
        .iter()
        .enumerate()
        .rev()
        .map(|(slot, &parent)| (parent, carries_chain(slot, parent)))
        .collect();
    let mut seen = potential
        .iter()
        .map(|(t, _)| *t)
        .collect::<HashSet<EditorGraphIndex>>();

    let mut parents = vec![];

    while let Some((node, carrying)) = potential.pop() {
        if graph.is_pick(node) {
            if carrying && (plain_targets.contains(&node) || !emitted_carrying.insert(node)) {
                continue;
            }
            parents.push(node);
            // Don't pursue the children
            continue;
        };

        for &parent in graph.parents(node).iter().rev() {
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

        use crate::graph_rebase::{EditorGraph, Step, util::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = EditorGraph::default();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            // Second parent - is a tombstone, so it flattens to its own parents
            let c = graph.add_node(Step::None);
            // Second parent's first child
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::new_pick(d_id));
            // Second parent's second child
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::new_pick(e_id));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::new_pick(f_id));

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
