use but_core::RefMetadata;

use crate::support::{
    assert_workspace_ref, checkout_branch_in_linked_worktree, create_empty_branch_above,
    repo_with_feature_branch, set_project_target_to_feature,
};

#[test]
fn branch_remove_rejects_non_local_reference() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let remote = gix::refs::FullName::try_from("refs/remotes/origin/main")?;

    let err = but_api::branch::branch_remove(&mut ctx, remote.clone())
        .expect_err("remote-tracking references cannot be deleted as local branches");
    assert!(
        err.to_string().contains("local branches"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(remote.as_ref())?.is_some(),
        "the rejected remote-tracking reference remains"
    );

    Ok(())
}

#[test]
fn branch_remove_rejects_gitbutler_technical_reference() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let technical = gix::refs::FullName::try_from("refs/heads/gitbutler/target")?;
    let head_id = repo.head_id()?.detach();
    repo.reference(
        technical.as_ref(),
        head_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "create technical branch for test",
    )?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::branch::branch_remove(&mut ctx, technical.clone())
        .expect_err("GitButler technical references cannot be deleted as user branches");
    assert!(
        err.to_string().contains("technical branch"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(technical.as_ref())?.is_some(),
        "the rejected technical reference remains"
    );

    Ok(())
}

#[test]
fn branch_remove_deletes_standalone_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;

    but_api::branch::branch_remove(&mut ctx, feature.clone())?;

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(feature.as_ref())?.is_none(),
        "the standalone branch reference was removed"
    );

    Ok(())
}

#[test]
fn branch_remove_deletes_middle_empty_branch_and_keeps_head() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // Stack two empty branches above the checked-out `main`: [tip, middle, main].
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;

    but_api::branch::branch_remove(&mut ctx, middle.clone())?;

    let repo = ctx.repo.get()?;
    // Removing a branch that isn't checked out leaves HEAD on the tip.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);
    assert!(repo.try_find_reference(middle.as_ref())?.is_none());
    // The order relinks the tip straight onto the base.
    let order = ctx
        .meta()?
        .branch_stack_order(tip.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(order, vec![tip, main]);

    Ok(())
}

#[test]
fn branch_remove_checked_out_empty_tip_moves_head_to_ref_below() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // [tip, middle, main] with HEAD on the empty `tip`.
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;

    let result = but_api::branch::branch_remove(&mut ctx, tip.clone())?;

    let repo = ctx.repo.get()?;
    // HEAD lands on the reference that was directly underneath the removed tip.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), middle);
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());
    assert_workspace_ref(&result.workspace, "refs/heads/middle");

    let order = ctx
        .meta()?
        .branch_stack_order(middle.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(order, vec![middle, main]);

    Ok(())
}

#[test]
fn branch_remove_rejects_checked_out_branch_with_commits() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;

    let err = but_api::branch::branch_remove(&mut ctx, main.clone())
        .expect_err("cannot delete the checked-out branch that owns commits");
    assert!(
        err.to_string().contains("contains commits"),
        "unexpected error: {err}"
    );

    // Nothing was removed.
    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(main.as_ref())?.is_some());
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), main);

    Ok(())
}

#[test]
fn branch_remove_refuses_when_checked_out_in_another_worktree() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // Keep `middle` in the primary worktree's projection while checking it out in a linked
    // worktree: [tip, middle, main], with the primary worktree on `tip`.
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;
    let _worktree = checkout_branch_in_linked_worktree(tmp.path(), "middle")?;

    let err = but_api::branch::branch_remove(&mut ctx, middle.clone())
        .expect_err("cannot remove a branch checked out in another worktree");
    assert!(
        err.to_string().contains("checked out"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(middle.as_ref())?.is_some(),
        "the checked-out branch must remain"
    );
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);

    Ok(())
}
