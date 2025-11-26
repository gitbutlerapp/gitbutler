use but_core::{TreeStatusKind, ref_metadata::StackId};
use gix::bstr::BString;

use crate::{
    InputCommit, InputDiffHunk, InputStack,
    input::InputFile,
    ranges::{
        WorkspaceRanges,
        tests::{id_from_hex_char, input_hunk_from_unified_diff},
    },
};

#[test]
fn workspace_simple() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let stack1_id = StackId::generate();

    let commit2_id = id_from_hex_char('1');
    let stack2_id = StackId::generate();

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![
        InputStack {
            stack_id: stack1_id,
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
            stack_id: stack2_id,
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

    let dependencies_1 = workspace_ranges.intersection(&path, 2, 1).unwrap();
    assert_eq!(dependencies_1.len(), 1);
    assert_eq!(dependencies_1[0].commit_id, commit1_id);
    assert_eq!(dependencies_1[0].stack_id, stack1_id);

    let dependencies_2 = workspace_ranges.intersection(&path, 10, 1).unwrap();
    assert_eq!(dependencies_2.len(), 1);
    assert_eq!(dependencies_2[0].commit_id, commit2_id);
    assert_eq!(dependencies_2[0].stack_id, stack2_id);

    let dependencies_3 = workspace_ranges.intersection(&path, 15, 1).unwrap();
    assert_eq!(dependencies_3.len(), 1);
    assert_eq!(dependencies_3[0].commit_id, commit2_id);
    assert_eq!(dependencies_3[0].stack_id, stack2_id);

    Ok(())
}

#[test]
fn overlapping_commits_in_a_stack() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let commit1_id = id_from_hex_char('1');
    let commit2_id = id_from_hex_char('2');
    let stack1_id = StackId::generate();

    let commit3_id = id_from_hex_char('3');
    let stack2_id = StackId::generate();

    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![
        InputStack {
            stack_id: stack1_id,
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
            stack_id: stack2_id,
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
        let dependencies = workspace_ranges.intersection(&path, 12, 1).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].commit_id, commit3_id);
        assert_eq!(dependencies[0].stack_id, stack2_id);
    }

    Ok(())
}

#[test]
fn gracefully_handle_invalid_input_commits() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let stack_id = StackId::generate();
    let commit_a_id = id_from_hex_char('a');
    let commit_b_id = id_from_hex_char('b');
    let commit_c_id = id_from_hex_char('c');

    // Invalid input, two subsequent commits with the same changes.
    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        stack_id,
        commits_from_base_to_tip: vec![
            InputCommit {
                commit_id: commit_a_id, // Delete file
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Deletion,
                    hunks: vec![InputDiffHunk {
                        old_start: 1,
                        old_lines: 2,
                        new_start: 0,
                        new_lines: 0,
                    }],
                }],
            },
            InputCommit {
                commit_id: commit_b_id, // Delete file, again
                files: vec![InputFile {
                    path: path.clone(),
                    change_type: TreeStatusKind::Deletion,
                    hunks: vec![InputDiffHunk {
                        old_start: 1,
                        old_lines: 2,
                        new_start: 0,
                        new_lines: 0,
                    }],
                }],
            },
            InputCommit {
                commit_id: commit_c_id, // Re-add file
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
            },
        ],
    }])?;

    let dependencies_1 = workspace_ranges.intersection(&path, 2, 1).unwrap();
    assert_eq!(dependencies_1.len(), 1);
    assert_eq!(dependencies_1[0].commit_id, commit_c_id);
    assert_eq!(dependencies_1[0].stack_id, stack_id);

    let errors = &workspace_ranges.errors;
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].commit_id, commit_b_id);
    assert_eq!(errors[0].stack_id, stack_id);
    assert_eq!(errors[0].path, path);
    assert_eq!(
        errors[0].error_message,
        "File recreation must be an addition"
    );

    Ok(())
}
