use but_oxidize::OidExt as _;
use but_workspace::ui::PushStatus;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn head() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit one").unwrap()
    };

    {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit two").unwrap()
    };

    let commit_three_oid = {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit three").unwrap()
    };
    let before_change_id = {
        let gix_repo = ctx.repo.get().unwrap();
        let commit_three = gix_repo.find_commit(commit_three_oid.to_gix()).unwrap();
        commit_three.change_id()
    };

    gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_entry.id,
        commit_three_oid,
        "commit three updated",
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    let messages = b
        .branch_details
        .iter()
        .flat_map(|branch| branch.commits.iter().map(|c| c.message.clone()))
        .collect::<Vec<_>>();

    // get the last commit
    let gix_repo = ctx.repo.get().unwrap();
    let commit = gix_repo
        .find_commit(b.branch_details[0].commits[0].id)
        .unwrap();

    // make sure the SHA changed, but the change ID did not
    assert_ne!(commit_three_oid.to_gix(), commit.id());
    assert_eq!(before_change_id, commit.change_id());

    assert_eq!(
        messages,
        vec!["commit three updated", "commit two", "commit one"]
    );
}

#[test]
fn middle() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit one").unwrap()
    };

    let commit_two_oid = {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit two").unwrap()
    };

    {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit three").unwrap()
    };

    gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_entry.id,
        commit_two_oid,
        "commit two updated",
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    let messages = b
        .branch_details
        .iter()
        .flat_map(|branch| branch.commits.iter().map(|c| c.message.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["commit three", "commit two updated", "commit one"]
    );
}

#[test]
fn forcepush_allowed() {
    let Test {
        data_dir,
        repo,
        project_id,

        ctx,
        ..
    } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    gitbutler_project::update_with_path(
        data_dir.as_ref().unwrap(),
        projects::UpdateRequest::default_with_id(*project_id),
    )
    .unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        super::create_commit(ctx, stack_entry.id, "commit one").unwrap()
    };

    gitbutler_branch_actions::stack::push_stack(
        ctx,
        stack_entry.id,
        false,
        false,
        stack_entry.name().map(|n| n.to_string()).unwrap(),
        false, // run_hooks
        vec![],
    )
    .unwrap();

    gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_entry.id,
        commit_one_oid,
        "commit one updated",
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    let messages = b
        .branch_details
        .iter()
        .flat_map(|branch| branch.commits.iter().map(|c| c.message.clone()))
        .collect::<Vec<_>>();

    assert_eq!(messages, vec!["commit one updated"]);
    assert!(matches!(
        b.push_status,
        PushStatus::UnpushedCommitsRequiringForce
    ));
}

#[test]
fn root() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let branch_id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        super::create_commit(ctx, branch_id.id, "commit one").unwrap()
    };

    {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        super::create_commit(ctx, branch_id.id, "commit two").unwrap()
    };

    {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        super::create_commit(ctx, branch_id.id, "commit three").unwrap()
    };

    gitbutler_branch_actions::update_commit_message(
        ctx,
        branch_id.id,
        commit_one_oid,
        "commit one updated",
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == branch_id.id)
        .unwrap();
    let messages = b
        .branch_details
        .iter()
        .flat_map(|branch| branch.commits.iter().map(|c| c.message.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["commit three", "commit two", "commit one updated"]
    );
}

#[test]
fn empty() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let branch_id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        super::create_commit(ctx, branch_id.id, "commit one").unwrap()
    };

    assert_eq!(
        gitbutler_branch_actions::update_commit_message(ctx, branch_id.id, commit_one_oid, "",)
            .unwrap_err()
            .to_string(),
        "commit message can not be empty"
    );
}
