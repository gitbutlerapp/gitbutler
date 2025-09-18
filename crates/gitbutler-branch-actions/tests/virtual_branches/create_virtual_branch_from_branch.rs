use but_workspace::ui::CommitState;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::LocalRefname;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn integration() {
    let Test { repo, ctx, .. } =
        &Test::new_with_settings(|settings| settings.feature_flags.ws3 = false);

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let branch_name = {
        // make a remote branch

        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            ctx.project().exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        std::fs::write(repo.path().join("file.txt"), "first\n").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "first", None).unwrap();
        gitbutler_branch_actions::stack::push_stack(
            ctx,
            stack_entry.id,
            false,
            false,
            stack_entry.name().map(|n| n.to_string()).unwrap(),
            false, // run_hooks
        )
        .unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|d| d.0 == stack_entry.id)
            .unwrap();

        gitbutler_branch_actions::unapply_stack(ctx, stack_entry.id, Vec::new()).unwrap();

        Refname::from_str(&format!("refs/remotes/origin/{}", b.derived_name)).unwrap()
    };

    // checkout a existing remote branch
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx,
        &branch_name,
        None,
        Some(123),
    )
    .map(|o| o.0)
    .unwrap();

    {
        // add a commit
        std::fs::write(repo.path().join("file.txt"), "first\nsecond").unwrap();

        gitbutler_branch_actions::create_commit(ctx, branch_id, "second", None).unwrap();
    }

    {
        // meanwhile, there is a new commit on master
        repo.checkout(&"refs/heads/master".parse().unwrap());
        std::fs::write(repo.path().join("another.txt"), "").unwrap();
        repo.commit_all("another");
        repo.push_branch(&"refs/heads/master".parse().unwrap());
        repo.checkout(&"refs/heads/gitbutler/workspace".parse().unwrap());
    }

    {
        // merge branch into master
        gitbutler_branch_actions::stack::push_stack(
            ctx,
            branch_id,
            false,
            false,
            branch_name.simple_name(),
            false, // run_hooks
        )
        .unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|d| d.0 == branch_id)
            .unwrap();
        assert!(matches!(
            b.branch_details[0].commits[0].state,
            CommitState::LocalAndRemote(_)
        ));
        assert!(matches!(
            b.branch_details[0].commits[1].state,
            CommitState::LocalAndRemote(_)
        ));

        repo.rebase_and_merge(&branch_name);
    }

    {
        // should mark commits as integrated
        gitbutler_branch_actions::fetch_from_remotes(ctx, None).unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|d| d.0 == branch_id)
            .unwrap();

        assert_eq!(b.branch_details[0].pr_number, Some(123));
        assert!(matches!(
            b.branch_details[0].commits[0].state,
            CommitState::Integrated
        ));
        assert!(matches!(
            b.branch_details[0].commits[1].state,
            CommitState::Integrated
        ));
    }
}

#[test]
fn no_conflicts() {
    let Test { repo, ctx, .. } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert!(stacks.is_empty());

    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .map(|o| o.0)
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].0, branch_id);
    assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
    assert_eq!(stacks[0].1.branch_details[0].commits[0].message, "first");
}

#[test]
fn conflicts_with_uncommited() {
    let Test { repo, ctx, .. } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repo.path().join("file.txt"), "conflict").unwrap();

        let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            ctx.project().exclusive_worktree_access().write_permission(),
        )
        .unwrap();
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .map(|o| o.0)
    .unwrap();
    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == new_branch_id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);
}

#[test]
fn conflicts_with_commited() {
    let Test { repo, ctx, .. } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "first").unwrap();
        repo.commit_all("first");
        repo.push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repo.path().join("file.txt"), "conflict").unwrap();

        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            ctx.project().exclusive_worktree_access().write_permission(),
        )
        .unwrap();
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);

        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "hej", None).unwrap();
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .map(|o| o.0)
    .unwrap();
    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == new_branch_id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);
}

#[test]
fn from_default_target() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            None,
            None,
        )
        .unwrap_err()
        .to_string(),
        "cannot create a branch from default target"
    );
}

#[test]
fn from_non_existent_branch() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &"refs/remotes/origin/branch".parse().unwrap(),
            None,
            None,
        )
        .unwrap_err()
        .to_string(),
        "branch refs/remotes/origin/branch was not found"
    );
}

#[test]
fn from_state_remote_branch() {
    let Test { repo, ctx, .. } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repo.checkout(&branch_name);
        fs::write(repo.path().join("file.txt"), "branch commit").unwrap();
        repo.commit_all("branch commit");
        repo.push_branch(&branch_name);
        repo.checkout(&"refs/heads/master".parse().unwrap());

        // make remote branch stale
        std::fs::write(repo.path().join("antoher_file.txt"), "master commit").unwrap();
        repo.commit_all("master commit");
        repo.push();
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let _ = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
        None,
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
    assert_eq!(
        stacks[0].1.branch_details[0].commits[0].message,
        "branch commit"
    );
}
