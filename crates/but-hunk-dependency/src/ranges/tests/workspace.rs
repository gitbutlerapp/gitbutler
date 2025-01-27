use crate::input::InputFile;
use crate::ranges::tests::{id_from_hex_char, input_hunk_from_unified_diff};
use crate::ranges::{get_inverted_dependency_maps, WorkspaceRanges};
use crate::{InputCommit, InputDiffHunk, InputStack};
use but_core::TreeStatusKind;
use but_workspace::StackId;
use gix::bstr::BString;
use std::collections::{HashMap, HashSet};

#[test]
fn get_inverted_dependency_maps_test_single_stack() {
    let stack_id = StackId::generate();
    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');
    let commit_c = id_from_hex_char('c');
    let commit_d = id_from_hex_char('d');

    let original_map = {
        let mut map = HashMap::new();
        map.insert(stack_id, {
            let mut inner_map = HashMap::new();
            inner_map.insert(commit_a, {
                let mut set = HashSet::new();
                set.insert(commit_b);
                set.insert(commit_c);
                set.insert(commit_d);
                set
            });
            inner_map
        });
        map
    };

    let inverted_map = get_inverted_dependency_maps(&original_map);
    assert_eq!(inverted_map.len(), 1);
    let stack_inverted_map = inverted_map.get(&stack_id).unwrap();
    assert_eq!(stack_inverted_map.len(), 3);
    // b
    assert!(stack_inverted_map.contains_key(&commit_b));
    let commit_b_deps = stack_inverted_map.get(&commit_b).unwrap();
    assert_eq!(commit_b_deps.len(), 1);
    assert!(commit_b_deps.contains(&commit_a));
    // c
    assert!(stack_inverted_map.contains_key(&commit_c));
    let commit_c_deps = stack_inverted_map.get(&commit_c).unwrap();
    assert_eq!(commit_c_deps.len(), 1);
    assert!(commit_c_deps.contains(&commit_a));
    // d
    assert!(stack_inverted_map.contains_key(&commit_d));
    let commit_d_deps = stack_inverted_map.get(&commit_d).unwrap();
    assert_eq!(commit_d_deps.len(), 1);
    assert!(commit_d_deps.contains(&commit_a));
}

#[test]
fn get_inverted_dependency_maps_test_multiple_stacks() {
    let stack_id_a = StackId::generate();
    let stack_id_b = StackId::generate();

    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');
    let commit_c = id_from_hex_char('c');
    let commit_d = id_from_hex_char('d');
    let commit_e = id_from_hex_char('e');
    let commit_f = id_from_hex_char('f');
    let commit_g = id_from_hex_char('0');
    let commit_h = id_from_hex_char('1');

    let original_map = {
        let mut map = HashMap::new();
        map.insert(stack_id_a, {
            let mut inner_map = HashMap::new();
            inner_map.insert(commit_a, {
                let mut set = HashSet::new();
                set.insert(commit_b);
                set.insert(commit_c);
                set.insert(commit_d);
                set
            });
            inner_map
        });
        map.insert(stack_id_b, {
            let mut inner_map = HashMap::new();
            inner_map.insert(commit_e, {
                let mut set = HashSet::new();
                set.insert(commit_f);
                set.insert(commit_g);
                set.insert(commit_h);
                set
            });
            inner_map
        });
        map
    };

    let inverted_map = get_inverted_dependency_maps(&original_map);
    assert_eq!(inverted_map.len(), 2);
    // stack a
    assert!(inverted_map.contains_key(&stack_id_a));
    let stack_a_inverted_map = inverted_map.get(&stack_id_a).unwrap();
    assert_eq!(stack_a_inverted_map.len(), 3);
    // === b
    assert!(stack_a_inverted_map.contains_key(&commit_b));
    let commit_b_deps = stack_a_inverted_map.get(&commit_b).unwrap();
    assert_eq!(commit_b_deps.len(), 1);
    assert!(commit_b_deps.contains(&commit_a));
    // === c
    assert!(stack_a_inverted_map.contains_key(&commit_c));
    let commit_c_deps = stack_a_inverted_map.get(&commit_c).unwrap();
    assert_eq!(commit_c_deps.len(), 1);
    assert!(commit_c_deps.contains(&commit_a));
    // === d
    assert!(stack_a_inverted_map.contains_key(&commit_d));
    let commit_d_deps = stack_a_inverted_map.get(&commit_d).unwrap();
    assert_eq!(commit_d_deps.len(), 1);
    assert!(commit_d_deps.contains(&commit_a));

    // stack b
    assert!(inverted_map.contains_key(&stack_id_b));
    let stack_b_inverted_map = inverted_map.get(&stack_id_b).unwrap();
    assert_eq!(stack_b_inverted_map.len(), 3);
    // === f
    assert!(stack_b_inverted_map.contains_key(&commit_f));
    let commit_f_deps = stack_b_inverted_map.get(&commit_f).unwrap();
    assert_eq!(commit_f_deps.len(), 1);
    assert!(commit_f_deps.contains(&commit_e));
    // === g
    assert!(stack_b_inverted_map.contains_key(&commit_g));
    let commit_g_deps = stack_b_inverted_map.get(&commit_g).unwrap();
    assert_eq!(commit_g_deps.len(), 1);
    assert!(commit_g_deps.contains(&commit_e));
    // === h
    assert!(stack_b_inverted_map.contains_key(&commit_h));
    let commit_h_deps = stack_b_inverted_map.get(&commit_h).unwrap();
    assert_eq!(commit_h_deps.len(), 1);
    assert!(commit_h_deps.contains(&commit_e));
}

#[test]
fn get_inverted_dependency_maps_test_multple_dependencies() {
    let stack_id_a = StackId::generate();
    let stack_id_b = StackId::generate();

    let commit_a = id_from_hex_char('a');
    let commit_b = id_from_hex_char('b');
    let commit_c = id_from_hex_char('c');
    let commit_d = id_from_hex_char('d');
    let commit_e = id_from_hex_char('e');
    let commit_f = id_from_hex_char('f');
    let commit_g = id_from_hex_char('0');
    let commit_h = id_from_hex_char('1');

    let original_map = {
        let mut map = HashMap::new();
        map.insert(stack_id_a, {
            let mut inner_map = HashMap::new();
            inner_map.insert(commit_a, {
                let mut set = HashSet::new();
                set.insert(commit_b);
                set.insert(commit_c);
                set.insert(commit_d);
                set
            });

            inner_map.insert(commit_b, {
                let mut set = HashSet::new();
                set.insert(commit_c);
                set.insert(commit_d);
                set
            });

            inner_map.insert(commit_c, {
                let mut set = HashSet::new();
                set.insert(commit_d);
                set
            });

            inner_map
        });
        map.insert(stack_id_b, {
            let mut inner_map = HashMap::new();
            inner_map.insert(commit_e, {
                let mut set = HashSet::new();
                set.insert(commit_f);
                set.insert(commit_g);
                set.insert(commit_h);
                set
            });

            inner_map.insert(commit_f, {
                let mut set = HashSet::new();
                set.insert(commit_g);
                set.insert(commit_h);
                set
            });

            inner_map.insert(commit_g, {
                let mut set = HashSet::new();
                set.insert(commit_h);
                set
            });

            inner_map
        });
        map
    };

    let inverted_map = get_inverted_dependency_maps(&original_map);
    assert_eq!(inverted_map.len(), 2);
    // stack a
    assert!(inverted_map.contains_key(&stack_id_a));
    let stack_a_inverted_map = inverted_map.get(&stack_id_a).unwrap();
    assert_eq!(stack_a_inverted_map.len(), 3);
    // === b
    assert!(stack_a_inverted_map.contains_key(&commit_b));
    let commit_b_deps = stack_a_inverted_map.get(&commit_b).unwrap();
    assert_eq!(commit_b_deps.len(), 1);
    assert!(commit_b_deps.contains(&commit_a));
    // === c
    assert!(stack_a_inverted_map.contains_key(&commit_c));
    let commit_c_deps = stack_a_inverted_map.get(&commit_c).unwrap();
    assert_eq!(commit_c_deps.len(), 2);
    assert!(commit_c_deps.contains(&commit_a));
    assert!(commit_c_deps.contains(&commit_b));
    // === d
    assert!(stack_a_inverted_map.contains_key(&commit_d));
    let commit_d_deps = stack_a_inverted_map.get(&commit_d).unwrap();
    assert_eq!(commit_d_deps.len(), 3);
    assert!(commit_d_deps.contains(&commit_a));
    assert!(commit_d_deps.contains(&commit_b));
    assert!(commit_d_deps.contains(&commit_c));

    // stack b
    assert!(inverted_map.contains_key(&stack_id_b));
    let stack_b_inverted_map = inverted_map.get(&stack_id_b).unwrap();
    assert_eq!(stack_b_inverted_map.len(), 3);
    // === f
    assert!(stack_b_inverted_map.contains_key(&commit_f));
    let commit_f_deps = stack_b_inverted_map.get(&commit_f).unwrap();
    assert_eq!(commit_f_deps.len(), 1);
    assert!(commit_f_deps.contains(&commit_e));

    // === g
    assert!(stack_b_inverted_map.contains_key(&commit_g));
    let commit_g_deps = stack_b_inverted_map.get(&commit_g).unwrap();
    assert_eq!(commit_g_deps.len(), 2);
    assert!(commit_g_deps.contains(&commit_e));
    assert!(commit_g_deps.contains(&commit_f));

    // === h
    assert!(stack_b_inverted_map.contains_key(&commit_h));
    let commit_h_deps = stack_b_inverted_map.get(&commit_h).unwrap();
    assert_eq!(commit_h_deps.len(), 3);
    assert!(commit_h_deps.contains(&commit_e));
    assert!(commit_h_deps.contains(&commit_f));
    assert!(commit_h_deps.contains(&commit_g));
}

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
            commits: vec![InputCommit {
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
            commits: vec![InputCommit {
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
fn gracefully_handle_invalid_input_commits() -> anyhow::Result<()> {
    let path = BString::from("/test.txt");

    let stack_id = StackId::generate();
    let commit_a_id = id_from_hex_char('a');
    let commit_b_id = id_from_hex_char('b');
    let commit_c_id = id_from_hex_char('c');

    // Invalid input, two subsequent commits with the same changes.
    let workspace_ranges = WorkspaceRanges::try_from_stacks(vec![InputStack {
        stack_id,
        commits: vec![
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
