use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;

use super::*;

#[test]
fn undo_commit_simple() -> anyhow::Result<()> {
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
    let _commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap();

    gitbutler_branch_actions::undo_commit(project, branch_id, commit2_id).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    // should be two uncommitted files now (file2.txt and file3.txt)
    assert_eq!(branch.files.len(), 2);
    assert_eq!(branch.series[0].clone().unwrap().patches.len(), 2);
    assert_eq!(
        list_commit_files(project, branch.series[0].clone().unwrap().patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone().unwrap().patches[1].id)?.len(),
        1
    );

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(descriptions, vec!["commit three", "commit one"]);
    Ok(())
}

#[test]
fn undo_commit_in_non_default_branch() -> anyhow::Result<()> {
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
    let _commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None).unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None).unwrap();

    // create default branch
    // this branch should not be affected by the undo
    let default_branch_id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..BranchCreateRequest::default()
        },
    )
    .unwrap();

    gitbutler_branch_actions::undo_commit(project, branch_id, commit2_id).unwrap();

    let mut branches = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter();

    let branch = &branches.find(|b| b.id == branch_id).unwrap();
    let default_branch = &branches.find(|b| b.id == default_branch_id).unwrap();

    // should be two uncommitted files now (file2.txt and file3.txt)
    assert_eq!(branch.files.len(), 2);
    assert_eq!(branch.series[0].clone().unwrap().patches.len(), 2);
    assert_eq!(
        list_commit_files(project, branch.series[0].clone().unwrap().patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone().unwrap().patches[1].id)?.len(),
        1
    );
    assert_eq!(default_branch.files.len(), 0);
    assert_eq!(default_branch.series[0].clone().unwrap().patches.len(), 0);

    let descriptions = branch.series[0]
        .clone()
        .unwrap()
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(descriptions, vec!["commit three", "commit one"]);
    Ok(())
}
