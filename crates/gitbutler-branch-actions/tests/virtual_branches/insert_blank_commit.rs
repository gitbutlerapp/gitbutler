use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;

use super::*;

#[test]
fn insert_blank_commit_down() -> anyhow::Result<()> {
    let Test {
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap();

    gitbutler_branch_actions::insert_blank_commit(ctx, stack_entry.id, commit2_id, 1).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();

    assert_eq!(branch.series[0].clone()?.patches.len(), 4);

    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[1].id)?.len(),
        2
    );
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[2].id)?.len(),
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
        repository, ctx, ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let _commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    // create commit
    fs::write(repository.path().join("file2.txt"), "content2").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();

    // create commit
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap();

    gitbutler_branch_actions::insert_blank_commit(ctx, stack_entry.id, commit2_id, -1).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();

    assert_eq!(branch.series[0].clone()?.patches.len(), 4);
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[1].id)?.len(),
        0
    ); // blank commit
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[2].id)?.len(),
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
