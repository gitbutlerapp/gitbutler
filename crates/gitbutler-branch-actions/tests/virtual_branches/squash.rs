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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap()
    };

    let commit_four_oid = {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None, false)
            .unwrap()
    };

    gitbutler_branch_actions::squash(project, branch_id, commit_four_oid).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let descriptions = branch
        .commits
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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap()
    };

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None, false)
            .unwrap()
    };

    gitbutler_branch_actions::squash(project, branch_id, commit_two_oid).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let descriptions = branch
        .commits
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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap()
    };

    gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None, false)
            .unwrap()
    };

    gitbutler_branch_actions::squash(project, branch_id, commit_two_oid).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let descriptions = branch
        .commits
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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap()
    };

    gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None, false)
            .unwrap()
    };

    assert_eq!(
        gitbutler_branch_actions::squash(project, branch_id, commit_two_oid)
            .unwrap_err()
            .to_string(),
        "force push not allowed"
    );
}

#[test]
fn root_forbidden() {
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

    let commit_one_oid = {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap()
    };

    assert_eq!(
        gitbutler_branch_actions::squash(project, branch_id, commit_one_oid)
            .unwrap_err()
            .to_string(),
        "can not squash root commit"
    );
}
