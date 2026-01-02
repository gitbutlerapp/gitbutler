use but_worktrees::{
    destroy::{worktree_destroy_by_id, worktree_destroy_by_reference},
    list::worktree_list,
    new::worktree_new,
};

use crate::util::test_ctx;

#[test]
fn can_destroy_worktree_by_id() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-and-parallel")?;
    let mut ctx = test_ctx.ctx;

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let outcome = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    // Verify it was created
    let list_before = worktree_list(&mut ctx, guard.read_permission())?;
    assert_eq!(list_before.entries.len(), 1);
    assert_eq!(list_before.entries[0].path, outcome.created.path);

    // Destroy it
    let destroy_outcome =
        worktree_destroy_by_id(&mut ctx, guard.write_permission(), &outcome.created.id)?;

    assert_eq!(destroy_outcome.destroyed_ids.len(), 1);
    assert_eq!(destroy_outcome.destroyed_ids[0], outcome.created.id);

    // Verify it was destroyed
    let list_after = worktree_list(&mut ctx, guard.read_permission())?;
    assert_eq!(list_after.entries.len(), 0);

    Ok(())
}

#[test]
fn can_destroy_worktrees_by_reference() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-and-parallel")?;
    let mut ctx = test_ctx.ctx;

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;

    // Create 3 worktrees from feature-a and 2 from feature-c
    worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
    worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
    worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
    worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;
    worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;

    // Verify all 5 were created
    let list_before = worktree_list(&mut ctx, guard.read_permission())?;
    assert_eq!(list_before.entries.len(), 5);

    // Destroy all feature-a worktrees
    let destroy_outcome =
        worktree_destroy_by_reference(&mut ctx, guard.write_permission(), feature_a_name.as_ref())?;

    assert_eq!(destroy_outcome.destroyed_ids.len(), 3);

    // Verify only feature-c worktrees remain
    let list_after = worktree_list(&mut ctx, guard.read_permission())?;
    assert_eq!(list_after.entries.len(), 2);
    assert!(
        list_after
            .entries
            .iter()
            .all(|e| e.created_from_ref.as_ref() == Some(&feature_c_name))
    );

    Ok(())
}

#[test]
fn destroy_by_reference_returns_empty_when_no_matches() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-and-parallel")?;
    let mut ctx = test_ctx.ctx;

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;

    // Create worktrees from feature-a
    worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    // Try to destroy worktrees from feature-b (which don't exist)
    let destroy_outcome =
        worktree_destroy_by_reference(&mut ctx, guard.write_permission(), feature_b_name.as_ref())?;

    assert_eq!(destroy_outcome.destroyed_ids.len(), 0);

    // Verify feature-a worktree is still there
    let list_after = worktree_list(&mut ctx, guard.read_permission())?;
    assert_eq!(list_after.entries.len(), 1);

    Ok(())
}
