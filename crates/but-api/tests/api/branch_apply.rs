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
fn stacked_apply_uses_one_api_operation_and_updates_the_cached_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_conflicting_branches()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch_a = gix::refs::FullName::try_from("refs/heads/A")?;
    let branch_b = gix::refs::FullName::try_from("refs/heads/B")?;

    let first = but_api::branch::apply(&mut ctx, branch_a.as_ref())?;
    assert_eq!(
        first.status,
        but_workspace::branch::apply::OutcomeStatus::Applied
    );
    let snapshots_after_first = snapshot_count(&ctx)?;

    let failed = but_api::branch::apply(&mut ctx, branch_b.as_ref())?;
    assert_eq!(
        failed.status,
        but_workspace::branch::apply::OutcomeStatus::ConflictAborted
    );
    assert_eq!(
        snapshot_count(&ctx)?,
        snapshots_after_first,
        "the aborted independent attempt should not create an oplog entry"
    );

    let stacked = but_api::branch::apply_stacked(&mut ctx, branch_b.as_ref(), branch_a.as_ref())?;
    assert_eq!(
        stacked.status,
        but_workspace::branch::apply::OutcomeStatus::Applied
    );
    assert_eq!(
        snapshot_count(&ctx)?,
        snapshots_after_first + 1,
        "the stacked retry should create exactly one oplog entry"
    );
    let workspace = crate::support::workspace_graph(&ctx)?;
    assert!(
        workspace.contains("A"),
        "destination is still cached:\n{workspace}"
    );
    assert!(
        workspace.contains("B"),
        "incoming branch is now cached:\n{workspace}"
    );

    Ok(())
}

fn snapshot_count(ctx: &but_ctx::Context) -> anyhow::Result<usize> {
    Ok(but_api::legacy::oplog::snapshots_iter(ctx, None, None, None)?.count())
}
