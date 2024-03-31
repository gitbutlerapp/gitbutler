use std::str::FromStr;

use gitbutler_core::{
    git,
    virtual_branches::{branch, errors, BranchId},
};

use crate::suite::virtual_branches::Test;

#[tokio::test]
async fn no_diffs() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    controller
        .move_commit(project_id, &target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == source_branch_id)
        .unwrap();

    assert_eq!(destination_branch.commits.len(), 1);
    assert_eq!(destination_branch.files.len(), 0);
    assert_eq!(source_branch.commits.len(), 0);
    assert_eq!(source_branch.files.len(), 0);
}

#[tokio::test]
async fn diffs_on_source_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    controller
        .move_commit(project_id, &target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == source_branch_id)
        .unwrap();

    assert_eq!(destination_branch.commits.len(), 1);
    assert_eq!(destination_branch.files.len(), 0);
    assert_eq!(source_branch.commits.len(), 0);
    assert_eq!(source_branch.files.len(), 1);
}

#[tokio::test]
async fn diffs_on_target_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(
            project_id,
            &branch::BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    controller
        .move_commit(project_id, &target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == source_branch_id)
        .unwrap();

    assert_eq!(destination_branch.commits.len(), 1);
    assert_eq!(destination_branch.files.len(), 1);
    assert_eq!(source_branch.commits.len(), 0);
    assert_eq!(source_branch.files.len(), 0);
}

#[tokio::test]
async fn locked_hunks_on_source_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "locked content").unwrap();

    let target_branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    assert!(matches!(
        controller
            .move_commit(project_id, &target_branch_id, commit_oid)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(errors::MoveCommitError::SourceLocked)
    ));
}

#[tokio::test]
async fn no_commit() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    assert!(matches!(
        controller
            .move_commit(
                project_id,
                &target_branch_id,
                git::Oid::from_str("a99c95cca7a60f1a2180c2f86fb18af97333c192").unwrap()
            )
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(errors::MoveCommitError::CommitNotFound(_))
    ));
}

#[tokio::test]
async fn no_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(project_id, &source_branch_id, "commit", None, false)
        .await
        .unwrap();

    assert!(matches!(
        controller
            .move_commit(project_id, &BranchId::generate(), commit_oid)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(errors::MoveCommitError::BranchNotFound(_))
    ));
}
