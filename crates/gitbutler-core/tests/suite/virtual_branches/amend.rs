use super::*;

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

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();

    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        controller
            .amend(*project_id, branch_id, commit_id, &to_amend)
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
        assert!(branch.requires_force);
        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(branch.commits[0].files.len(), 2);
    }
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

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();

    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            controller
                .amend(*project_id, branch_id, commit_oid, &to_amend)
                .await
                .unwrap_err()
                .to_string(),
            "force-push is not allowed"
        );
    }
}

#[tokio::test]
async fn non_locked_hunk() {
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

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
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
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits[0].files.len(), 1);

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        controller
            .amend(*project_id, branch_id, commit_oid, &to_amend)
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
        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(branch.commits[0].files.len(), 2);
    }
}

#[tokio::test]
async fn locked_hunk() {
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

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
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
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(
        branch.commits[0].files[0].hunks[0].diff,
        "@@ -0,0 +1 @@\n+content\n\\ No newline at end of file\n"
    );

    {
        // amend another hunk
        fs::write(repository.path().join("file.txt"), "more content").unwrap();
        let to_amend: branch::BranchOwnershipClaims = "file.txt:1-2".parse().unwrap();
        controller
            .amend(*project_id, branch_id, commit_oid, &to_amend)
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

        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(branch.commits[0].files.len(), 1);
        assert_eq!(
            branch.commits[0].files[0].hunks[0].diff,
            "@@ -0,0 +1 @@\n+more content\n\\ No newline at end of file\n"
        );
    }
}

#[tokio::test]
async fn non_existing_ownership() {
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

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
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
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits[0].files.len(), 1);

    {
        // amend non existing hunk
        let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            controller
                .amend(*project_id, branch_id, commit_oid, &to_amend)
                .await
                .unwrap_err()
                .to_string(),
            "target ownership not found"
        );
    }
}
