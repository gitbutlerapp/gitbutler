use but_testsupport::legacy::stack_details;

use super::*;

#[test]
fn should_unapply_diff() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // write some
    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let stacks = stack_details(ctx);
    let c = super::create_commit(ctx, stacks[0].0, "asdf");
    assert!(c.is_ok());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::unapply_stack(ctx, guard.write_permission(), stacks[0].0, Vec::new())
        .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);
    assert!(!repo.path().join("file.txt").exists());

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.local_repo.statuses(Some(&mut opts)).unwrap();
    assert!(statuses.is_empty());

    let refnames = repo
        .references()
        .into_iter()
        .filter_map(|reference| reference.name().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
}

#[test]
fn should_unapply_diff_cv3() {
    let Test { repo, ctx, .. } =
        &mut Test::new_with_settings(|settings| settings.feature_flags.cv3 = true);

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let stacks = stack_details(ctx);
    let c = super::create_commit(ctx, stacks[0].0, "asdf");
    assert!(c.is_ok());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::unapply_stack(ctx, guard.write_permission(), stacks[0].0, Vec::new())
        .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);
    assert!(!repo.path().join("file.txt").exists());

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.local_repo.statuses(Some(&mut opts)).unwrap();
    assert!(statuses.is_empty());

    assert_eq!(
        repo.local_repo.head().unwrap().name(),
        Some("refs/heads/gitbutler/workspace")
    );
    assert!(
        repo.local_repo
            .find_reference("refs/heads/gitbutler/workspace")
            .is_ok()
    );
}
