use gitbutler_core::virtual_branches::{branch, BranchId};

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    controller
        .move_commit(*project_id, target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(*project_id)
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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    controller
        .move_commit(*project_id, target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(*project_id)
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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(
            *project_id,
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
        .move_commit(*project_id, target_branch_id, commit_oid)
        .await
        .unwrap();

    let destination_branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = controller
        .list_virtual_branches(*project_id)
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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "locked content").unwrap();

    let target_branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    assert_eq!(
        controller
            .move_commit(*project_id, target_branch_id, commit_oid)
            .await
            .unwrap_err()
            .to_string(),
        "the source branch contains hunks locked to the target commit"
    );
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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let target_branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let commit_id_hex = "a99c95cca7a60f1a2180c2f86fb18af97333c192";
    assert_eq!(
        controller
            .move_commit(
                *project_id,
                target_branch_id,
                git2::Oid::from_str(commit_id_hex).unwrap()
            )
            .await
            .unwrap_err()
            .to_string(),
        format!("commit {commit_id_hex} to be moved could not be found")
    );
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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid = controller
        .create_commit(*project_id, source_branch_id, "commit", None, false)
        .await
        .unwrap();

    let id = BranchId::generate();
    assert_eq!(
        controller
            .move_commit(*project_id, id, commit_oid)
            .await
            .unwrap_err()
            .to_string(),
        format!("branch {id} is not among applied branches")
    );
}
