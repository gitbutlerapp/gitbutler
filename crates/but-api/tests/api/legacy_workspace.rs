use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn head_info_discovers_worktrees_through_the_database() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

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

#[tokio::test]
async fn unreviewed_branch_push_needs_no_target_upstream() -> anyhow::Result<()> {
    crate::support::disable_askpass();
    let (repo, tmp) = crate::support::writable_scenario("checkout-head-info");
    let root = repo.workdir().expect("fixture has a workdir").to_owned();
    let remote_path = tmp.path().join("remote.git");
    git_at_dir(&root)
        .args(["init", "--bare"])
        .arg(&remote_path)
        .run();
    git_at_dir(&root)
        .args(["remote", "set-url", "origin"])
        .arg(&remote_path)
        .run();
    git_at_dir(&root)
        .args(["branch", "--unset-upstream", "main"])
        .run();
    git_at_dir(&root).args(["checkout", "feature"]).run();
    drop(repo);

    let repo = open_repo(&root)?;
    crate::support::persist_default_target(&repo)?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::legacy::workspace::workspace_branch_and_ancestors_push(
        ctx.to_sync(),
        false,
        false,
        "refs/heads/feature".into(),
        false,
        Vec::new(),
    )
    .await?;

    assert_eq!(
        open_repo(&remote_path)?
            .rev_parse_single("refs/heads/feature")?
            .detach(),
        ctx.repo.get()?.rev_parse_single("feature")?.detach(),
        "the push remote suffices when no review needs the target's name on the remote"
    );
    Ok(())
}

#[test]
fn create_virtual_branch_returns_the_stack_that_switching_back_created() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let canned_name = but_api::branch::branch_canned_name(&ctx)?;

    but_api::legacy::virtual_branches::set_base_branch(&mut ctx, "origin/main".into(), None)?;
    let stack = but_api::legacy::virtual_branches::create_virtual_branch(
        &mut ctx,
        gitbutler_branch::BranchCreateRequest {
            name: Some(canned_name.clone()),
            order: Some(0),
        },
    )?;

    assert_eq!(stack.heads[0].name, canned_name);
    // The stack created by setting the base branch is reused, not duplicated.
    snapbox::assert_data_eq!(
        crate::support::workspace_graph(&ctx)?,
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 33fbd87
└── ≡📙:amo-branch-1 on 33fbd87 {[..]}
    └── 📙:amo-branch-1
        └── ·b12810b (🏘️)

"#]]
    );
    Ok(())
}
