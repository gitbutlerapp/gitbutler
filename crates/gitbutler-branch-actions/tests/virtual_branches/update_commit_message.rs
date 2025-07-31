use but_workspace::ui::PushStatus;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn head() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap()
    };

    {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap()
    };

    let commit_three_oid = {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap()
    };
    let commit_three = repo.find_commit(commit_three_oid).unwrap();
    let before_change_id = &commit_three.change_id();

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
    let commit = repo
        .find_commit(b.branch_details[0].commits[0].id.to_git2())
        .unwrap();

    // make sure the SHA changed, but the change ID did not
    assert_ne!(&commit_three.id(), &commit.id());
    assert_eq!(before_change_id, &commit.change_id());

    assert_eq!(
        messages,
        vec!["commit three updated", "commit two", "commit one"]
    );
}

#[test]
fn middle() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap()
    };

    let commit_two_oid = {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap()
    };

    {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap()
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
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    gitbutler_project::update_with_path(
        data_dir.as_ref().unwrap(),
        &projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        },
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap()
    };

    gitbutler_branch_actions::stack::push_stack(
        ctx,
        stack_entry.id,
        false,
        stack_entry.name().map(|n| n.to_string()).unwrap(),
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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let branch_id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch_id.id, "commit one", None).unwrap()
    };

    {
        fs::write(repo.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch_id.id, "commit two", None).unwrap()
    };

    {
        fs::write(repo.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch_id.id, "commit three", None).unwrap()
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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let branch_id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, branch_id.id, "commit one", None).unwrap()
    };

    assert_eq!(
        gitbutler_branch_actions::update_commit_message(ctx, branch_id.id, commit_one_oid, "",)
            .unwrap_err()
            .to_string(),
        "commit message can not be empty"
    );
}
