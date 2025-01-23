use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::VirtualBranch;
use gitbutler_id::id::Id;
use gitbutler_stack::Stack;

use super::*;

#[test]
fn should_lock_updated_hunks() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    {
        // by default, hunks are not locked
        repository.write_file("file.txt", &["content".to_string()]);

        let branch = get_virtual_branch(ctx, stack_entry.id);
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "test", None).unwrap();

    {
        // change in the committed hunks leads to hunk locking
        repository.write_file("file.txt", &["updated content".to_string()]);

        let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.id == stack_entry.id)
            .unwrap();
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(branch.files[0].hunks[0].locked);
    }
}

#[test]
fn should_reset_into_same_branch() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 7);
    commit_and_push_initial(repository);

    let base_branch = gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default()).unwrap();

    let stack_entry_2 = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    lines[0] = "change 1".to_string();
    repository.write_file("file.txt", &lines);

    gitbutler_branch_actions::create_commit(ctx, stack_entry_2.id, "commit to branch 2", None)
        .unwrap();

    let files = get_virtual_branch(ctx, stack_entry_2.id).files;
    assert_eq!(files.len(), 0);

    // Set target to branch 1 and verify the file resets into branch 2.
    gitbutler_branch_actions::update_virtual_branch(
        ctx,
        BranchUpdateRequest {
            id: stack_entry_2.id,
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    gitbutler_branch_actions::reset_virtual_branch(ctx, stack_entry_2.id, base_branch.base_sha)
        .unwrap();

    let files = get_virtual_branch(ctx, stack_entry_2.id).files;
    assert_eq!(files.len(), 1);
}

fn commit_and_push_initial(repository: &TestProject) {
    repository.commit_all("initial commit");
    repository.push();
}

fn get_virtual_branch(ctx: &CommandContext, branch_id: Id<Stack>) -> VirtualBranch {
    gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap()
}
