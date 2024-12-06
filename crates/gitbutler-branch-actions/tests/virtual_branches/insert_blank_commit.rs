use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;

use super::*;

#[test]
fn insert_blank_commit_down() -> anyhow::Result<()> {
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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap();

    gitbutler_branch_actions::insert_blank_commit(project, branch_id, commit2_id, 1).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    assert_eq!(branch.series[0].clone()?.patches.len(), 4);

    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[1].id)?.len(),
        2
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[2].id)?.len(),
        0
    ); // blank commit

    let descriptions = branch.series[0]
        .clone()?
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        descriptions,
        vec!["commit three", "commit two", "", "commit one"]
    );
    Ok(())
}

#[test]
fn insert_blank_commit_up() -> anyhow::Result<()> {
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
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None, false)
            .unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None, false)
            .unwrap();

    gitbutler_branch_actions::insert_blank_commit(project, branch_id, commit2_id, -1).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    assert_eq!(branch.series[0].clone()?.patches.len(), 4);
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[1].id)?.len(),
        0
    ); // blank commit
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[2].id)?.len(),
        2
    );

    let descriptions = branch.series[0]
        .clone()?
        .patches
        .iter()
        .map(|c| c.description.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        descriptions,
        vec!["commit three", "", "commit two", "commit one"]
    );
    Ok(())
}
