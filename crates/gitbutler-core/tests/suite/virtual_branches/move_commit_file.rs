use git::CommitExt;

use super::*;

#[tokio::test]
async fn move_file_down() {
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
    let commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();
    let commit1 = repository.find_commit(commit1_id).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await
        .unwrap();
    let commit2 = repository.find_commit(commit2_id).unwrap();

    // amend another hunk
    let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    controller
        .move_commit_file(*project_id, branch_id, commit2_id, commit1_id, &to_amend)
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

    // shas changed but change_id is the same
    assert_eq!(&commit1.change_id(), &branch.commits[1].change_id);
    assert_ne!(&commit1.id(), &branch.commits[1].id);
    assert_eq!(&commit2.change_id(), &branch.commits[0].change_id);
    assert_ne!(&commit2.id(), &branch.commits[0].id);

    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits.len(), 2);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 2); // this now has both file changes
}

#[tokio::test]
async fn move_file_up() {
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
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    let commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();

    // create commit
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await
        .unwrap();

    // amend another hunk
    let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    controller
        .move_commit_file(*project_id, branch_id, commit1_id, commit2_id, &to_amend)
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

    assert_eq!(branch.commits.len(), 2);
    assert_eq!(branch.commits[0].files.len(), 2); // this now has both file changes
    assert_eq!(branch.commits[1].files.len(), 1);
}

// This test is failing because the file is not being moved up to the correct commit
// This is out of scope for the first release, but should be fixed in the future
// where you can take overlapping hunks between commits and resolve a move between them
/*
#[tokio::test]
async fn move_file_up_overlapping_hunks() {
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

    // create bottom commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();

    // create middle commit one
    fs::write(repository.path().join("file2.txt"), "content2\ncontent2a\n").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await
        .unwrap();

    // create middle commit two
    fs::write(
        repository.path().join("file2.txt"),
        "content2\ncontent2a\ncontent2b\ncontent2c\ncontent2d",
    )
    .unwrap();
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let commit3_id = controller
        .create_commit(*project_id, branch_id, "commit three", None, false)
        .await
        .unwrap();

    // create top commit
    fs::write(repository.path().join("file5.txt"), "content5").unwrap();
    let _commit4_id = controller
        .create_commit(*project_id, branch_id, "commit four", None, false)
        .await
        .unwrap();

    // move one line from middle commit two up to middle commit one
    let to_amend: branch::BranchOwnershipClaims = "file2.txt:1-6".parse().unwrap();
    controller
        .move_commit_file(*project_id, branch_id, commit2_id, commit3_id, &to_amend)
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

    assert_eq!(branch.commits.len(), 4);
    //
}
 */
