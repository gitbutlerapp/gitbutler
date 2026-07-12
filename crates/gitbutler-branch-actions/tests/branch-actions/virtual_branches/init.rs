use but_core::git_config::{edit_config, remove_config_value, set_config_value};

use super::*;

#[test]
fn twice() {
    let data_dir = tempfile::tempdir().unwrap();

    let test_project = TestRepo::default();

    {
        let project = gitbutler_project::add_at_app_data_dir(data_dir.path(), test_project.path())
            .expect("failed to add project")
            .unwrap_project();
        let mut ctx = Context::new_from_legacy_project_and_settings_with_repo_open_mode(
            &project,
            AppSettings::default(),
            but_ctx::RepoOpenMode::Isolated,
        )
        .expect("can create context")
        .with_memory_app_cache();

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);
        let stacks = stack_details(&ctx);
        assert_eq!(stacks.len(), 0);
        gitbutler_project::delete_with_path(data_dir.path(), project.id).unwrap();
    }

    {
        let project = gitbutler_project::add_at_app_data_dir(data_dir.path(), test_project.path())
            .unwrap()
            .unwrap_project();
        let mut ctx = Context::new_from_legacy_project_and_settings_with_repo_open_mode(
            &project,
            AppSettings::default(),
            but_ctx::RepoOpenMode::Isolated,
        )
        .expect("can create context")
        .with_memory_app_cache();
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();

        // even though project is on gitbutler/workspace, we should not import it
        drop(guard);
        let stacks = stack_details(&ctx);
        assert_eq!(stacks.len(), 0);
    }
}

#[test]
fn dirty_non_target() {
    // a situation when you initialize project while being on the local version of the master
    // that has uncommitted changes.
    let Test { repo, ctx, .. } = &mut Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());

    fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
}

#[test]
fn dirty_target() {
    // a situation when you initialize project while being on the local version of the master
    // that has uncommitted changes.
    let Test { repo, ctx, .. } = &mut Test::default();

    fs::write(repo.path().join("file.txt"), "content").unwrap();

    let old = std::env::var("GIT_AUTHOR_NAME").ok();
    unsafe { std::env::set_var("GIT_AUTHOR_NAME", "GitButler") };
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    // Due to race conditions, this can either be "g-branch-1" or "a-branch-1".
    // This is a stop-gap measure since these tests are due to be nixed at some
    // point in the future.
    assert!(matches!(
        stacks[0].1.derived_name.as_ref(),
        "g-branch-1" | "a-branch-1"
    ));
    if let Some(old) = old {
        unsafe {
            std::env::set_var("GIT_AUTHOR_NAME", old);
        }
    }
}

#[test]
fn commit_on_non_target_local() {
    let Test { repo, ctx, .. } = &mut Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
}

#[test]
fn commit_on_non_target_remote() {
    let Test { repo, ctx, .. } = &mut Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");
    repo.simulate_push_branch(&"refs/heads/some-feature".parse().unwrap());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
    assert_eq!(stacks[0].1.branch_details[0].clone().commits.len(), 1);
}

#[test]
fn commit_on_target() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let old = std::env::var("GIT_AUTHOR_NAME").ok();
    unsafe {
        std::env::set_var("GIT_AUTHOR_NAME", "GitButler");
    }

    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    // Due to race conditions, this can either be "g-branch-1" or "a-branch-1".
    // This is a stop-gap measure since these tests are due to be nixed at some
    // point in the future.
    assert!(matches!(
        stacks[0].1.derived_name.as_ref(),
        "g-branch-1" | "a-branch-1"
    ));
    assert_eq!(stacks[0].1.branch_details[0].clone().commits.len(), 1);
    if let Some(old) = old {
        unsafe {
            std::env::set_var("GIT_AUTHOR_NAME", old);
        }
    }
}

#[test]
fn submodule() {
    let Test { ctx, .. } = &mut Test::from_fixture("scenario/repo-with-origin-and-submodule.sh");

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    drop(guard);
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
}

#[test]
fn bootstrap_missing_target_preserves_existing_workspace_ref() -> anyhow::Result<()> {
    let test = &mut Test::default();
    let Test {
        repo,
        project_id,
        ctx,
        ..
    } = test;

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content")?;
    repo.commit_all("commit on feature");

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )?;
    drop(guard);

    let repo = ctx.repo.get()?;
    let original_workspace_ref_target = repo
        .try_find_reference("refs/heads/gitbutler/workspace")?
        .expect("workspace ref should exist")
        .peel_to_id()?
        .detach();
    let expected_stack_name = stack_details(ctx)[0].1.derived_name.clone();

    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(
            config,
            but_project_handle::storage_path_config_key(),
            "gitbutler-alt",
        )?;
        remove_config_value(config, "gitbutler.project.targetRef")?;
        remove_config_value(config, "gitbutler.project.targetCommitId")?;
        remove_config_value(config, "gitbutler.project.pushRemote")?;
        Ok(())
    })?;
    drop(repo);

    let mut reopened: Context =
        but_testsupport::isolated_app_data_dir(|| project_id.clone().try_into())?;
    assert!(
        reopened.project_meta()?.target_ref.is_none(),
        "the freshly selected storage location has no project target"
    );

    let mut guard = reopened.exclusive_worktree_access();
    assert!(gitbutler_branch_actions::base::bootstrap_default_target_if_missing(&reopened)?);
    let meta = reopened.legacy_meta_mut(guard.write_permission())?;
    let repo = reopened.repo.get()?;
    meta.write_reconciled(&repo)?;
    drop(repo);
    drop(guard);

    let workspace_ref_target_after_activation = reopened
        .repo
        .get()?
        .try_find_reference("refs/heads/gitbutler/workspace")?
        .expect("workspace ref should still exist")
        .peel_to_id()?
        .detach();
    assert_eq!(
        workspace_ref_target_after_activation,
        original_workspace_ref_target
    );

    let stacks = stack_details(&reopened);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, expected_stack_name);
    Ok(())
}

/// Regression: the legacy target metadata is gone but Git config still carries the ported target
/// (as happens when the `virtual_branches.toml` / DB store is reset while `.git/config` survives).
/// Recovery must still run — keying the "already configured?" check off Git config used to skip it
/// and left target setup deadlocked. It must also restore the *surviving* target, not a re-inferred
/// default, or a custom target gets silently rewritten.
#[test]
fn bootstrap_missing_target_recovers_surviving_git_config_target() -> anyhow::Result<()> {
    let test = &mut Test::default();
    let Test {
        repo,
        project_id,
        ctx,
        ..
    } = test;

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content")?;
    repo.commit_all("commit on feature");
    // A second remote-tracking branch, so the surviving Git-config target can differ from the branch
    // inference would pick (origin/master).
    repo.simulate_push_branch(&"refs/heads/some-feature".parse().unwrap());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )?;
    drop(guard);

    // The base commit recorded in Git config; recovery must preserve it rather than re-peel the ref
    // tip (which would advance the workspace's frame of reference).
    let recorded_target_commit = ctx
        .project_meta()?
        .target_commit_id
        .expect("set_base_branch records a target commit");

    let repo = ctx.repo.get()?;
    let original_workspace_ref_target = repo
        .try_find_reference("refs/heads/gitbutler/workspace")?
        .expect("workspace ref should exist")
        .peel_to_id()?
        .detach();

    // Empty the legacy store (fresh storage location) while Git config keeps a custom target that
    // inference wouldn't pick. Only `targetRef` is rewritten; the recorded `targetCommitId` stays at
    // the base captured above — an ancestor of `some-feature`, i.e. a base that lags the ref tip as
    // it would after a fetch — so recovery must preserve it rather than re-peel `some-feature`'s tip.
    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(
            config,
            but_project_handle::storage_path_config_key(),
            "gitbutler-alt",
        )?;
        set_config_value(
            config,
            "gitbutler.project.targetRef",
            "refs/remotes/origin/some-feature",
        )?;
        Ok(())
    })?;
    drop(repo);

    let mut reopened: Context =
        but_testsupport::isolated_app_data_dir(|| project_id.clone().try_into())?;
    // Precondition: legacy store has no target, but Git config resolves the custom one.
    assert!(
        but_meta::legacy_storage::read_synced_virtual_branches(
            &reopened.project_data_dir().join("virtual_branches.toml")
        )?
        .default_target
        .is_none()
    );
    assert_eq!(
        reopened
            .project_meta()?
            .target_ref
            .map(|r| r.to_string())
            .as_deref(),
        Some("refs/remotes/origin/some-feature")
    );

    let mut guard = reopened.exclusive_worktree_access();
    assert!(
        gitbutler_branch_actions::base::bootstrap_default_target_if_missing(&reopened)?,
        "recovery must run even though Git config still has the target"
    );
    let meta = reopened.legacy_meta_mut(guard.write_permission())?;
    let repo = reopened.repo.get()?;
    meta.write_reconciled(&repo)?;
    drop(repo);
    drop(guard);

    // Healed from the surviving Git-config target, not re-inferred to origin/master.
    let restored = but_meta::legacy_storage::read_synced_virtual_branches(
        &reopened.project_data_dir().join("virtual_branches.toml"),
    )?
    .default_target
    .expect("default target restored");
    assert_eq!(restored.branch.remote(), "origin");
    assert_eq!(restored.branch.branch(), "some-feature");
    // Preserved the surviving base commit rather than re-peeling the ref tip.
    assert_eq!(restored.sha, recorded_target_commit);

    let workspace_ref_target_after_activation = reopened
        .repo
        .get()?
        .try_find_reference("refs/heads/gitbutler/workspace")?
        .expect("workspace ref should still exist")
        .peel_to_id()?
        .detach();
    assert_eq!(
        workspace_ref_target_after_activation,
        original_workspace_ref_target
    );
    Ok(())
}
