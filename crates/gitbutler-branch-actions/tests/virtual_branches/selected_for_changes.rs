use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

use super::*;

#[test]
fn unapplying_selected_branch_selects_anther() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file one.txt"), "").unwrap();

    // first branch should be created as default
    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // if default branch exists, new branch should not be created as default
    let stack_entry_2 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    let b = branches.iter().find(|b| b.id == stack_entry_1.id).unwrap();

    let b2 = branches.iter().find(|b| b.id == stack_entry_2.id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    gitbutler_branch_actions::save_and_unapply_virutal_branch(ctx, stack_entry_1.id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, b2.id);
    assert!(branches[0].selected_for_changes);
    assert!(branches[0].active);
}

#[test]
fn deleting_selected_branch_selects_anther() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // first branch should be created as default
    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // if default branch exists, new branch should not be created as default
    let stack_entry_2 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    let b = branches.iter().find(|b| b.id == stack_entry.id).unwrap();

    let b2 = branches.iter().find(|b| b.id == stack_entry_2.id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    gitbutler_branch_actions::unapply_without_saving_virtual_branch(ctx, stack_entry.id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, b2.id);
    assert!(branches[0].selected_for_changes);
}

#[test]
fn create_virtual_branch_should_set_selected_for_changes() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // first branch should be created as default
    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();
    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert!(branch.selected_for_changes);

    // if default branch exists, new branch should not be created as default
    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();
    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly don't make this one default
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly make this one default
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert!(branch.selected_for_changes);
}

#[test]
fn update_virtual_branch_should_reset_selected_for_changes() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();
    let b1 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_1.id)
        .unwrap();
    assert!(b1.selected_for_changes);

    let stack_entry_2 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();
    let b2 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_2.id)
        .unwrap();
    assert!(!b2.selected_for_changes);

    gitbutler_branch_actions::update_virtual_branch(
        ctx,
        BranchUpdateRequest {
            id: stack_entry_2.id,
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    let b1 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_1.id)
        .unwrap();
    assert!(!b1.selected_for_changes);

    let b2 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_2.id)
        .unwrap();
    assert!(b2.selected_for_changes);
}

#[test]
fn unapply_virtual_branch_should_reset_selected_for_changes() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry_1 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let b1 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_1.id)
        .unwrap();
    assert!(b1.selected_for_changes);

    let stack_entry_2 =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let b2 = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry_2.id)
        .unwrap();
    assert!(!b2.selected_for_changes);

    gitbutler_branch_actions::save_and_unapply_virutal_branch(ctx, stack_entry_1.id).unwrap();

    assert!(gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .any(|b| b.selected_for_changes && b.id != stack_entry_1.id))
}

#[test]
fn hunks_distribution() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 1);

    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    std::fs::write(repository.path().join("another_file.txt"), "content").unwrap();
    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 1);
}

#[test]
fn applying_first_branch() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);

    let unapplied_branch =
        gitbutler_branch_actions::save_and_unapply_virutal_branch(ctx, branches[0].id).unwrap();
    let unapplied_branch = Refname::from_str(&unapplied_branch).unwrap();
    gitbutler_branch_actions::create_virtual_branch_from_branch(ctx, &unapplied_branch, None, None)
        .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    assert!(branches[0].active);
    assert!(branches[0].selected_for_changes);
}

// This test was written in response to issue #4148, to ensure the appearence
// of a locked hunk doesn't drag along unrelated hunks to its branch.
#[test]
fn new_locked_hunk_without_modifying_existing() {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 9);
    repository.commit_all("first commit");
    repository.push();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    lines[0] = "modification 1".to_string();
    repository.write_file("file.txt", &lines);

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 1);

    gitbutler_branch_actions::create_commit(ctx, branches[0].id, "second commit", None)
        .expect("failed to create commit");

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[0].series[0].clone().unwrap().patches.len(), 1);

    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    lines[8] = "modification 2".to_string();
    repository.write_file("file.txt", &lines);

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[1].files.len(), 1);

    lines[0] = "modification 3".to_string();
    repository.write_file("file.txt", &lines);
    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 1);
}
