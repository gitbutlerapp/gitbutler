use super::*;

#[tokio::test]
async fn head() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit one", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit two", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit three", None, false)
            .await
            .unwrap()
    };

    let commit_four_oid = {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit four", None, false)
            .await
            .unwrap()
    };

    controller
        .squash(*project_id, branch_id, commit_four_oid)
        .await
        .unwrap();

    let branch = controller
        .list_virtual_branches(*project_id)
        .await
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

#[tokio::test]
async fn middle() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit one", None, false)
            .await
            .unwrap()
    };

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit two", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit three", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit four", None, false)
            .await
            .unwrap()
    };

    controller
        .squash(*project_id, branch_id, commit_two_oid)
        .await
        .unwrap();

    let branch = controller
        .list_virtual_branches(*project_id)
        .await
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

#[tokio::test]
async fn forcepush_allowed() {
    let Test {
        repository,
        project_id,
        controller,
        projects,
        ..
    } = &Test::default();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .await
        .unwrap();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit one", None, false)
            .await
            .unwrap()
    };

    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit two", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit three", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit four", None, false)
            .await
            .unwrap()
    };

    controller
        .squash(*project_id, branch_id, commit_two_oid)
        .await
        .unwrap();

    let branch = controller
        .list_virtual_branches(*project_id)
        .await
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

#[tokio::test]
async fn forcepush_forbidden() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    controller
        .update_virtual_branch(
            *project_id,
            branch::BranchUpdateRequest {
                id: branch_id,
                allow_rebasing: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit one", None, false)
            .await
            .unwrap()
    };

    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    let commit_two_oid = {
        fs::write(repository.path().join("file two.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit two", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit three", None, false)
            .await
            .unwrap()
    };

    {
        fs::write(repository.path().join("file four.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit four", None, false)
            .await
            .unwrap()
    };

    assert_eq!(
        controller
            .squash(*project_id, branch_id, commit_two_oid)
            .await
            .unwrap_err()
            .to_string(),
        "force push not allowed"
    );
}

#[tokio::test]
async fn root_forbidden() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let commit_one_oid = {
        fs::write(repository.path().join("file one.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit one", None, false)
            .await
            .unwrap()
    };

    assert_eq!(
        controller
            .squash(*project_id, branch_id, commit_one_oid)
            .await
            .unwrap_err()
            .to_string(),
        "can not squash root commit"
    );
}
