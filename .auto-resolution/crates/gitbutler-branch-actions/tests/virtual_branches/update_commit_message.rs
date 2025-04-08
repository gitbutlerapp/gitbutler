use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_commit::commit_ext::CommitExt;

use super::*;

#[test]
fn head() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
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

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    // get the last commit
    let commit = repo.find_commit(branch.head).unwrap();

    // make sure the SHA changed, but the change ID did not
    assert_ne!(&commit_three.id(), &commit.id());
    assert_eq!(before_change_id, &commit.change_id());

    assert_eq!(
        descriptions,
        vec!["commit three updated", "commit two", "commit one"]
    );
}

#[test]
fn middle() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
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

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        descriptions,
        vec!["commit three", "commit two updated", "commit one"]
    );
}

#[test]
fn forcepush_allowed() {
    let Test {
        repo,
        project_id,

        projects,
        ctx,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap()
    };

    #[allow(deprecated)]
    gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry.id, false, None).unwrap();

    gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_entry.id,
        commit_one_oid,
        "commit one updated",
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();
    assert_eq!(descriptions, vec!["commit one updated"]);
    assert!(branch.requires_force);
}

#[test]
fn forcepush_forbidden() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::update_virtual_branch(
        ctx,
        BranchUpdateRequest {
            id: stack_entry.id,
            allow_rebasing: Some(false),
            ..Default::default()
        },
    )
    .unwrap();

    let commit_one_oid = {
        fs::write(repo.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap()
    };

    #[allow(deprecated)]
    gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry.id, false, None).unwrap();

    assert_eq!(
        gitbutler_branch_actions::update_commit_message(
            ctx,
            stack_entry.id,
            commit_one_oid,
            "commit one updated",
        )
        .unwrap_err()
        .to_string(),
        "force push not allowed"
    );
}

#[test]
fn root() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
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

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id.id)
        .unwrap();

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        descriptions,
        vec!["commit three", "commit two", "commit one updated"]
    );
}

#[test]
fn empty() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
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
