use gitbutler_hunk_dependency::{parse_diff_from_string, HunkRange, InputDiff, PathRanges};
use gitbutler_stack::StackId;

#[test]
fn stack_simple() -> anyhow::Result<()> {
    let diff = parse_diff_from_string(
        "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_id = git2::Oid::from_str("a")?;

    stack_ranges.add(stack_id, commit_id, vec![diff])?;

    let intersection = stack_ranges.intersection(4, 1);
    assert_eq!(intersection.len(), 1);

    Ok(())
}

#[test]
fn stack_simple_update() -> anyhow::Result<()> {
    let diff = InputDiff {
        old_start: 4,
        old_lines: 1,
        new_start: 4,
        new_lines: 1,
        change_type: gitbutler_diff::ChangeType::Modified,
    };

    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_id = git2::Oid::from_str("a")?;

    stack_ranges.add(stack_id, commit_id, vec![diff])?;

    let intersection = stack_ranges.intersection(4, 1);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_id);

    Ok(())
}

#[test]
fn stack_delete_file() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,7 @@
+a
+a
+a
+a
+a
+a
+a
",
        gitbutler_diff::ChangeType::Added,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,7 +1,7 @@
a
a
a
-a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let diff_3 = parse_diff_from_string(
        "@@ -1,7 +0,0 @@
-a
-a
-a
-b
-a
-a
-a
",
        gitbutler_diff::ChangeType::Deleted,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_a_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_a_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        hunks[0],
        HunkRange {
            change_type: gitbutler_diff::ChangeType::Added,
            stack_id,
            commit_id: commit_a_id,
            start: 1,
            lines: 7,
            line_shift: 7,
        }
    );

    let commit_b_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_b_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 3);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit_a_id,
                start: 1,
                lines: 3,
                line_shift: 7
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit_b_id,
                start: 4,
                lines: 1,
                line_shift: 0
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit_a_id,
                start: 5,
                lines: 3,
                line_shift: 7
            }
        ]
    );

    let commit_c_id = git2::Oid::from_str("c")?;
    stack_ranges.add(stack_id, commit_c_id, vec![diff_3])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Deleted,
            stack_id,
            commit_id: commit_c_id,
            start: 0,
            lines: 0,
            line_shift: 0
        }]
    );

    // The file is deleted in the second commit.
    // If we recreate it, it should intersect.
    let intersection = stack_ranges.intersection(1, 1);
    assert_eq!(stack_ranges.hunk_ranges.len(), 1);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_c_id);

    Ok(())
}

#[test]
fn stack_delete_and_recreate_file() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,7 @@
+a
+a
+a
+a
+a
+a
+a
",
        gitbutler_diff::ChangeType::Added,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,7 +1,7 @@
a
a
a
-a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let diff_3 = parse_diff_from_string(
        "@@ -1,7 +0,0 @@
-a
-a
-a
-b
-a
-a
-a
",
        gitbutler_diff::ChangeType::Deleted,
    )?;
    let diff_4 = parse_diff_from_string(
        "@@ -0,0 +1,5 @@
+c
+c
+c
+c
+c
",
        gitbutler_diff::ChangeType::Added,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_a_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_a_id, vec![diff_1])?;

    let commit_b_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_b_id, vec![diff_2])?;

    let commit_c_id = git2::Oid::from_str("c")?;
    stack_ranges.add(stack_id, commit_c_id, vec![diff_3])?;

    let commit_d_id = git2::Oid::from_str("d")?;
    stack_ranges.add(stack_id, commit_d_id, vec![diff_4])?;

    // The file is deleted in the second commit.
    // If we recreate it, it should intersect.
    let intersection = stack_ranges.intersection(1, 1);
    assert_eq!(stack_ranges.hunk_ranges.len(), 1);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_d_id);

    Ok(())
}

#[test]
fn uncommitted_file_deletion() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -1,0 +1,7 @@
+a
+a
+a
+a
+a
+a
+a
",
        gitbutler_diff::ChangeType::Added,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_id, vec![diff_1])?;

    // If the file is completely deleted, the old start and lines are 1 and 7.
    let intersection = stack_ranges.intersection(1, 7);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_id);

    Ok(())
}

#[test]
fn stack_overwrite_file() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,7 @@
+1
+2
+3
+4
+5
+6
+7
",
        gitbutler_diff::ChangeType::Added,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,7 +1,7 @@
-1
-2
-3
-4
-5
-6
-7
+a
+b
+c
+d
+e
+f
+g
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_a_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_a_id, vec![diff_1])?;

    let commit_b_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_b_id, vec![diff_2])?;

    let intersection = stack_ranges.intersection(1, 1);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn stack_overwrite_line() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,7 +1,7 @@
1
2
3
-4
+4.5
5
6
7
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();
    let commit_a_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_a_id, vec![diff_1])?;

    let commit_b_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_b_id, vec![diff_2])?;

    let intersection = stack_ranges.intersection(3, 3);
    assert_eq!(intersection.len(), 1);
    assert_eq!(intersection[0].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn stack_complex() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -2,6 +2,7 @@
2
3
4
+4.5
5
6
7
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_id, vec![diff_1])?;

    let commit_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_id, vec![diff_2])?;

    let intersection = stack_ranges.intersection(4, 1);
    assert_eq!(intersection.len(), 1);

    let intersection = stack_ranges.intersection(5, 1);
    assert_eq!(intersection.len(), 1);

    let intersection = stack_ranges.intersection(4, 2);
    assert_eq!(intersection.len(), 2);

    Ok(())
}

#[test]
fn stack_basic_line_shift() -> anyhow::Result<()> {
    let diff_1 = parse_diff_from_string(
        "@@ -1,4 +1,5 @@
a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,3 +1,4 @@
+c
a
b
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit_a_id = git2::Oid::from_str("a")?;
    stack_ranges.add(stack_id, commit_a_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit_a_id,
            start: 2,
            lines: 1,
            line_shift: 1
        }]
    );

    let commit_b_id = git2::Oid::from_str("b")?;
    stack_ranges.add(stack_id, commit_b_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 2);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit_b_id,
                start: 1,
                lines: 1,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit_a_id,
                start: 3,
                lines: 1,
                line_shift: 1
            }
        ]
    );

    let result = stack_ranges.intersection(1, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit_b_id);

    Ok(())
}

#[test]
fn stack_complex_line_shift() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff1 = parse_diff_from_string(
        "@@ -1,4 +1,5 @@
a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let commit2_id = git2::Oid::from_str("b")?;
    let diff2 = parse_diff_from_string(
        "@@ -1,3 +1,4 @@
+c
a
b
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let commit3_id = git2::Oid::from_str("c")?;
    let diff3 = parse_diff_from_string(
        "@@ -1,4 +1,3 @@
-c
-a
+b
b
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let commit4_id = git2::Oid::from_str("d")?;
    let diff4 = parse_diff_from_string(
        "@@ -1,3 +1,5 @@
b
b
+added
+added
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let commit5_id = git2::Oid::from_str("e")?;
    let diff5 = parse_diff_from_string(
        "@@ -1,5 +1,6 @@
b
-b
-added
+c
+c
+c
added
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let commit6_id = git2::Oid::from_str("f")?;
    // Delete the first line
    let diff6 = InputDiff {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 0,
        change_type: gitbutler_diff::ChangeType::Modified,
    };

    // commit 1
    stack_ranges.add(stack_id, commit1_id, vec![diff1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 1,
            line_shift: 1
        }]
    );

    // commit 2
    stack_ranges.add(stack_id, commit2_id, vec![diff2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 2);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 1,
                lines: 1,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 3,
                lines: 1,
                line_shift: 1
            }
        ]
    );

    // commit 3
    stack_ranges.add(stack_id, commit3_id, vec![diff3])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 2);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit3_id,
                start: 1,
                lines: 1,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 1,
                line_shift: 1
            }
        ]
    );

    // commit 4
    stack_ranges.add(stack_id, commit4_id, vec![diff4])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 3);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit3_id,
                start: 1,
                lines: 1,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 1,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit4_id,
                start: 3,
                lines: 2,
                line_shift: 2
            }
        ]
    );

    // commit 5
    stack_ranges.add(stack_id, commit5_id, vec![diff5])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(hunks.len(), 3);
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit3_id,
                start: 1,
                lines: 1,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit5_id,
                start: 2,
                lines: 3,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit4_id,
                start: 5,
                lines: 1,
                line_shift: 2
            }
        ]
    );

    // commit 6
    stack_ranges.add(stack_id, commit6_id, vec![diff6])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit6_id,
                start: 1,
                lines: 0,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit5_id,
                start: 1,
                lines: 3,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit4_id,
                start: 4,
                lines: 1,
                line_shift: 2
            }
        ]
    );

    let result = stack_ranges.intersection(1, 1);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].commit_id, commit6_id);
    assert_eq!(result[1].commit_id, commit5_id);

    let result = stack_ranges.intersection(2, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit5_id);

    let result = stack_ranges.intersection(4, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit4_id);

    let result = stack_ranges.intersection(5, 1);
    assert_eq!(result.len(), 0);

    Ok(())
}

#[test]
fn stack_multiple_overwrites() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,7 @@
+a
+a
+a
+a
+a
+a
+a
",
        gitbutler_diff::ChangeType::Added,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

    let commit2_id = git2::Oid::from_str("b")?;
    let diff2 = parse_diff_from_string(
        "@@ -1,5 +1,5 @@
a
-a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff2])?;

    let commit3_id = git2::Oid::from_str("c")?;
    let diff3 = parse_diff_from_string(
        "@@ -1,7 +1,7 @@
a
b
a
-a
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit3_id, vec![diff3])?;

    let commit4_id = git2::Oid::from_str("d")?;
    let diff4 = parse_diff_from_string(
        "@@ -3,5 +3,5 @@
a
b
a
-a
+b
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit4_id, vec![diff4])?;

    let result = stack_ranges.intersection(1, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit1_id);

    let result = stack_ranges.intersection(2, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit2_id);

    let result = stack_ranges.intersection(4, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit3_id);

    let result = stack_ranges.intersection(6, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit4_id);

    Ok(())
}

#[test]
fn stack_detect_deletion() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -1,7 +1,6 @@
a
a
a
-a
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

    let result = stack_ranges.intersection(3, 2);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit1_id);

    Ok(())
}

#[test]
fn stack_offset_and_split() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -10,6 +10,9 @@
a
a
a
+b
+b
+b
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,6 +1,9 @@
a
a
a
+c
+c
+c
a
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;

    let commit3_id = git2::Oid::from_str("c")?;
    let diff_3 = parse_diff_from_string(
        "@@ -14,7 +14,7 @@
a
a
b
-b
+d
b
a
a
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit3_id, vec![diff_3])?;

    assert_eq!(stack_ranges.intersection(4, 3)[0].commit_id, commit2_id);
    assert_eq!(stack_ranges.intersection(15, 1).len(), 0);
    assert_eq!(stack_ranges.intersection(16, 1)[0].commit_id, commit1_id);
    assert_eq!(stack_ranges.intersection(17, 1)[0].commit_id, commit3_id);
    assert_eq!(stack_ranges.intersection(18, 1)[0].commit_id, commit1_id);
    assert_eq!(stack_ranges.intersection(19, 1).len(), 0);

    Ok(())
}

#[test]
fn create_file_update_and_trim() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,9 @@
+a
+b
+c
+d
+e
+f
+g
+h
+i",
        gitbutler_diff::ChangeType::Added,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Added,
            stack_id,
            commit_id: commit1_id,
            start: 1,
            lines: 9,
            line_shift: 9
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -7,3 +7,0 @@
-g
-h
-i",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 1,
                lines: 6,
                line_shift: 9
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 7,
                lines: 0,
                line_shift: -3
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 7,
                lines: 0,
                line_shift: 9
            }
        ]
    );

    let commit3_id = git2::Oid::from_str("c")?;
    let diff_3 = parse_diff_from_string(
        "@@ -1,1 +1,1 @@
-a
+1",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit3_id, vec![diff_3])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit3_id,
                start: 1,
                lines: 1,
                line_shift: 0
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 5,
                line_shift: 0
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 7,
                lines: 0,
                line_shift: -3
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 7,
                lines: 0,
                line_shift: 9
            }
        ]
    );

    Ok(())
}

#[test]
fn adding_line_splits_range() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,2 @@
-1
-1
+a
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 2,
            line_shift: 0
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -2,0 +3,1 @@
+b
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 1,
                line_shift: 0
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 3,
                lines: 1,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 4,
                lines: 1,
                line_shift: 0
            },
        ]
    );

    Ok(())
}

#[test]
fn adding_line_before_shifts_range() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,2 @@
-1
-1
+a
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 2,
            line_shift: 0
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,0 +2,1 @@
+b
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 2,
                lines: 1,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 3,
                lines: 2,
                line_shift: 0
            },
        ]
    );

    Ok(())
}

#[test]
fn adding_line_after_shifts_range() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,2 @@
-1
-1
+a
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 2,
            line_shift: 0
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -3,0 +4,1 @@
+b
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 2,
                line_shift: 0
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 4,
                lines: 1,
                line_shift: 1
            },
        ]
    );

    Ok(())
}

#[test]
fn removing_line_updates_range() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,3 @@
-1
-1
+a
+b
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 3,
            line_shift: 1
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -3,1 +2,0 @@
-b
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 0,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 2,
                lines: 0,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 2,
                line_shift: 1
            }
        ]
    );

    Ok(())
}

#[test]
fn removing_line_before_shifts_range() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,3 @@
-1
-1
+a
+b
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 3,
            line_shift: 1
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -1,1 +1,0 @@
-start
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 1,
                lines: 0,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 1,
                lines: 3,
                line_shift: 1
            }
        ]
    );

    Ok(())
}

#[test]
fn removing_line_after_is_ignored() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -2,2 +2,3 @@
-1
-1
+a
+b
+c
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id,
            commit_id: commit1_id,
            start: 2,
            lines: 3,
            line_shift: 1
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -5,1 +4,0 @@
-end
",
        gitbutler_diff::ChangeType::Modified,
    )?;
    stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit1_id,
                start: 2,
                lines: 3,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 4,
                lines: 0,
                line_shift: -1
            },
        ]
    );

    Ok(())
}

#[test]
fn shift_is_correct_after_multiple_changes() -> anyhow::Result<()> {
    let stack_ranges = &mut PathRanges::default();
    let stack_id = StackId::generate();

    let commit1_id = git2::Oid::from_str("a")?;
    let diff_1 = parse_diff_from_string(
        "@@ -0,0 +1,10 @@
+1
+2
+3
+4
+5
+6
+7
+8
+9
+10
",
        gitbutler_diff::ChangeType::Added,
    )?;
    stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Added,
            stack_id,
            commit_id: commit1_id,
            start: 1,
            lines: 10,
            line_shift: 10
        }]
    );

    let commit2_id = git2::Oid::from_str("b")?;
    let diff_2 = parse_diff_from_string(
        "@@ -3,1 +3,4 @@
-3
+ update 3
+ add line 1
+ add line 2
+ add line 4
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let diff_3 = parse_diff_from_string(
        "@@ -5,1 +7,0 @@
-5
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let diff_4 = parse_diff_from_string(
        "@@ -7,1 +9,2 @@
-7
+ update 7
+ add line
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    let diff_5 = parse_diff_from_string(
        "@@ -11,0 +14,3 @@
+ added
+ lines
+ at the bottom
",
        gitbutler_diff::ChangeType::Modified,
    )?;

    stack_ranges.add(stack_id, commit2_id, vec![diff_2, diff_3, diff_4, diff_5])?;
    let hunks = &stack_ranges.hunk_ranges;
    assert_eq!(
        hunks,
        &vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 1,
                lines: 2,
                line_shift: 10
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 3,
                lines: 4,
                line_shift: 3
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 7,
                lines: 0,
                line_shift: 10
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 7,
                lines: 0,
                line_shift: -1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 7,
                lines: 2,
                line_shift: 10
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 9,
                lines: 2,
                line_shift: 1
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id,
                commit_id: commit1_id,
                start: 11,
                lines: 3,
                line_shift: 10
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id,
                commit_id: commit2_id,
                start: 14,
                lines: 3,
                line_shift: 3
            },
        ]
    );

    Ok(())
}
