use super::*;

#[tokio::test]
async fn reorder_commit_down() {
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
    let _commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await
        .unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await
        .unwrap();

    controller
        .reorder_commit(*project_id, branch_id, commit2_id, 1)
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
    assert_eq!(branch.commits[0].files.len(), 1); // this now has the 2 file changes
    assert_eq!(branch.commits[1].files.len(), 2); // and this has the single file change

    let descriptions = branch
        .commits
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(descriptions, vec!["commit one", "commit two"]);
}

#[tokio::test]
async fn reorder_commit_up() {
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

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let _commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await
        .unwrap();

    controller
        .reorder_commit(*project_id, branch_id, commit1_id, -1)
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
    assert_eq!(branch.commits[0].files.len(), 1); // this now has the 2 file changes
    assert_eq!(branch.commits[1].files.len(), 2); // and this has the single file change

    let descriptions = branch
        .commits
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(descriptions, vec!["commit one", "commit two"]);
}
