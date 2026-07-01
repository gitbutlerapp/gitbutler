use but_core::{TreeStatusKind, ref_metadata::StackId, unified_diff::DiffHunk};
use gix::bstr::BString;

use crate::{
    InputCommit, InputDiffHunk, InputStack,
    input::InputFile,
    ranges::{
        WorkspaceRanges,
        tests::{id_from_hex_char, input_hunk_from_unified_diff},
    },
    ui::HunkLockTarget,
};

#[test]
fn workspace_simple() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let commit2_id = id_from_hex_char('1');
    let stack2_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![
        InputStack {
            target: stack1_id,
            commits_from_base_to_tip: vec![InputCommit {
                commit_id: commit1_id,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![InputDiffHunk {
                        old_start: 2,
                        old_lines: 1,
                        new_start: 2,
                        new_lines: 1,
                    }],
                }],
            }],
        },
        InputStack {
            target: stack2_id,
            commits_from_base_to_tip: vec![InputCommit {
                commit_id: commit2_id,
                files: vec![InputFile {
                    change_type: TreeStatusKind::Modification,
                    path: path.clone(),
                    hunks: vec![
                        input_hunk_from_unified_diff(
                            "@@ -6,8 +6,6 @@

6
7
8
-9
-10
11
12
13
",
                        )?,
                        input_hunk_from_unified_diff(
                            "@@ -14,6 +12,7 @@
14
15
16
+17
18
19
20
",
                        )?,
                    ],
                }],
            }],
        },
    ])?;

    let dependencies_1 = workspace_ranges.intersection_at(&path, 2, 1).unwrap();
    assert_eq!(dependencies_1.len(), 1);
    assert_eq!(dependencies_1[0].commit_id, commit1_id);
    assert_eq!(dependencies_1[0].target, stack1_id);

    let dependencies_2 = workspace_ranges.intersection_at(&path, 9, 1).unwrap();
    assert_eq!(dependencies_2.len(), 1);
    assert_eq!(dependencies_2[0].commit_id, commit2_id);
    assert_eq!(dependencies_2[0].target, stack2_id);

    let dependencies_3 = workspace_ranges.intersection_at(&path, 15, 1).unwrap();
    assert_eq!(dependencies_3.len(), 1);
    assert_eq!(dependencies_3[0].commit_id, commit2_id);
    assert_eq!(dependencies_3[0].target, stack2_id);

    Ok(())
}

#[test]
fn overlapping_commits_in_a_stack() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let commit2_id = id_from_hex_char('2');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let commit3_id = id_from_hex_char('3');
    let stack2_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![
        InputStack {
            target: stack1_id,
            commits_from_base_to_tip: vec![
                InputCommit {
                    commit_id: commit1_id,
                    files: vec![InputFile {
                        path: path.clone(),
                        change_type: TreeStatusKind::Modification,
                        hunks: vec![input_hunk_from_unified_diff(
                            "@@ -1,4 +1,9 @@
1
+P1
+P2
+P3
+P4
+P5
2
3
4
",
                        )?],
                    }],
                },
                InputCommit {
                    commit_id: commit2_id,
                    files: vec![InputFile {
                        path: path.clone(),
                        change_type: TreeStatusKind::Modification,
                        hunks: vec![input_hunk_from_unified_diff(
                            "@@ -1,6 +1,7 @@
1
P1
P2
+Q1
P3
P4
P5
",
                        )?],
                    }],
                },
            ],
        },
        InputStack {
            target: stack2_id,
            commits_from_base_to_tip: vec![InputCommit {
                commit_id: commit3_id,
                files: vec![InputFile {
                    change_type: TreeStatusKind::Modification,
                    path: path.clone(),
                    hunks: vec![input_hunk_from_unified_diff(
                        "@@ -3,6 +3,7 @@
3
4
5
+R1
6
7
8
",
                    )?],
                }],
            }],
        },
    ])?;

    {
        // According to stack2, R1 is on line 6. Then, stack1 added 6 lines
        // before that, so R1 should now be on line 12.
        let dependencies = workspace_ranges.intersection_at(&path, 12, 1).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].commit_id, commit3_id);
        assert_eq!(dependencies[0].target, stack2_id);
    }

    Ok(())
}

#[test]
fn intersection_with_addition_change_type() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack1_id,
        commits_from_base_to_tip: vec![InputCommit {
            commit_id: commit1_id,
            files: vec![InputFile {
                path: path.clone(),
                change_type: TreeStatusKind::Addition,
                hunks: vec![InputDiffHunk {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 5,
                }],
            }],
        }],
    }])?;

    // For additions, any intersection query should return the hunk
    let dependencies = workspace_ranges.intersection_at(&path, 1, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit1_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Addition);

    // Query outside the hunk range should still return it for additions
    let dependencies = workspace_ranges.intersection_at(&path, 10, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit1_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Addition);

    Ok(())
}

#[test]
fn intersection_with_deletion_change_type() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack1_id,
        commits_from_base_to_tip: vec![InputCommit {
            commit_id: commit1_id,
            files: vec![InputFile {
                path: path.clone(),
                change_type: TreeStatusKind::Deletion,
                hunks: vec![InputDiffHunk {
                    old_start: 1,
                    old_lines: 5,
                    new_start: 0,
                    new_lines: 0,
                }],
            }],
        }],
    }])?;

    // For deletions, any intersection query should return the hunk
    let dependencies = workspace_ranges.intersection_at(&path, 1, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit1_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Deletion);

    // Query outside the hunk range should still return it for deletions
    let dependencies = workspace_ranges.intersection_at(&path, 100, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit1_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Deletion);

    Ok(())
}

#[test]
fn intersection_with_modification_respects_range() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack1_id,
        commits_from_base_to_tip: vec![InputCommit {
            commit_id: commit1_id,
            files: vec![InputFile {
                path: path.clone(),
                change_type: TreeStatusKind::Modification,
                hunks: vec![InputDiffHunk {
                    old_start: 5,
                    old_lines: 3,
                    new_start: 5,
                    new_lines: 3,
                }],
            }],
        }],
    }])?;

    // Query inside the modification range should return it
    let dependencies = workspace_ranges.intersection_at(&path, 5, 2).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit1_id);

    // Query outside the modification range should return None
    let dependencies = workspace_ranges.intersection_at(&path, 1, 1);
    assert!(dependencies.is_none());

    let dependencies = workspace_ranges.intersection_at(&path, 10, 1);
    assert!(dependencies.is_none());

    Ok(())
}

#[test]
fn intersection_mixed_change_types() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = HunkLockTarget::Stack(StackId::generate());

    let commit2_id = id_from_hex_char('2');
    let stack2_id = HunkLockTarget::Stack(StackId::generate());

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![
        InputStack {
            target: stack1_id,
            commits_from_base_to_tip: vec![InputCommit {
                commit_id: commit1_id,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![InputDiffHunk {
                        old_start: 5,
                        old_lines: 2,
                        new_start: 5,
                        new_lines: 2,
                    }],
                }],
            }],
        },
        InputStack {
            target: stack2_id,
            commits_from_base_to_tip: vec![InputCommit {
                commit_id: commit2_id,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Addition,
                    hunks: vec![InputDiffHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 1,
                        new_lines: 10,
                    }],
                }],
            }],
        },
    ])?;

    // Query at any line should return the addition (since additions always intersect)
    let dependencies = workspace_ranges.intersection_at(&path, 1, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit2_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Addition);

    // Query at a different line should still return the addition
    let dependencies = workspace_ranges.intersection_at(&path, 100, 1).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].commit_id, commit2_id);
    assert_eq!(dependencies[0].change_type, TreeStatusKind::Addition);

    Ok(())
}

/// A worktree hunk that is adjacent to (but not overlapping) a commit's changed lines should
/// be locked when the diff content signals a reorder/block-move (shared lines, different last
/// line) — but NOT when it is a plain substitution (no shared lines).
///
/// Real-world scenario: commit A creates a file; commit B modifies lines 4-6.
/// A worktree change reorders lines 1-3.  B's hunk range starts at line 4 — immediately
/// after the worktree hunk ends at line 3.
///
/// `WorkspaceRanges::intersection()` uses `intersects_or_adjacent()` for Modification-type
/// hunks, but only when `hunk_suggests_boundary_insertion()` returns true for the diff.
#[test]
fn adjacent_reorder_is_locked_but_substitution_is_not() -> anyhow::Result<()> {
    let path = BString::from("file.txt");
    let stack_id = HunkLockTarget::Stack(StackId::generate());
    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack_id,
        commits_from_base_to_tip: vec![
            // Commit A: creates the file (6 lines).
            InputCommit {
                commit_id: commit_a,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Addition,
                    hunks: vec![InputDiffHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 1,
                        new_lines: 6,
                    }],
                }],
            },
            // Commit B: modifies lines 4-6. Range ends up starting at line 4 after
            // PathRanges processes the Addition above.
            InputCommit {
                commit_id: commit_b,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![InputDiffHunk {
                        old_start: 4,
                        old_lines: 3,
                        new_start: 4,
                        new_lines: 3,
                    }],
                }],
            },
        ],
    }])?;

    // Reorder hunk at lines 1-3: shared lines, different last line → signals boundary insertion.
    // Adjacent to commit B at line 4, so B should be locked.
    let reorder_hunk = DiffHunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        diff: "@@ -1,3 +1,3 @@\n-line_a\n-line_b\n-line_c\n+line_c\n+line_b\n+line_a\n".into(),
    };
    let locks = workspace_ranges
        .intersection(&path, &reorder_hunk)
        .unwrap_or_default();
    let lock_commits: Vec<_> = locks.iter().map(|h| h.commit_id).collect();
    assert!(
        lock_commits.contains(&commit_b),
        "reorder hunk at lines 1-3 should lock to commit B (adjacent at line 4), got: {lock_commits:?}"
    );

    // Substitution hunk at lines 1-3: no shared lines → no boundary-insertion signal.
    // Adjacent to commit B but should NOT be locked.
    let substitution_hunk = DiffHunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        diff: "@@ -1,3 +1,3 @@\n-x\n-y\n-z\n+a\n+b\n+c\n".into(),
    };
    let locks = workspace_ranges
        .intersection(&path, &substitution_hunk)
        .unwrap_or_default();
    let lock_commits: Vec<_> = locks.iter().map(|h| h.commit_id).collect();
    assert!(
        !lock_commits.contains(&commit_b),
        "substitution hunk at lines 1-3 should NOT lock to commit B (no shared lines), got: {lock_commits:?}"
    );

    Ok(())
}

/// Mimics the real StackCodegen.svelte scenario:
/// - Commit A creates a file with 20 lines
/// - Commit B deletes line 14 (context_lines=0 → old_start=14, old_lines=1, new_lines=0)
/// - Worktree reorders lines 11-13
///
/// After PathRanges processes B's deletion, B owns a point at position 14 with lines=0.
/// The worktree hunk at 11-13 ends at line 13, which is adjacent to position 14.
#[test]
fn adjacent_deletion_point_locked_for_reorder() -> anyhow::Result<()> {
    let path = BString::from("file.txt");
    let stack_id = HunkLockTarget::Stack(StackId::generate());
    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack_id,
        commits_from_base_to_tip: vec![
            InputCommit {
                commit_id: commit_a,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Addition,
                    hunks: vec![InputDiffHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 1,
                        new_lines: 20,
                    }],
                }],
            },
            // Commit B: deletes a single line at position 14 (like removing an import).
            InputCommit {
                commit_id: commit_b,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![InputDiffHunk {
                        old_start: 14,
                        old_lines: 1,
                        new_start: 14,
                        new_lines: 0,
                    }],
                }],
            },
        ],
    }])?;

    // Reorder hunk at lines 11-13 (adjacent to B's deletion point at 14).
    let reorder_hunk = DiffHunk {
        old_start: 11,
        old_lines: 3,
        new_start: 11,
        new_lines: 3,
        diff: "@@ -11,3 +11,3 @@\n-line_a\n-line_b\n-line_c\n+line_c\n+line_b\n+line_a\n".into(),
    };
    let locks = workspace_ranges
        .intersection(&path, &reorder_hunk)
        .unwrap_or_default();
    let lock_commits: Vec<_> = locks.iter().map(|h| h.commit_id).collect();
    assert!(
        lock_commits.contains(&commit_b),
        "reorder hunk at lines 11-13 should lock to commit B (deletion point at 14), got: {lock_commits:?}"
    );

    Ok(())
}

/// Faithfully reproduces the StackCodegen.svelte scenario from the workspace.
///
/// Three commits in the stack (refactor/filelist-controller):
/// - Commit A (741cd057d): creates StackCodegen.svelte with 164 lines
/// - Commit B (846d4c4ab): heavy refactor — deletes imports, state, actions (164→71 lines)
///   With context_lines=0, this produces many small hunks (deletion at line 14, etc.)
/// - Commit C (f7e70bef9): adds UI_STATE import (insertion after line 15)
///
/// The uncommitted worktree change reorders three imports at lines 11-13.
/// With context_lines=0, this is TWO hunks (not one):
///   1. @@ -11,2 +10,0 @$ — delete CodegenMessages + CodegenMcpConfigModal
///   2. @@ -13,0 +12,2 $$ — insert them back after ReduxResult
///
/// When `context_lines=0`, a line reorder (delete + re-insert in different order)
/// is split into two separate hunks by the diff algorithm: a deletion hunk and an
/// insertion hunk. The insertion lands adjacent to a tracked deletion point from
/// an earlier commit.
///
/// The dependency on commit B should be detected because B's deletion at old
/// line 14 creates a tracked point at position 14 in the final file, and the
/// worktree insertion after position 13 is adjacent to it.
#[test]
fn split_reorder_adjacent_to_scattered_deletions() -> anyhow::Result<()> {
    let path = BString::from("file.txt");
    let stack_id = HunkLockTarget::Stack(StackId::generate());
    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');
    let commit_c = id_from_hex_char('c');

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack_id,
        commits_from_base_to_tip: vec![
            // Commit A: creates the file (164 lines).
            InputCommit {
                commit_id: commit_a,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Addition,
                    hunks: vec![InputDiffHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 1,
                        new_lines: 164,
                    }],
                }],
            },
            // Commit B: heavy refactor with many scattered deletions (context_lines=0).
            // Crucially, the deletion at old line 14 creates a tracked point that
            // the worktree reorder will land adjacent to.
            InputCommit {
                commit_id: commit_b,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![
                        InputDiffHunk {
                            old_start: 2,
                            old_lines: 1,
                            new_start: 2,
                            new_lines: 1,
                        },
                        InputDiffHunk {
                            old_start: 14,
                            old_lines: 1,
                            new_start: 13,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 16,
                            old_lines: 2,
                            new_start: 14,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 20,
                            old_lines: 1,
                            new_start: 16,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 26,
                            old_lines: 1,
                            new_start: 21,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 30,
                            old_lines: 1,
                            new_start: 25,
                            new_lines: 1,
                        },
                        InputDiffHunk {
                            old_start: 33,
                            old_lines: 2,
                            new_start: 27,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 36,
                            old_lines: 65,
                            new_start: 28,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 110,
                            old_lines: 4,
                            new_start: 37,
                            new_lines: 0,
                        },
                        InputDiffHunk {
                            old_start: 115,
                            old_lines: 11,
                            new_start: 39,
                            new_lines: 1,
                        },
                        InputDiffHunk {
                            old_start: 128,
                            old_lines: 7,
                            new_start: 41,
                            new_lines: 0,
                        },
                    ],
                }],
            },
            // Commit C: a few small additions and edits.
            InputCommit {
                commit_id: commit_c,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![
                        InputDiffHunk {
                            old_start: 15,
                            old_lines: 0,
                            new_start: 16,
                            new_lines: 1,
                        },
                        InputDiffHunk {
                            old_start: 27,
                            old_lines: 0,
                            new_start: 29,
                            new_lines: 1,
                        },
                        InputDiffHunk {
                            old_start: 52,
                            old_lines: 3,
                            new_start: 54,
                            new_lines: 1,
                        },
                    ],
                }],
            },
        ],
    }])?;

    // Worktree hunk 1: deletion of two lines at position 11-12.
    // With context_lines=0, a line reorder is split into a deletion and insertion.
    let deletion_hunk = DiffHunk {
        old_start: 11,
        old_lines: 2,
        new_start: 10,
        new_lines: 0,
        diff: "@@ -11,2 +10,0 @@\n-line_11\n-line_12\n".into(),
    };

    // Worktree hunk 2: re-insertion of the same two lines (reordered) after line 13.
    // This is a pure insertion (old_lines=0) adjacent to commit B's deletion point.
    let insertion_hunk = DiffHunk {
        old_start: 13,
        old_lines: 0,
        new_start: 12,
        new_lines: 2,
        diff: "@@ -13,0 +12,2 @@\n+line_12\n+line_11\n".into(),
    };

    // At least one of the two worktree hunks should lock to commit B.
    let deletion_locks = workspace_ranges
        .intersection(&path, &deletion_hunk)
        .unwrap_or_default();
    let insertion_locks = workspace_ranges
        .intersection(&path, &insertion_hunk)
        .unwrap_or_default();

    let all_lock_commits: Vec<_> = deletion_locks
        .iter()
        .chain(insertion_locks.iter())
        .map(|h| h.commit_id)
        .collect();

    assert!(
        all_lock_commits.contains(&commit_b),
        "reorder adjacent to commit B's deletion should lock to B, got: {all_lock_commits:?}"
    );

    Ok(())
}

/// A pure insertion of completely new content adjacent to a commit's modification range.
/// In a real 3-way merge, git would cleanly insert these new lines — there's no content
/// overlap or reorder, just fresh lines added right after the commit's changed region.
///
/// Adjacency is only extended for pure insertions when the commit range is also a
/// deletion point (lines == 0), because that's when the merge algorithm loses its
/// anchor. Normal modification spans have clear boundaries, so insertions outside
/// them apply cleanly.
#[test]
fn pure_insertion_of_new_content_adjacent_to_modification_is_not_locked() -> anyhow::Result<()> {
    let path = BString::from("file.txt");
    let stack_id = HunkLockTarget::Stack(StackId::generate());
    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        target: stack_id,
        commits_from_base_to_tip: vec![
            // Commit A: creates a 20-line file.
            InputCommit {
                commit_id: commit_a,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Addition,
                    hunks: vec![InputDiffHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 1,
                        new_lines: 20,
                    }],
                }],
            },
            // Commit B: modifies lines 10-12 (e.g. a substitution).
            InputCommit {
                commit_id: commit_b,
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Modification,
                    hunks: vec![InputDiffHunk {
                        old_start: 10,
                        old_lines: 3,
                        new_start: 10,
                        new_lines: 3,
                    }],
                }],
            },
        ],
    }])?;

    // Worktree: insert two completely new lines right after commit B's range (after line 13).
    // old_start=13 means "insert after line 13" — one past commit B's last line (12).
    // This is a pure insertion (old_lines=0) with content that has nothing to do with
    // commit B's changes. A 3-way merge would apply this cleanly.
    let insertion_hunk = DiffHunk {
        old_start: 13,
        old_lines: 0,
        new_start: 14,
        new_lines: 2,
        diff: "@@ -13,0 +14,2 @@\n+completely_new_line_1\n+completely_new_line_2\n".into(),
    };

    let locks = workspace_ranges
        .intersection(&path, &insertion_hunk)
        .unwrap_or_default();
    let lock_commits: Vec<_> = locks.iter().map(|h| h.commit_id).collect();

    // Commit A (file creation) will match because the insertion is inside its range.
    // But commit B should NOT match — it's a normal modification span (lines > 0),
    // and the insertion is adjacent, not overlapping. A 3-way merge handles this cleanly.
    assert!(
        !lock_commits.contains(&commit_b),
        "Pure insertion of new content adjacent to a modification span should NOT lock to B, got: {lock_commits:?}"
    );

    Ok(())
}
