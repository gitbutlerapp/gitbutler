use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::LocalRefname;

use super::*;
#[test]
fn no_conflicts() {
    let Test { repo, ctx, .. } = &mut Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.simulate_push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert!(stacks.is_empty());

    let mut guard = ctx.exclusive_worktree_access();
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .map(|o| o.0)
    .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].0, branch_id);
    assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
    assert_eq!(stacks[0].1.branch_details[0].commits[0].message, "first");
}

#[test]
#[ignore = "new apply path still aborts on conflicting uncommitted worktree changes"]
fn conflicts_with_uncommited() {
    let Test { repo, ctx, .. } = &mut Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.simulate_push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // create a local branch that conflicts with remote
    {
        std::fs::write(repo.path().join("file.txt"), "conflict").unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        )
        .unwrap();
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
    };

    // branch should be created unapplied, because of the conflict

    let mut guard = ctx.exclusive_worktree_access();
    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .map(|o| o.0)
    .unwrap();
    drop(guard);
    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == new_branch_id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);
}

#[test]
fn conflicts_with_commited() {
    let Test { repo, ctx, .. } = &mut Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.simulate_push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // create a local branch that conflicts with remote
    {
        std::fs::write(repo.path().join("file.txt"), "conflict").unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);

        super::create_commit(ctx, stack_entry.id, "hej").unwrap();
    };

    // branch should be created unapplied, because of the conflict

    let mut guard = ctx.exclusive_worktree_access();
    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .map(|o| o.0)
    .unwrap();
    drop(guard);
    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == new_branch_id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);
}

#[test]
fn from_default_target() {
    let Test { ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // branch should be created unapplied, because of the conflict

    let mut guard = ctx.exclusive_worktree_access();
    let error = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .unwrap_err()
    .to_string();
    drop(guard);

    assert_eq!(
        error,
        "Cannot add the target 'refs/remotes/origin/master' branch to its own workspace"
    );
}

#[test]
fn from_non_existent_branch() {
    let Test { ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // branch should be created unapplied, because of the conflict

    let mut guard = ctx.exclusive_worktree_access();
    let error = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .unwrap_err()
    .to_string();
    drop(guard);

    assert_eq!(
        error,
        "The reference 'refs/remotes/origin/branch' did not exist"
    );
}

#[test]
fn from_state_remote_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "branch commit").unwrap();
        repo.commit_all("branch commit");
        repo.simulate_push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());

        // make remote branch stale
        std::fs::write(repo.path().join("antoher_file.txt"), "master commit").unwrap();
        repo.commit_all("master commit");
        repo.push();
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let _ = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
    assert_eq!(
        stacks[0].1.branch_details[0].commits[0].message,
        "branch commit"
    );
}
