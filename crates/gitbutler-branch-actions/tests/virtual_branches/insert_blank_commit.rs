use super::*;
use gitbutler_branch::BranchCreateRequest;

#[test]
fn insert_blank_commit_down() {
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

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id = controller
        .create_commit(project, branch_id, "commit three", None, false)
        .unwrap();

    controller
        .insert_blank_commit(project, branch_id, commit2_id, 1)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    assert_eq!(branch.commits.len(), 4);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 2);
    assert_eq!(branch.commits[2].files.len(), 0); // blank commit

    let descriptions = branch
        .commits
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        descriptions,
        vec!["commit three", "commit two", "", "commit one"]
    );
}

#[test]
fn insert_blank_commit_up() {
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

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id = controller
        .create_commit(project, branch_id, "commit three", None, false)
        .unwrap();

    controller
        .insert_blank_commit(project, branch_id, commit2_id, -1)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    assert_eq!(branch.commits.len(), 4);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 0); // blank commit
    assert_eq!(branch.commits[2].files.len(), 2);

    let descriptions = branch
        .commits
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        descriptions,
        vec!["commit three", "", "commit two", "commit one"]
    );
}
