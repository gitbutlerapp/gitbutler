#[test]
fn head_info_discovers_worktrees_through_the_database() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;

    assert!(
        !ctx.db.get_cache()?.worktree_meta().adoption_ran()?,
        "worktree adoption has not run before the workspace graph is built"
    );
    but_api::legacy::workspace::head_info(&ctx)?;
    assert!(
        ctx.db.get_cache()?.worktree_meta().adoption_ran()?,
        "head-info construction passes the feature-gated database handle into graph discovery"
    );
    Ok(())
}

#[test]
fn head_info_snapshot_preserves_the_shared_head_info_contract() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let direct: but_workspace::ui::RefInfo =
        but_api::legacy::workspace::head_info(&ctx)?.try_into()?;
    let snapshot: but_workspace::ui::RefInfo =
        but_api::legacy::workspace::head_info_snapshot(&ctx)?
            .head_info
            .try_into()?;

    assert_eq!(
        serde_json::to_value(direct)?,
        serde_json::to_value(snapshot)?,
        "the Lite snapshot endpoint wraps the same projection the shared endpoint returns"
    );
    Ok(())
}

#[test]
fn workspace_revision_tracks_projection_inputs_not_worktree_files() -> anyhow::Result<()> {
    use but_core::RefMetadata;
    use but_testsupport::{CommandExt, git_at_dir};

    let (repo, tmp) = crate::support::repo_with_feature_branch()?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let initial = but_api::workspace_revision::compute(&ctx)?;

    let metadata_path = ctx.project_data_dir().join("virtual_branches.toml");
    let mut metadata = std::fs::read(&metadata_path).unwrap_or_default();
    metadata.extend_from_slice(b"\n# formatting-only change\n");
    std::fs::write(metadata_path, metadata)?;
    assert_eq!(
        but_api::workspace_revision::compute(&ctx)?,
        initial,
        "storage formatting is not a semantic workspace input"
    );

    crate::support::write_file(tmp.path(), "untracked.txt", "not projected\n")?;
    assert_eq!(
        but_api::workspace_revision::compute(&ctx)?,
        initial,
        "untracked worktree contents are outside the workspace projection"
    );

    ctx.meta()?.set_branch_stack_order(&[
        "refs/heads/main".try_into()?,
        "refs/heads/feature".try_into()?,
    ])?;
    let reordered = but_api::workspace_revision::compute(&ctx)?;
    assert_ne!(
        reordered, initial,
        "persisted branch ordering changes the workspace projection inputs"
    );

    git_at_dir(tmp.path()).args(["branch", "other"]).run();
    assert_ne!(
        but_api::workspace_revision::compute(&ctx)?,
        reordered,
        "a new local ref changes the workspace projection inputs"
    );
    Ok(())
}
