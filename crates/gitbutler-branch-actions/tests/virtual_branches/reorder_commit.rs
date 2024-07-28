use super::*;
use gitbutler_branch::BranchCreateRequest;

#[test]
fn reorder_commit_down() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = controller
        .create_commit(project, branch_id, "commit two", None, false)
        .unwrap();

    controller
        .reorder_commit(project, branch_id, commit2_id, 1)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
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

#[test]
fn reorder_commit_up() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit1_id = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let _commit2_id = controller
        .create_commit(project, branch_id, "commit two", None, false)
        .unwrap();

    controller
        .reorder_commit(project, branch_id, commit1_id, -1)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
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
