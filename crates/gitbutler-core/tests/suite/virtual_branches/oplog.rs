use std::io::Write;

use gitbutler_core::ops::oplog::Oplog;

use super::*;

#[tokio::test]
async fn test_basic_oplog() {
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id = controller
        .create_commit(project_id, &branch_id, "commit one", None, false)
        .await
        .unwrap();

    // dont store large files
    let file_path = repository.path().join("large.txt");
    // write 33MB of random data in the file
    let mut file = std::fs::File::create(file_path).unwrap();
    for _ in 0..33 * 1024 {
        let data = [0u8; 1024];
        file.write_all(&data).unwrap();
    }

    // create commit with large file
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(project_id, &branch_id, "commit two", None, false)
        .await
        .unwrap();

    // Create conflict state
    let conflicts_path = repository.path().join(".git").join("conflicts");
    std::fs::write(&conflicts_path, "conflict A").unwrap();
    let base_merge_parent_path = repository.path().join(".git").join("base_merge_parent");
    std::fs::write(&base_merge_parent_path, "parent A").unwrap();

    // create state with conflict state
    let _empty_branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    std::fs::remove_file(&base_merge_parent_path).unwrap();
    std::fs::remove_file(&conflicts_path).unwrap();

    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id = controller
        .create_commit(project_id, &branch_id, "commit three", None, false)
        .await
        .unwrap();

    let branch = controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let branches = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.0.len(), 2);

    assert_eq!(branch.commits.len(), 3);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 3);

    let snapshots = project.list_snapshots(10, None).unwrap();

    let ops = snapshots
        .iter()
        .map(|c| &c.details.as_ref().unwrap().title)
        .collect::<Vec<_>>();

    assert_eq!(
        ops,
        vec![
            "CreateCommit",
            "CreateBranch",
            "CreateCommit",
            "CreateCommit",
            "CreateBranch",
            "SetBaseBranch",
        ]
    );

    project.restore_snapshot(snapshots[1].clone().id).unwrap();

    // restores the conflict files
    let file_lines = std::fs::read_to_string(&conflicts_path).unwrap();
    assert_eq!(file_lines, "conflict A");
    let file_lines = std::fs::read_to_string(&base_merge_parent_path).unwrap();
    assert_eq!(file_lines, "parent A");

    assert_eq!(snapshots[2].lines_added, 2);
    assert_eq!(snapshots[2].lines_removed, 0);

    project.restore_snapshot(snapshots[3].clone().id).unwrap();

    // the restore removed our new branch
    let branches = controller.list_virtual_branches(project_id).await.unwrap();
    assert_eq!(branches.0.len(), 1);

    // assert that the conflicts file was removed
    assert!(!&conflicts_path.try_exists().unwrap());

    // remove commit2_oid from odb
    let commit_str = &commit2_id.to_string();
    // find file in odb
    let file_path = repository
        .path()
        .join(".git")
        .join("objects")
        .join(&commit_str[..2]);
    let file_path = file_path.join(&commit_str[2..]);
    assert!(file_path.exists());
    // remove file
    std::fs::remove_file(file_path).unwrap();

    // try to look up that object
    let repo = git2::Repository::open(&project.path).unwrap();
    let commit = repo.find_commit(commit2_id.into());
    assert!(commit.is_err());

    project.restore_snapshot(snapshots[2].clone().id).unwrap();

    // test missing commits are recreated
    let commit = repo.find_commit(commit2_id.into());
    assert!(commit.is_ok());

    let file_path = repository.path().join("large.txt");
    assert!(file_path.exists());

    let file_path = repository.path().join("file.txt");
    let file_lines = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(file_lines, "content");
}

// test oplog.toml head is not a commit

#[tokio::test]
async fn test_oplog_head_corrupt() {
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let snapshots = project.list_snapshots(10, None).unwrap();
    assert_eq!(snapshots.len(), 1);

    // overwrite oplog head with a non-commit sha
    let file_path = repository.path().join(".git").join("operations-log.toml");
    fs::write(
        file_path,
        "head_sha = \"758d54f587227fba3da3b61fbb54a99c17903d59\"",
    )
    .unwrap();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // it should have just reset the oplog head, so only 1, not 2
    let snapshots = project.list_snapshots(10, None).unwrap();
    assert_eq!(snapshots.len(), 1);
}
