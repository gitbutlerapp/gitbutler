use crate::HunkRange;
use crate::ranges::tests::id_from_hex_char;
use but_core::TreeStatusKind;
use but_workspace::StackId;

#[test]
fn test_split_hunk_range() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![HunkRange {
        change_type: TreeStatusKind::Addition,
        stack_id: StackId::generate(),
        commit_id: commit_a_id,
        start: 1,
        lines: 10,
        line_shift: 9,
    }];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 3,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 1,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 5,
            lines: 6,
            line_shift: 9,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 1, hunks, 1);

    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 3);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_a_id);

    Ok(())
}

#[test]
fn test_split_hunk_range_filter_before() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![HunkRange {
        change_type: TreeStatusKind::Addition,
        stack_id: StackId::generate(),
        commit_id: commit_a_id,
        start: 1,
        lines: 10,
        line_shift: 9,
    }];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 0,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 1,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 5,
            lines: 6,
            line_shift: 9,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 1, hunks, 1);

    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 3);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_a_id);

    Ok(())
}

#[test]
fn test_split_hunk_range_filter_after() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![HunkRange {
        change_type: TreeStatusKind::Addition,
        stack_id: StackId::generate(),
        commit_id: commit_a_id,
        start: 1,
        lines: 10,
        line_shift: 9,
    }];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 3,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 1,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 5,
            lines: 0,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 6,
            lines: 0,
            line_shift: 9,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 1, hunks, 1);

    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 4);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[0].start, 1);
    assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[1].start, 4);
    assert_eq!(hunk_ranges[2].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[2].start, 5);
    assert_eq!(hunk_ranges[3].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[3].start, 6);

    Ok(())
}

#[test]
fn test_split_hunk_range_filter_incoming() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![HunkRange {
        change_type: TreeStatusKind::Addition,
        stack_id: StackId::generate(),
        commit_id: commit_a_id,
        start: 1,
        lines: 10,
        line_shift: 9,
    }];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 0,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 3,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 0,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 5,
            lines: 6,
            line_shift: 9,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 1, hunks, 2);

    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 3);
    assert_eq!(index_after_last_added, 4);
    assert_eq!(hunk_ranges[0].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[0].start, 4);
    assert_eq!(hunk_ranges[1].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].start, 1);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].start, 4);
    assert_eq!(hunk_ranges[3].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[3].start, 5);

    Ok(())
}

#[test]
fn test_split_hunk_range_filter_all() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 10,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 11,
            lines: 10,
            line_shift: 9,
        },
    ];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 0,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 0,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 4,
            lines: 0,
            line_shift: 0,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 5,
            lines: 0,
            line_shift: 9,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 1, hunks, 2);

    assert_eq!(hunk_ranges.len(), 5);
    assert_eq!(index_after_interest, 3);
    assert_eq!(index_after_last_added, 4);
    assert_eq!(hunk_ranges[0].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[0].start, 4);
    assert_eq!(hunk_ranges[1].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].start, 1);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].start, 4);
    assert_eq!(hunk_ranges[3].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[3].start, 5);
    assert_eq!(hunk_ranges[4].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[4].start, 11);

    Ok(())
}

#[test]
fn test_split_hunk_range_filter_only() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 10,
            line_shift: 9,
        },
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 11,
            lines: 10,
            line_shift: 9,
        },
    ];

    let hunks = vec![HunkRange {
        change_type: TreeStatusKind::Addition,
        stack_id: StackId::generate(),
        commit_id: commit_b_id,
        start: 4,
        lines: 0,
        line_shift: 0,
    }];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 2, 2, hunks, 0);

    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 3);
    assert_eq!(index_after_last_added, 3);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[0].start, 1);
    assert_eq!(hunk_ranges[1].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].start, 11);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].start, 4);

    Ok(())
}

#[test]
fn test_replace_hunk_ranges_at_the_end() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: id_from_hex_char('1'),
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: id_from_hex_char('1'),
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let commit_c_id = id_from_hex_char('1');

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 2,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 4,
            lines: 1,
            line_shift: 1,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 5, hunks, 1);

    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 3);
    assert_eq!(index_after_last_added, 3);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_c_id);

    Ok(())
}

#[test]
fn test_replace_hunk_ranges_at_the_beginning() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let commit_c_id = id_from_hex_char('1');

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 0,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 2,
            lines: 1,
            line_shift: 1,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 2, hunks, 1);

    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 2);
    assert_eq!(hunk_ranges[0].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn test_insert_single_hunk_range_at_the_beginning() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![HunkRange {
        change_type: TreeStatusKind::Modification,
        stack_id: StackId::generate(),
        commit_id: commit_c_id,
        start: 0,
        lines: 1,
        line_shift: 1,
    }];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 0, 0, hunks, 0);
    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 1);
    assert_eq!(index_after_last_added, 1);
    assert_eq!(hunk_ranges[0].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[3].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn test_insert_at_the_end() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![HunkRange {
        change_type: TreeStatusKind::Modification,
        stack_id: StackId::generate(),
        commit_id: commit_c_id,
        start: 0,
        lines: 1,
        line_shift: 1,
    }];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 3, 3, hunks, 0);
    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 4);
    assert_eq!(index_after_last_added, 4);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[3].commit_id, commit_c_id);

    Ok(())
}

#[test]
fn test_replace_hunk_ranges_between_add() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 0,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 2,
            lines: 1,
            line_shift: 1,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks, 1);
    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 3);
    assert_eq!(index_after_last_added, 3);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[3].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn test_filter_out_single_range_replacement() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![HunkRange {
        change_type: TreeStatusKind::Modification,
        stack_id: StackId::generate(),
        commit_id: commit_c_id,
        start: 4,
        lines: 0,
        line_shift: 1,
    }];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks, 0);
    assert_eq!(hunk_ranges.len(), 3);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 2);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].start, 5);

    Ok(())
}

#[test]
fn test_filter_out_multiple_range_replacement() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 4,
            lines: 0,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 5,
            lines: 3,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 6,
            lines: 0,
            line_shift: 1,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks, 2);
    assert_eq!(hunk_ranges.len(), 5);
    assert_eq!(index_after_interest, 4);
    assert_eq!(index_after_last_added, 4);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[1].start, 4);
    assert_eq!(hunk_ranges[2].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].start, 5);
    assert_eq!(hunk_ranges[3].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[3].start, 6);
    assert_eq!(hunk_ranges[4].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[4].start, 5);

    Ok(())
}

#[test]
fn test_filter_out_single_range_insertion() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![HunkRange {
        change_type: TreeStatusKind::Modification,
        stack_id: StackId::generate(),
        commit_id: commit_c_id,
        start: 4,
        lines: 0,
        line_shift: 1,
    }];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 1, hunks, 0);
    assert_eq!(hunk_ranges.len(), 4);
    assert_eq!(index_after_interest, 2);
    assert_eq!(index_after_last_added, 2);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[2].start, 3);
    assert_eq!(hunk_ranges[3].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[3].start, 5);

    Ok(())
}

#[test]
fn test_filter_out_multiple_range_insertion() -> anyhow::Result<()> {
    let commit_a_id = id_from_hex_char('1');
    let commit_b_id = id_from_hex_char('1');
    let commit_c_id = id_from_hex_char('1');

    let mut hunk_ranges = vec![
        HunkRange {
            change_type: TreeStatusKind::Addition,
            stack_id: StackId::generate(),
            commit_id: commit_a_id,
            start: 1,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 3,
            lines: 1,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_b_id,
            start: 5,
            lines: 1,
            line_shift: 1,
        },
    ];

    let hunks = vec![
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 4,
            lines: 0,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 5,
            lines: 3,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 6,
            lines: 0,
            line_shift: 1,
        },
        HunkRange {
            change_type: TreeStatusKind::Modification,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 8,
            lines: 3,
            line_shift: 1,
        },
    ];

    let (index_after_interest, index_after_last_added) =
        crate::ranges::paths::insert_hunk_ranges(&mut hunk_ranges, 1, 1, hunks, 3);
    assert_eq!(hunk_ranges.len(), 7);
    assert_eq!(index_after_interest, 5);
    assert_eq!(index_after_last_added, 5);
    assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
    assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[1].start, 4);
    assert_eq!(hunk_ranges[2].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[2].start, 5);
    assert_eq!(hunk_ranges[3].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[3].start, 6);
    assert_eq!(hunk_ranges[4].commit_id, commit_c_id);
    assert_eq!(hunk_ranges[4].start, 8);
    assert_eq!(hunk_ranges[5].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[5].start, 3);
    assert_eq!(hunk_ranges[6].commit_id, commit_b_id);
    assert_eq!(hunk_ranges[6].start, 5);

    Ok(())
}
