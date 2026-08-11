use gitbutler_oplog::{OplogExt as _, RestoreKind};

#[test]
fn apply_only_threads_returned_workspace_back_into_context_cache() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let sibling = gix::refs::FullName::try_from("refs/heads/sibling")?;

    let first = but_api::branch::apply_only(&mut ctx, feature.as_ref())?;
    assert!(
        first.applied_branches.contains(&feature),
        "first apply should persist the requested feature branch: {:?}",
        first.applied_branches
    );

    let second = but_api::branch::apply_only(&mut ctx, sibling.as_ref())?;
    assert!(
        second.applied_branches.contains(&sibling),
        "second apply should use the cached workspace updated by the first apply: {:?}",
        second.applied_branches
    );

    let workspace = crate::support::workspace_graph(&ctx)?;
    assert!(
        workspace.contains("feature"),
        "cached workspace should still contain the first applied branch after the second apply:\n{workspace}"
    );
    assert!(
        workspace.contains("sibling"),
        "cached workspace should contain the second applied branch:\n{workspace}"
    );

    Ok(())
}

#[test]
fn undo_apply_from_ad_hoc_checkout_restores_checkout_and_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let sibling = gix::refs::FullName::try_from("refs/heads/sibling")?;

    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;
    let workspace_before = ctx
        .repo
        .get()?
        .find_reference(but_core::WORKSPACE_REF_NAME)?
        .peel_to_id()?
        .detach();
    but_api::branch::branch_checkout(&mut ctx, sibling.clone())?;
    let checkout_before = ctx.repo.get()?.head_id()?.detach();

    let outcome = but_api::branch::apply(&mut ctx, feature.as_ref())?;
    assert!(
        outcome.status.persisted_mutation(),
        "applying from the ad-hoc checkout must mutate the managed workspace"
    );
    assert_eq!(
        ctx.repo
            .get()?
            .head_name()?
            .expect("apply leaves HEAD symbolic")
            .as_bstr(),
        but_core::WORKSPACE_REF_NAME,
        "applying from an ad-hoc checkout must enter the managed workspace"
    );

    let snapshot = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .next()
        .expect("a successful apply records its pre-apply snapshot")?;
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        snapshot.commit_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    drop(guard);

    assert_eq!(
        ctx.repo
            .get()?
            .head_name()?
            .expect("restored HEAD is symbolic"),
        sibling,
        "undoing the apply must return to the original ad-hoc branch"
    );
    assert_eq!(
        ctx.repo.get()?.head_id()?.detach(),
        checkout_before,
        "undoing the apply must restore the original ad-hoc commit"
    );
    assert_eq!(
        ctx.repo
            .get()?
            .find_reference(but_core::WORKSPACE_REF_NAME)?
            .peel_to_id()?
            .detach(),
        workspace_before,
        "undoing the apply must restore the prior managed workspace commit"
    );

    Ok(())
}
