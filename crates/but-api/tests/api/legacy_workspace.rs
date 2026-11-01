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

#[test]
fn create_virtual_branch_on_the_target_checks_out_the_new_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let stack = but_api::legacy::virtual_branches::create_virtual_branch(
        &mut ctx,
        gitbutler_branch::BranchCreateRequest {
            name: Some("new-branch".into()),
            order: None,
        },
    )?;

    assert_eq!(stack.heads[0].name, "new-branch");
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.head_name()?.map(|name| name.to_string()),
        Some("refs/heads/new-branch".into()),
        "alone on the target, the new branch is checked out as `git checkout -b` would"
    );
    assert!(
        repo.try_find_reference("refs/heads/gitbutler/workspace")?
            .is_none(),
        "a single branch doesn't need the workspace"
    );
    Ok(())
}

#[test]
fn create_virtual_branch_beside_another_branch_switches_to_the_workspace() -> anyhow::Result<()> {
    let (repo, tmp) = crate::support::repo_with_feature_branch()?;
    crate::support::persist_default_target(&repo)?;
    git_at_dir(tmp.path())
        .args(["checkout", "-b", "other"])
        .run();
    crate::support::write_file(tmp.path(), "other.txt", "other\n")?;
    git_at_dir(tmp.path()).args(["add", "other.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "other"]).run();
    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();

    but_api::legacy::virtual_branches::create_virtual_branch(
        &mut ctx,
        gitbutler_branch::BranchCreateRequest {
            name: Some("new-branch".into()),
            order: None,
        },
    )?;

    // The checked-out branch and the new one are both applied in the workspace.
    snapbox::assert_data_eq!(
        crate::support::workspace_graph(&ctx)?,
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on b12810b
├── ≡📙:other on b12810b {[..]}
│   └── 📙:other
│       └── ·8dfb293 (🏘️)
└── ≡📙:new-branch on b12810b {[..]}
    └── 📙:new-branch

"#]]
    );
    Ok(())
}
