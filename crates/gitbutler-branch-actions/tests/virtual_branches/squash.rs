use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

use super::*;

#[test]
fn head() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap()
    };

    {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap()
    };

    let commit_four_oid = {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None).unwrap()
    };

    let commit_four_parent_oid = repository
        .find_commit(commit_four_oid)
        .unwrap()
        .parent(0)
        .unwrap()
        .id();

    gitbutler_branch_actions::squash_commits(
        project,
        branch_id,
        vec![commit_four_oid],
        commit_four_parent_oid,
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
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
        vec!["commit three\ncommit four", "commit two", "commit one"]
    );
}

#[test]
fn middle() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap()
    };

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None).unwrap()
    };

    let commit_two_parent_oid = repository
        .find_commit(commit_two_oid)
        .unwrap()
        .parent(0)
        .unwrap()
        .id();

    gitbutler_branch_actions::squash_commits(
        project,
        branch_id,
        vec![commit_two_oid],
        commit_two_parent_oid,
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
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
        vec!["commit four", "commit three", "commit one\ncommit two"]
    );
}

#[test]
fn forcepush_allowed() {
    let Test {
        repository,
        project_id,
        project,
        projects,
        ..
    } = &Test::default();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap()
    };

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None).unwrap()
    };

    gitbutler_branch_actions::stack::push_stack(project, branch_id, false).unwrap();

    let commit_two_parent_oid = repository
        .find_commit(commit_two_oid)
        .unwrap()
        .parent(0)
        .unwrap()
        .id();

    gitbutler_branch_actions::squash_commits(
        project,
        branch_id,
        vec![commit_two_oid],
        commit_two_parent_oid,
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
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
        vec!["commit four", "commit three", "commit one\ncommit two"]
    );
    assert!(branch.requires_force);
}

#[test]
fn forcepush_forbidden() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::update_virtual_branch(
        project,
        BranchUpdateRequest {
            id: branch_id,
            allow_rebasing: Some(false),
            ..Default::default()
        },
    )
    .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap()
    };

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None).unwrap()
    };

    // TODO: flag the old one as deprecated
    gitbutler_branch_actions::stack::push_stack(project, branch_id, false).unwrap();

    let commit_two_parent_oid = repository
        .find_commit(commit_two_oid)
        .unwrap()
        .parent(0)
        .unwrap()
        .id();

    assert_eq!(
        gitbutler_branch_actions::squash_commits(
            project,
            branch_id,
            vec![commit_two_oid],
            commit_two_parent_oid,
        )
        .unwrap_err()
        .to_string(),
        format!(
            "Force push is now allowed. Source commits with id {} has already been pushed",
            commit_two_oid
        )
    );
}
