use but_oxidize::ObjectIdExt;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn undo_commit_simple() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let _commit1_id = super::create_commit(ctx, stack_entry.id, "commit one").unwrap();

    // create commit
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = super::create_commit(ctx, stack_entry.id, "commit two").unwrap();

    // create commit
    fs::write(repo.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id = super::create_commit(ctx, stack_entry.id, "commit three").unwrap();

    gitbutler_branch_actions::undo_commit(ctx, stack_entry.id, commit2_id).unwrap();

    // should be two uncommitted files now (file2.txt and file3.txt)
    let changes = but_core::diff::ui::worktree_changes(&*ctx.repo.get()?)?.changes;
    assert_eq!(changes.len(), 2);
    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].clone().commits.len(), 2);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[0].id.to_git2())?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[1].id.to_git2())?.len(),
        1
    );

    let messages = b.branch_details[0]
        .clone()
        .commits
        .iter()
        .map(|c| c.message.clone())
        .collect::<Vec<_>>();

    assert_eq!(messages, vec!["commit three", "commit one"]);
    Ok(())
}

#[test]
fn undo_commit_in_non_default_branch() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let _commit1_id = super::create_commit(ctx, stack_entry.id, "commit one").unwrap();

    // create commit
    fs::write(repo.path().join("file2.txt"), "content2").unwrap();
    fs::write(repo.path().join("file3.txt"), "content3").unwrap();
    let commit2_id = super::create_commit(ctx, stack_entry.id, "commit two").unwrap();

    // create commit
    fs::write(repo.path().join("file4.txt"), "content4").unwrap();
    let _commit3_id = super::create_commit(ctx, stack_entry.id, "commit three").unwrap();

    // create default branch
    // this branch should not be affected by the undo
    let mut guard = ctx.exclusive_worktree_access();
    let default_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            ..BranchCreateRequest::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    gitbutler_branch_actions::undo_commit(ctx, stack_entry.id, commit2_id).unwrap();

    // should be two uncommitted files now (file2.txt and file3.txt)
    let changes = but_core::diff::ui::worktree_changes(&*ctx.repo.get()?)?.changes;
    assert_eq!(changes.len(), 2);

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();

    assert_eq!(b.branch_details[0].clone().commits.len(), 2);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[0].id.to_git2())?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[1].id.to_git2())?.len(),
        1
    );

    let (_, default) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == default_stack_entry.id)
        .unwrap();

    assert_eq!(default.branch_details[0].clone().commits.len(), 0);

    let messages = b.branch_details[0]
        .clone()
        .commits
        .iter()
        .map(|c| c.message.clone())
        .collect::<Vec<_>>();

    assert_eq!(messages, vec!["commit three", "commit one"]);
    Ok(())
}
