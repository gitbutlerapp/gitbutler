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

    let dependencies_2 = workspace_ranges.intersection(&path, 9, 1).unwrap();
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
