use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn should_unapply_diff() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // write some
    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let stacks = stack_details(ctx);
    let c = gitbutler_branch_actions::create_commit(ctx, stacks[0].0, "asdf", None);
    assert!(c.is_ok());

    gitbutler_branch_actions::unapply_stack(ctx, stacks[0].0, Vec::new()).unwrap();

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
