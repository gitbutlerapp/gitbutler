use super::*;

#[test]
fn swapping_commits_also_swaps_incident_connections() {
    let mut graph = PetGraph::default();
    let a = graph.add_node(Segment::default());
    let b = graph.add_node(Segment::default());
    let before = graph.add_node(Segment::default());
    let after = graph.add_node(Segment::default());
    let edge = Edge {
        src: None,
        src_id: None,
        dst: None,
        dst_id: None,
        parent_order: 0,
    };
    graph.add_edge(before, a, edge);
    graph.add_edge(a, after, edge);
    graph.add_edge(b, before, edge);
    graph.add_edge(a, b, edge);

    swap_commits_and_connections(&mut graph, a, b);

    assert!(
        graph.find_edge(before, b).is_some(),
        "incoming connections move from a to b"
    );
    assert!(
        graph.find_edge(b, after).is_some(),
        "outgoing connections move from a to b"
    );
    assert!(
        graph.find_edge(a, before).is_some(),
        "connections move from b to a"
    );
    assert!(
        graph.find_edge(b, a).is_some(),
        "connections directly between swapped segments reverse"
    );
    assert!(
        graph.find_edge(before, a).is_none(),
        "a no longer owns its old incoming connection"
    );
    assert!(
        graph.find_edge(a, after).is_none(),
        "a no longer owns its old outgoing connection"
    );
    assert!(
        graph.find_edge(b, before).is_none(),
        "b no longer owns its old outgoing connection"
    );
}

#[test]
fn swapping_connections_preserves_untouched_sibling_parent_order() {
    let mut graph = PetGraph::default();
    let a = graph.add_node(Segment::default());
    let b = graph.add_node(Segment::default());
    let source = graph.add_node(Segment::default());
    let other_parent = graph.add_node(Segment::default());
    let edge = Edge {
        src: None,
        src_id: None,
        dst: None,
        dst_id: None,
        parent_order: 0,
    };
    graph.add_edge(
        source,
        a,
        Edge {
            parent_order: 1,
            ..edge
        },
    );
    graph.add_edge(source, other_parent, edge);

    swap_commits_and_connections(&mut graph, a, b);

    assert_eq!(
        graph
            .edges_directed(source, Direction::Outgoing)
            .map(|edge| (edge.target(), edge.weight().parent_order))
            .collect::<Vec<_>>(),
        [(other_parent, 0), (b, 1)],
        "the swapped connection stays after its untouched sibling"
    );
}

fn gtt(generation: Option<u32>, committer_time: u64) -> GenThenTime {
    GenThenTime {
        generation,
        committer_time,
    }
}

#[test]
fn gen_then_time_total_ordering_is_transitive_with_mixed_generations() {
    // This is the exact scenario that previously caused a panic:
    //   "user-provided comparison function does not correctly implement a total order"
    //
    // With the old implementation (fall back to time-only when generations are mixed):
    //   A < B (by time: 200 > 150)
    //   B < C (by time: 150 > 100)
    //   A > C (by generation: 5 > 3)  — transitivity violation!
    let a = gtt(Some(3), 200);
    let b = gtt(None, 150);
    let c = gtt(Some(5), 100);

    let ab = a.cmp(&b);
    let bc = b.cmp(&c);
    let ac = a.cmp(&c);

    // With the fix, None is treated as u32::MAX (youngest), so B sorts first.
    // B(gen=MAX) > A(gen=3) → B < A (reversed)
    // B(gen=MAX) > C(gen=5) → B < C (reversed)
    // C(gen=5) > A(gen=3)  → C < A (reversed)
    // Order: B < C < A — fully transitive.
    assert_eq!(
        ab,
        Ordering::Greater,
        "A should sort after B (B has None → u32::MAX)"
    );
    assert_eq!(
        bc,
        Ordering::Less,
        "B should sort before C (B has None → u32::MAX)"
    );
    assert_eq!(
        ac,
        Ordering::Greater,
        "A should sort after C (gen 5 > gen 3)"
    );
}

#[test]
fn gen_then_time_none_generation_treated_as_youngest() {
    // None generation maps to u32::MAX, which reversed sorts first (youngest).
    let with_gen = gtt(Some(100), 500);
    let without_gen = gtt(None, 500);
    assert_eq!(
        without_gen.cmp(&with_gen),
        Ordering::Less,
        "None generation (u32::MAX) should sort before any known generation"
    );
}

#[test]
fn gen_then_time_both_some_sorts_by_generation_then_time() {
    let young_gen = gtt(Some(10), 100);
    let old_gen = gtt(Some(2), 200);
    assert_eq!(
        young_gen.cmp(&old_gen),
        Ordering::Less,
        "Higher generation sorts first (reversed), regardless of time."
    );

    let recent = gtt(Some(5), 300);
    let old = gtt(Some(5), 100);
    assert_eq!(
        recent.cmp(&old),
        Ordering::Less,
        "Equal generation falls back to time (higher time sorts first)."
    );
}

#[test]
fn gen_then_time_both_none_sorts_by_time() {
    let recent = gtt(None, 300);
    let old = gtt(None, 100);
    assert_eq!(
        recent.cmp(&old),
        Ordering::Less,
        "Higher time sorts first (reversed)."
    );
    assert_eq!(
        gtt(None, 100).cmp(&gtt(None, 100)),
        Ordering::Equal,
        "Equal time → equal."
    );
}

#[test]
fn gen_then_time_sort_is_deterministic_and_total_issue_12343() {
    // Throw a mix of items at sort and ensure it doesn't panic.
    // This directly exercises the code path from the stack trace.
    let mut items = [
        gtt(Some(3), 200),
        gtt(None, 150),
        gtt(Some(5), 100),
        gtt(None, 300),
        gtt(Some(1), 300),
        gtt(Some(5), 200),
        gtt(None, 100),
        gtt(Some(3), 100),
    ];
    items.sort();

    // Verify the result is actually sorted (each element ≤ the next).
    for window in items.windows(2) {
        assert!(
            window[0].cmp(&window[1]) != Ordering::Greater,
            "Sort result is not ordered: {window:?}"
        );
    }
}
