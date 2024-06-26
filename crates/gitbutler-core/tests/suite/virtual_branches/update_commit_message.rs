use git::CommitExt;

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

    let commit_three_oid = {
        fs::write(repository.path().join("file three.txt"), "").unwrap();
        controller
            .create_commit(*project_id, branch_id, "commit three", None, false)
            .await
            .unwrap()
    };
    let commit_three = repository.find_commit(commit_three_oid).unwrap();
    let before_change_id = &commit_three.change_id();

    controller
        .update_commit_message(
            *project_id,
            branch_id,
            commit_three_oid,
            "commit three updated",
        )
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

    // get the last commit
    let commit = repository.find_commit(branch.head).unwrap();

    // make sure the SHA changed, but the change ID did not
    assert_ne!(&commit_three.id(), &commit.id());
    assert_eq!(before_change_id, &commit.change_id());

    assert_eq!(
        descriptions,
        vec!["commit three updated", "commit two", "commit one"]
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

    controller
        .update_commit_message(*project_id, branch_id, commit_two_oid, "commit two updated")
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
        vec!["commit three", "commit two updated", "commit one"]
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

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
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

    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    controller
        .update_commit_message(*project_id, branch_id, commit_one_oid, "commit one updated")
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
    assert_eq!(descriptions, vec!["commit one updated"]);
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

    let commit_one_oid = {
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

    assert_eq!(
        controller
            .update_commit_message(*project_id, branch_id, commit_one_oid, "commit one updated",)
            .await
            .unwrap_err()
            .to_string(),
        "force push not allowed"
    );
}

#[tokio::test]
async fn root() {
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

    controller
        .update_commit_message(*project_id, branch_id, commit_one_oid, "commit one updated")
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
        vec!["commit three", "commit two", "commit one updated"]
    );
}

#[tokio::test]
async fn empty() {
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
            .update_commit_message(*project_id, branch_id, commit_one_oid, "",)
            .await
            .unwrap_err()
            .to_string(),
        "commit message can not be empty"
    );
}
