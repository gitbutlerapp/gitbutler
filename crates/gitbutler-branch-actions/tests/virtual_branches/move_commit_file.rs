use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_stack::{BranchCreateRequest, BranchOwnershipClaims};

use super::*;

#[test]
fn move_file_down() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();
    let commit1 = repository.find_commit(commit1_id).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap();
    let commit2 = repository.find_commit(commit2_id).unwrap();

    // amend another hunk
    let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    gitbutler_branch_actions::move_commit_file(
        project, branch_id, commit2_id, commit1_id, &to_amend,
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
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

#[test]
fn move_file_up() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    let commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    // create commit
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap();

    // amend another hunk
    let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    gitbutler_branch_actions::move_commit_file(
        project, branch_id, commit1_id, commit2_id, &to_amend,
    )
    .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
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
#[test]
fn move_file_up_overlapping_hunks() {
    let Test {
        repository,
        project_id,

        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())

        .unwrap();

    let branch_id = gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())

        .unwrap();

    // create bottom commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)

        .unwrap();

    // create middle commit one
    fs::write(repository.path().join("file2.txt"), "content2\ncontent2a\n").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)

        .unwrap();

    // create middle commit two
    fs::write(
        repository.path().join("file2.txt"),
        "content2\ncontent2a\ncontent2b\ncontent2c\ncontent2d",
    )
    .unwrap();
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let commit3_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)

        .unwrap();

    // create top commit
    fs::write(repository.path().join("file5.txt"), "content5").unwrap();
    let _commit4_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None, false)

        .unwrap();

    // move one line from middle commit two up to middle commit one
    let to_amend: BranchOwnershipClaims = "file2.txt:1-6".parse().unwrap();
    gitbutler_branch_actions::move_commit_file(project, branch_id, commit2_id, commit3_id, &to_amend)

        .unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)

        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    assert_eq!(branch.commits.len(), 4);
    //
}
 */
