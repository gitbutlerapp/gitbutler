use crate::suite::virtual_branches::Test;
use gitbutler::virtual_branches::{branch, controller::ControllerError, errors::ResetBranchError};
use std::fs;

#[tokio::test]
async fn to_head() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let oid = {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid = controller
            .create_commit(project_id, &branch1_id, "commit", None, false)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    {
        // reset changes to head
        controller
            .reset_virtual_branch(project_id, &branch1_id, oid)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }
}

#[tokio::test]
async fn to_target() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    let base_branch = controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid = controller
            .create_commit(project_id, &branch1_id, "commit", None, false)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }

    {
        // reset changes to head
        controller
            .reset_virtual_branch(project_id, &branch1_id, base_branch.base_sha)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 0);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
    }
}

#[tokio::test]
async fn to_commit() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let first_commit_oid = {
        // commit some changes

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        let oid = controller
            .create_commit(project_id, &branch1_id, "commit", None, false)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    {
        // commit some more
        fs::write(repository.path().join("file.txt"), "more content").unwrap();

        let second_commit_oid = controller
            .create_commit(project_id, &branch1_id, "commit", None, false)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 2);
        assert_eq!(branches[0].commits[0].id, second_commit_oid);
        assert_eq!(branches[0].commits[1].id, first_commit_oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "more content"
        );
    }

    {
        // reset changes to the first commit
        controller
            .reset_virtual_branch(project_id, &branch1_id, first_commit_oid)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, first_commit_oid);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "more content"
        );
    }
}

#[tokio::test]
async fn to_non_existing() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        fs::write(repository.path().join("file.txt"), "content").unwrap();

        // commit changes
        let oid = controller
            .create_commit(project_id, &branch1_id, "commit", None, false)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].id, oid);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        oid
    };

    assert!(matches!(
        controller
            .reset_virtual_branch(
                project_id,
                &branch1_id,
                "fe14df8c66b73c6276f7bb26102ad91da680afcb".parse().unwrap()
            )
            .await,
        Err(ControllerError::Action(
            ResetBranchError::CommitNotFoundInBranch(_)
        ))
    ));
}
