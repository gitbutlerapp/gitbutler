use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn stash_in_the_workspace_commits_changes_and_unapplies_the_new_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let root = repo.workdir().expect("fixture has a workdir").to_owned();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::branch::apply_only(&mut ctx, "refs/heads/feature".try_into()?)?;
    crate::support::write_file(&root, "shared.txt", "stashed changes\n")?;

    let outcome = but_api::legacy::workspace::stash_into_branch(
        &mut ctx,
        "stash".into(),
        vec![but_core::DiffSpec {
            previous_path: None,
            path: "shared.txt".into(),
            hunk_headers: vec![],
        }],
    )?;

    let (_guard, repo, ws, _db) = ctx.workspace_and_db()?;
    assert_eq!(
        repo.rev_parse_single("refs/heads/stash")?.detach(),
        outcome.new_commit.expect("stash creates a commit"),
        "the unapplied branch retains the stashed commit"
    );
    assert!(
        ws.find_segment_and_stack_by_refname("refs/heads/stash".try_into()?)
            .is_none(),
        "the stash branch is no longer applied"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("shared.txt"))?,
        "main\n",
        "the stashed changes are removed from the worktree"
    );
    Ok(())
}

#[test]
fn stash_on_the_target_refuses_before_changing_the_checkout() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let root = repo.workdir().expect("fixture has a workdir").to_owned();
    crate::support::write_file(&root, "shared.txt", "keep these changes\n")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let outcome = but_api::legacy::workspace::stash_into_branch(
        &mut ctx,
        "stash".into(),
        vec![but_core::DiffSpec {
            previous_path: None,
            path: "shared.txt".into(),
            hunk_headers: vec![],
        }],
    );

    assert!(
        outcome.is_err(),
        "stash requires a managed workspace - making this work is a TODO"
    );
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.head_name()?,
        Some("refs/heads/main".try_into()?),
        "rejected stash must leave HEAD on the original branch"
    );
    assert!(
        repo.try_find_reference("refs/heads/stash")?.is_none(),
        "rejected stash must not leave a new branch behind"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("shared.txt"))?,
        "keep these changes\n",
        "rejected stash must preserve the uncommitted changes"
    );
    Ok(())
}

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
