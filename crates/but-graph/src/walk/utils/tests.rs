use super::*;

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
