use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_stack::BranchOwnershipClaims;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn move_file_down() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();
    let commit1 = repo.find_commit(commit1_id).unwrap();

    // create commit
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();
    let commit2 = repo.find_commit(commit2_id).unwrap();

    // amend another hunk
    let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    gitbutler_branch_actions::move_commit_file(
        ctx,
        stack_entry.id,
        commit2_id,
        commit1_id,
        &to_amend,
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();

    assert_ne!(&commit1.id(), &b.branch_details[0].commits[1].id.to_git2());
    assert_ne!(&commit2.id(), &b.branch_details[0].commits[0].id.to_git2());

    assert_eq!(b.branch_details[0].commits.len(), 2);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].commits[1].id.to_git2())?.len(),
        2
    ); // this now has both file changes
    Ok(())
}

#[test]
fn move_file_up() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    let commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    // create commit
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();

    // amend another hunk
    let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
    gitbutler_branch_actions::move_commit_file(
        ctx,
        stack_entry.id,
        commit1_id,
        commit2_id,
        &to_amend,
    )
    .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();

    assert_eq!(b.branch_details[0].commits.len(), 2);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?.len(),
        2
    ); // this now has both file changes
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].commits[1].id.to_git2())?.len(),
        1
    ); // this now has both file changes
    Ok(())
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
    let _commit1_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None)

        .unwrap();

    // create middle commit one
    fs::write(repository.path().join("file2.txt"), "content2\ncontent2a\n").unwrap();
    fs::write(repository.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None)

        .unwrap();

    // create middle commit two
    fs::write(
        repository.path().join("file2.txt"),
        "content2\ncontent2a\ncontent2b\ncontent2c\ncontent2d",
    )
    .unwrap();
    fs::write(repository.path().join("file4.txt"), "content4").unwrap();
    let commit3_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None)

        .unwrap();

    // create top commit
    fs::write(repository.path().join("file5.txt"), "content5").unwrap();
    let _commit4_id = gitbutler_branch_actions::create_commit(project, branch_id, "commit four", None)

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
