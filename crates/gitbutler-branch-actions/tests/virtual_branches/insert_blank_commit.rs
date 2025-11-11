use but_oxidize::ObjectIdExt;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn insert_blank_commit_down() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
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
    let _commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    // create commit
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();

    // create commit
    fs::write(repo.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap();

    gitbutler_branch_actions::insert_blank_commit(ctx, stack_entry.id, commit2_id, 1, None)
        .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].clone().commits.len(), 4);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[0].id.to_git2())?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[1].id.to_git2())?.len(),
        2
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[2].id.to_git2())?.len(),
        0
    );
    let messages = b.branch_details[0]
        .clone()
        .commits
        .iter()
        .map(|c| c.message.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["commit three", "commit two", "", "commit one"]
    );
    Ok(())
}

#[test]
fn insert_blank_commit_up() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
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
    let _commit1_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    // create commit
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit two", None).unwrap();

    // create commit
    fs::write(repo.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit three", None).unwrap();

    gitbutler_branch_actions::insert_blank_commit(ctx, stack_entry.id, commit2_id, -1, None)
        .unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].clone().commits.len(), 4);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[0].id.to_git2())?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[1].id.to_git2())?.len(),
        0
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[2].id.to_git2())?.len(),
        2
    );
    let messages = b.branch_details[0]
        .clone()
        .commits
        .iter()
        .map(|c| c.message.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["commit three", "", "commit two", "commit one"]
    );
    Ok(())
}
