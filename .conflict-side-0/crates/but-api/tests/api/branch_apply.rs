#[test]
fn apply_only_threads_returned_workspace_back_into_context_cache() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;

    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
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
