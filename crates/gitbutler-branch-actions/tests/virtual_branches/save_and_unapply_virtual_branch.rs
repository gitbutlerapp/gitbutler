use super::*;

#[test]
fn unapply_with_data() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::unapply_stack(ctx, branches[0].id).unwrap();

    assert!(!repo.path().join("file.txt").exists());

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 0);
}

#[test]
fn delete_if_empty() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default()).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::unapply_stack(ctx, branches[0].id).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 0);
}
