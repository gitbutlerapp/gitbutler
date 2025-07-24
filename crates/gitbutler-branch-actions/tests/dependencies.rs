use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::Result;
use gitbutler_branch_actions::compute_workspace_dependencies;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::{ChangeType, GitHunk, Hunk};
use gitbutler_hunk_dependency::HunkLock;
use gitbutler_stack::VirtualBranchesHandle;

#[test]
fn every_commit_is_independent() -> Result<()> {
    let ctx = command_ctx("independent-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    // No uncommited changes
    assert_eq!(dependencies.diffs.len(), 0);
    // One stack
    assert_eq!(dependencies.commit_dependencies.len(), 1);
    // No interdependencies
    let stack_commit_dependencies = dependencies.commit_dependencies.get(&stack.id).unwrap();
    assert_eq!(stack_commit_dependencies.len(), 0);
    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 0);
    assert_eq!(dependencies.commit_dependent_diffs.len(), 0);

    Ok(())
}

#[test]
fn every_commit_is_independent_multi_stack() -> Result<()> {
    let ctx = command_ctx("independent-commits-multi-stack")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    // No uncommited changes
    assert_eq!(dependencies.diffs.len(), 0);
    // One stack
    assert_eq!(dependencies.commit_dependencies.len(), 2);
    // No interdependencies
    let stack_commit_dependencies = dependencies.commit_dependencies.get(&stack.id).unwrap();
    assert_eq!(stack_commit_dependencies.len(), 0);
    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 0);
    assert_eq!(dependencies.commit_dependent_diffs.len(), 0);

    Ok(())
}

#[test]
fn every_commit_is_sequentially_dependent() -> Result<()> {
    let ctx = command_ctx("sequentially-dependent-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    // No uncommited changes
    assert_eq!(dependencies.diffs.len(), 0);
    // One stack
    assert_eq!(dependencies.commit_dependencies.len(), 1);
    // Interdependencies
    let stack_commit_dependencies = dependencies.commit_dependencies.get(&stack.id).unwrap();
    assert_eq!(stack_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_commit_dependencies,
        HashMap::from([
            ("overwrite file with b", vec!["add file"]),
            ("overwrite file with c", vec!["overwrite file with b"]),
            ("overwrite file with d", vec!["overwrite file with c"]),
            ("overwrite file with e", vec!["overwrite file with d"]),
            ("overwrite file with f", vec!["overwrite file with e"]),
        ]),
        &ctx,
        "commit_dependencies",
    );

    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_inverse_commit_dependencies,
        HashMap::from([
            ("add file", vec!["overwrite file with b"]),
            ("overwrite file with b", vec!["overwrite file with c"]),
            ("overwrite file with c", vec!["overwrite file with d"]),
            ("overwrite file with d", vec!["overwrite file with e"]),
            ("overwrite file with e", vec!["overwrite file with f"]),
        ]),
        &ctx,
        "inverse_commit_dependencies",
    );

    assert_eq!(dependencies.commit_dependent_diffs.len(), 0);

    Ok(())
}

#[test]
fn every_commit_is_sequentially_dependent_multi_stack() -> Result<()> {
    let ctx = command_ctx("sequentially-dependent-commits-muli-stack")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;
    let other_stack = test_ctx
        .all_stacks
        .iter()
        .find(|s| s.name == "other_stack")
        .unwrap();

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    // No uncommited changes
    assert_eq!(dependencies.diffs.len(), 0);
    // One stack
    assert_eq!(dependencies.commit_dependencies.len(), 2);
    // Interdependencies - other_stack
    let stack_commit_dependencies = dependencies
        .commit_dependencies
        .get(&other_stack.id)
        .unwrap();
    assert_eq!(stack_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_commit_dependencies,
        HashMap::from([
            ("overwrite file with b", vec!["add file"]),
            ("overwrite file with c", vec!["overwrite file with b"]),
            ("overwrite file with d", vec!["overwrite file with c"]),
            ("overwrite file with e", vec!["overwrite file with d"]),
            ("overwrite file with f", vec!["overwrite file with e"]),
        ]),
        &ctx,
        "other_stack - commit_dependencies",
    );

    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&other_stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_inverse_commit_dependencies,
        HashMap::from([
            ("add file", vec!["overwrite file with b"]),
            ("overwrite file with b", vec!["overwrite file with c"]),
            ("overwrite file with c", vec!["overwrite file with d"]),
            ("overwrite file with d", vec!["overwrite file with e"]),
            ("overwrite file with e", vec!["overwrite file with f"]),
        ]),
        &ctx,
        "other_stack - inverse_commit_dependencies",
    );

    // Interdependencies - my_stack
    let stack_commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_eq!(stack_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_commit_dependencies,
        HashMap::from([
            ("overwrite file_2 with b", vec!["add file_2"]),
            ("overwrite file_2 with c", vec!["overwrite file_2 with b"]),
            ("overwrite file_2 with d", vec!["overwrite file_2 with c"]),
            ("overwrite file_2 with e", vec!["overwrite file_2 with d"]),
            ("overwrite file_2 with f", vec!["overwrite file_2 with e"]),
        ]),
        &ctx,
        "my_stack - commit_dependencies",
    );

    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&my_stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_inverse_commit_dependencies,
        HashMap::from([
            ("add file_2", vec!["overwrite file_2 with b"]),
            ("overwrite file_2 with b", vec!["overwrite file_2 with c"]),
            ("overwrite file_2 with c", vec!["overwrite file_2 with d"]),
            ("overwrite file_2 with d", vec!["overwrite file_2 with e"]),
            ("overwrite file_2 with e", vec!["overwrite file_2 with f"]),
        ]),
        &ctx,
        "my_stack - inverse_commit_dependencies",
    );

    assert_eq!(dependencies.commit_dependent_diffs.len(), 0);

    Ok(())
}

#[test]
fn delete_and_recreate_file_multi_stack() -> Result<()> {
    let ctx = command_ctx("delete-and-recreate-file-multi-stack")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;
    let other_stack = test_ctx
        .all_stacks
        .iter()
        .find(|s| s.name == "other_stack")
        .unwrap();

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    // No uncommited changes
    assert_eq!(dependencies.diffs.len(), 0);
    // One stack
    assert_eq!(dependencies.commit_dependencies.len(), 2);
    // Interdependencies - other_stack
    let stack_commit_dependencies = dependencies
        .commit_dependencies
        .get(&other_stack.id)
        .unwrap();

    assert_commit_map_matches_by_message(
        stack_commit_dependencies,
        HashMap::from([
            ("overwrite file with b", vec!["add file"]),
            ("remove file", vec!["overwrite file with b"]),
            ("recreate file with d", vec!["remove file"]),
            ("remove file again", vec!["recreate file with d"]),
            ("recreate file with f", vec!["remove file again"]),
        ]),
        &ctx,
        "other_stack - commit_dependencies",
    );

    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&other_stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_inverse_commit_dependencies,
        HashMap::from([
            ("add file", vec!["overwrite file with b"]),
            ("overwrite file with b", vec!["remove file"]),
            ("remove file", vec!["recreate file with d"]),
            ("recreate file with d", vec!["remove file again"]),
            ("remove file again", vec!["recreate file with f"]),
        ]),
        &ctx,
        "other_stack - inverse_commit_dependencies",
    );

    // Interdependencies - my_stack
    let stack_commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_eq!(stack_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_commit_dependencies,
        HashMap::from([
            ("remove file_2", vec!["add file_2"]),
            ("recreate file_2 with c", vec!["remove file_2"]),
            ("remove file_2 again", vec!["recreate file_2 with c"]),
            ("recreate file_2 with e", vec!["remove file_2 again"]),
            (
                "remove file_2 one last time",
                vec!["recreate file_2 with e"],
            ),
        ]),
        &ctx,
        "my_stack - commit_dependencies",
    );

    let stack_inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&my_stack.id)
        .unwrap();
    assert_eq!(stack_inverse_commit_dependencies.len(), 5);
    assert_commit_map_matches_by_message(
        stack_inverse_commit_dependencies,
        HashMap::from([
            ("add file_2", vec!["remove file_2"]),
            ("remove file_2", vec!["recreate file_2 with c"]),
            ("recreate file_2 with c", vec!["remove file_2 again"]),
            ("remove file_2 again", vec!["recreate file_2 with e"]),
            (
                "recreate file_2 with e",
                vec!["remove file_2 one last time"],
            ),
        ]),
        &ctx,
        "my_stack - inverse_commit_dependencies",
    );

    assert_eq!(dependencies.commit_dependent_diffs.len(), 0);

    Ok(())
}

#[test]
fn complex_file_manipulation() -> Result<()> {
    let ctx = command_ctx("complex-file-manipulation")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    let commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_commit_map_matches_by_message(
        commit_dependencies,
        HashMap::from([
            ("modify line 5", vec!["add file"]),
            (
                "file: add lines d and e at the beginning | file_2: modify line 1",
                vec!["add file", "recreate file"],
            ),
            (
                "remove file",
                vec!["file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"],
            ),
            ("recreate file", vec!["remove file"]),
            (
                "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                vec!["modify line 5", "add file"],
            ),
            ("add lines a, b and c at the end", vec!["recreate file"]),
        ]),
        &ctx,
        "commit_dependencies",
    );
    let inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&my_stack.id)
        .unwrap();

    assert_commit_map_matches_by_message(
        inverse_commit_dependencies,
        HashMap::from([
            (
                "recreate file",
                vec![
                    "file: add lines d and e at the beginning | file_2: modify line 1",
                    "add lines a, b and c at the end",
                ],
            ),
            (
                "modify line 5",
                vec!["file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"],
            ),
            (
                "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                vec!["remove file"],
            ),
            (
                "add file",
                vec![
                    "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                    "file: add lines d and e at the beginning | file_2: modify line 1",
                    "modify line 5",
                ],
            ),
            ("remove file", vec!["recreate file"]),
        ]),
        &ctx,
        "inverse_commit_dependencies",
    );

    Ok(())
}

#[test]
fn complex_file_manipulation_with_uncommitted_changes() -> Result<()> {
    let ctx = command_ctx("complex-file-manipulation")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;

    let file_path = Path::new("file");
    let file_hunk = GitHunk {
        old_start: 2,
        old_lines: 4,
        new_start: 3,
        new_lines: 3,
        diff_lines: "@@ -2,4 +3,3 @@
-e
-1
-2
-3
+updated line 3
+updated line 4
+updated line 5
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let file_2_path = Path::new("file_2");
    let file_2_hunk = GitHunk {
        old_start: 4,
        old_lines: 1,
        new_start: 4,
        new_lines: 1,
        diff_lines: "@@ -4,1 +4,1 @@
-d
+updated d
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let base_diffs: HashMap<PathBuf, Vec<GitHunk>> = HashMap::from([
        (file_path.to_path_buf(), vec![file_hunk.clone()]),
        (file_2_path.to_path_buf(), vec![file_2_hunk.clone()]),
    ]);

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &base_diffs,
        &test_ctx.all_stacks,
    )?;

    assert_eq!(dependencies.diffs.len(), 2);
    let file_hunk_hash = Hunk::hash_diff(&file_hunk.diff_lines);
    let file_2_hunk_hash = Hunk::hash_diff(&file_2_hunk.diff_lines);

    let file_diffs = dependencies.diffs.get(&file_hunk_hash).unwrap();
    assert_eq!(file_diffs.len(), 2);
    assert_hunk_lock_matches_by_message(
        file_diffs[0],
        "file: add lines d and e at the beginning | file_2: modify line 1",
        &ctx,
        "file_diffs",
    );
    assert_hunk_lock_matches_by_message(file_diffs[1], "recreate file", &ctx, "file_diffs");

    let file_2_diffs = dependencies.diffs.get(&file_2_hunk_hash).unwrap();
    assert_eq!(file_2_diffs.len(), 1);
    assert_hunk_lock_matches_by_message(file_2_diffs[0], "add file", &ctx, "file_2_diffs");

    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks() -> Result<()> {
    let ctx = command_ctx("complex-file-manipulation-multiple-hunks")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    let commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_commit_map_matches_by_message(
        commit_dependencies,
        HashMap::from([
            ("modify lines 4 and 8", vec!["create file"]),
            (
                "insert 2 lines after 2, modify line 4 and remove line 6",
                vec!["modify lines 4 and 8", "create file"],
            ),
            (
                "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
                vec![
                    "insert 2 lines after 2, modify line 4 and remove line 6",
                    "create file",
                ],
            ),
        ]),
        &ctx,
        "commit_dependencies",
    );

    let inverse_commit_dependencies = dependencies
        .inverse_commit_dependencies
        .get(&my_stack.id)
        .unwrap();
    assert_commit_map_matches_by_message(
        inverse_commit_dependencies,
        HashMap::from([
            (
                "create file",
                vec![
                    "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
                    "insert 2 lines after 2, modify line 4 and remove line 6",
                    "modify lines 4 and 8",
                ],
            ),
            (
                "modify lines 4 and 8",
                vec!["insert 2 lines after 2, modify line 4 and remove line 6"],
            ),
            (
                "insert 2 lines after 2, modify line 4 and remove line 6",
                vec!["insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7"],
            ),
        ]),
        &ctx,
        "inverse_commit_dependencies",
    );

    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks_with_uncommitted_changes() -> Result<()> {
    let ctx = command_ctx("complex-file-manipulation-multiple-hunks")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;

    let file_path = Path::new("file");
    let file_hunk_1 = GitHunk {
        old_start: 3,
        old_lines: 1,
        new_start: 3,
        new_lines: 2,
        diff_lines: "@@ -3,1 +3,2 @@
-2
+aaaaa
+aaaaa
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let file_hunk_2 = GitHunk {
        old_start: 7,
        old_lines: 2,
        new_start: 8,
        new_lines: 1,
        diff_lines: "@@ -7,2 +8,1 @@
-update 7
-update 8
+aaaaa
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let file_hunk_3 = GitHunk {
        old_start: 10,
        old_lines: 1,
        new_start: 10,
        new_lines: 2,
        diff_lines: "@@ -10,1 +10,2 @@
-added at the bottom
+update bottom
+add another line
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let base_diffs: HashMap<PathBuf, Vec<GitHunk>> = HashMap::from([(
        file_path.to_path_buf(),
        vec![
            file_hunk_1.clone(),
            file_hunk_2.clone(),
            file_hunk_3.clone(),
        ],
    )]);

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &base_diffs,
        &test_ctx.all_stacks,
    )?;

    assert_eq!(dependencies.diffs.len(), 3);
    let file_hunk_1_hash = Hunk::hash_diff(&file_hunk_1.diff_lines);
    let file_hunk_2_hash = Hunk::hash_diff(&file_hunk_2.diff_lines);
    let file_hunk_3_hash = Hunk::hash_diff(&file_hunk_3.diff_lines);

    let hunk_1_locks = dependencies.diffs.get(&file_hunk_1_hash).unwrap();
    assert_eq!(hunk_1_locks.len(), 2);
    assert_hunk_lock_matches_by_message(
        hunk_1_locks[0],
        "create file",
        &ctx,
        "Hunk depends on the commit that created the file",
    );
    assert_hunk_lock_matches_by_message(
        hunk_1_locks[1],
        "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        &ctx,
        "Hunk depends on the commit that deleted lines 3 and 4",
    );

    let hunk_2_locks = dependencies.diffs.get(&file_hunk_2_hash).unwrap();
    assert_eq!(hunk_2_locks.len(), 3);
    assert_hunk_lock_matches_by_message(
        hunk_2_locks[0],
        "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        &ctx,
        "Hunk depends on the commit that updated the line 7",
    );
    assert_hunk_lock_matches_by_message(
        hunk_2_locks[1],
        "create file",
        &ctx,
        "Hunk depends on the commit that created the file",
    );
    assert_hunk_lock_matches_by_message(
        hunk_2_locks[2],
        "modify lines 4 and 8",
        &ctx,
        "Hunk depends on the commit updated line 8",
    );

    let hunk_3_locks = dependencies.diffs.get(&file_hunk_3_hash).unwrap();
    assert_eq!(hunk_3_locks.len(), 1);
    assert_hunk_lock_matches_by_message(
        hunk_3_locks[0],
        "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        &ctx,
        "hunk 3",
    );

    Ok(())
}

#[test]
fn dependencies_ignore_merge_commits() -> Result<()> {
    let ctx = command_ctx_after_updates("merge-commit")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;

    let file_path = Path::new("file");
    let file_hunk_1 = GitHunk {
        old_start: 5,
        old_lines: 1,
        new_start: 5,
        new_lines: 1,
        diff_lines: "@@ -5,1 +5,1 @@
-update line 5
+update line 5 again
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let file_hunk_2 = GitHunk {
        old_start: 8,
        old_lines: 1,
        new_start: 8,
        new_lines: 1,
        diff_lines: "@@ -8,1 +8,1 @@
-update line 8
+update line 8 again
"
        .into(),
        binary: false,
        change_type: ChangeType::Modified,
    };

    let base_diffs: HashMap<PathBuf, Vec<GitHunk>> = HashMap::from([(
        file_path.to_path_buf(),
        vec![file_hunk_1.clone(), file_hunk_2.clone()],
    )]);

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &base_diffs,
        &test_ctx.all_stacks,
    )?;

    let commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_eq!(
        commit_dependencies.len(),
        0,
        "There are no commit interdependencies"
    );

    assert_eq!(
        dependencies.diffs.len(),
        1,
        "Only one diff depends on commits"
    );
    let file_hunk_1_hash = Hunk::hash_diff(&file_hunk_1.diff_lines);
    let file_hunk_2_hash = Hunk::hash_diff(&file_hunk_2.diff_lines);

    let hunk_1_locks = dependencies.diffs.get(&file_hunk_1_hash);
    assert_eq!(
        hunk_1_locks, None,
        "Hunk 1 should not have any dependencies, because it only intersects with a merge commit"
    );

    let hunk_2_locks = dependencies.diffs.get(&file_hunk_2_hash).unwrap();
    assert_eq!(hunk_2_locks.len(), 1, "Hunk 2 should have one dependency");
    assert_hunk_lock_matches_by_message(
        hunk_2_locks[0],
        "update line 8 and delete the line after 7",
        &ctx,
        "hunk 2",
    );

    Ok(())
}

#[test]
fn dependencies_handle_complex_branch_checkout() -> Result<()> {
    // This test ensures that checking out branches with *complex* histories
    // does not cause the dependency calculation to fail.
    //
    // The *complexity* of the branch is that is contains a merge commit from itself to itself,
    // mimicking checking-out a remote branch that had PRs merged to it.
    let ctx = command_ctx("complex-branch-checkout")?;
    let test_ctx = test_ctx(&ctx)?;
    let default_target = test_ctx.virtual_branches.get_default_target()?;
    let my_stack = &test_ctx.stack;

    let dependencies = compute_workspace_dependencies(
        &ctx,
        &default_target.sha,
        &HashMap::new(),
        &test_ctx.all_stacks,
    )?;

    let commit_dependencies = dependencies.commit_dependencies.get(&my_stack.id).unwrap();
    assert_commit_map_matches_by_message(
        commit_dependencies,
        HashMap::from([
            ("update a again", vec!["update a"]),
            ("update a", vec!["add a"]),
            ("Merge branch 'delete-b' into my_stack", vec!["add b"]),
        ]),
        &ctx,
        "Commit interdependencies correctly calculated. They should only pick up the merge commit when calculating dependencies",
    );

    Ok(())
}

// Test utility functions
// ---

fn assert_hunk_lock_matches_by_message(
    actual_hunk_lock: HunkLock,
    expected_commit_message: &str,
    ctx: &CommandContext,
    message: &str,
) {
    let commit_message = get_commit_message(ctx, actual_hunk_lock.commit_id);
    assert_eq!(
        commit_message, expected_commit_message,
        "should have same commit message - {}",
        message
    );
}

fn get_commit_message(ctx: &CommandContext, commit_id: git2::Oid) -> String {
    let repo = ctx.repo();
    let commit = repo.find_commit(commit_id).unwrap();
    let commit_message = commit.message().unwrap();
    commit_message.to_string()
}

fn assert_commit_map_matches_by_message(
    actual: &HashMap<git2::Oid, HashSet<git2::Oid>>,
    expected: HashMap<&str, Vec<&str>>,
    ctx: &CommandContext,
    message: &str,
) {
    let actual_messages = extract_commit_messages(actual, ctx);

    let expected: HashMap<String, Vec<String>> = expected
        .iter()
        .map(|(key, values)| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            (key.to_string(), values)
        })
        .collect();

    assert_eq!(
        actual_messages.len(),
        expected.len(),
        "should have same size {}",
        message
    );

    for (key, values) in expected {
        assert!(
            actual_messages.contains_key(&key),
            "should contain key '{}' - {}",
            key,
            message
        );
        let actual_values = actual_messages.get(&key).unwrap();
        assert_eq!(
            actual_values.len(),
            values.len(),
            "should have same length '{}' - {}",
            key,
            message
        );
        for value in values {
            assert!(
                actual_values.contains(&value.to_string()),
                "'{}' should contain '{}' - {}",
                key,
                value,
                message
            );
        }
    }
}

fn extract_commit_messages(
    actual: &HashMap<git2::Oid, HashSet<git2::Oid>>,
    ctx: &CommandContext,
) -> HashMap<String, Vec<String>> {
    let mut actual_messages: HashMap<String, Vec<String>> = HashMap::new();

    for (oid_key, oid_values) in actual {
        let repo = ctx.repo();
        let key_commit = repo.find_commit(*oid_key).unwrap();
        let key_message = key_commit.message().unwrap().trim().to_string();
        let actual_values: Vec<String> = oid_values
            .iter()
            .map(|oid| {
                let value_commit = repo.find_commit(*oid).unwrap();
                let value_commit_message = value_commit.message().unwrap().trim();
                value_commit_message.to_string()
            })
            .collect();

        actual_messages.insert(key_message, actual_values);
    }
    actual_messages
}

fn command_ctx(name: &str) -> Result<CommandContext> {
    gitbutler_testsupport::read_only::fixture("dependencies.sh", name)
}

fn command_ctx_after_updates(name: &str) -> Result<CommandContext> {
    gitbutler_testsupport::read_only::fixture("dependencies-after-updates.sh", name)
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_stacks()?;
    let stack = branches.iter().find(|b| b.name == "my_stack").unwrap();

    Ok(TestContext {
        stack: stack.clone(),
        all_stacks: branches,
        virtual_branches: handle,
    })
}

struct TestContext {
    stack: gitbutler_stack::Stack,
    all_stacks: Vec<gitbutler_stack::Stack>,
    virtual_branches: VirtualBranchesHandle,
}
