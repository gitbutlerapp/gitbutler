use but_core::git_config::{set_config_value, write_config};
use but_testsupport::legacy::stack_details;

use super::*;

#[test]
fn twice() {
    let data_dir = paths::data_dir();

    let test_project = TestProject::default();

    {
        let project = gitbutler_project::add_at_app_data_dir(data_dir.path(), test_project.path())
            .expect("failed to add project")
            .unwrap_project();
        let mut ctx = Context::new_from_legacy_project_and_settings_with_repo_open_mode(
            &project,
            AppSettings::default(),
            but_ctx::RepoOpenMode::Isolated,
        )
        .expect("can create context");

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
        .expect("can create context");
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
    repo.push_branch(&"refs/heads/some-feature".parse().unwrap());

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
    let Test { repo, ctx, .. } = &mut Test::default();

    let test_project = TestProject::default();
    let submodule_url: gitbutler_url::Url =
        test_project.path().display().to_string().parse().unwrap();
    repo.add_submodule(&submodule_url, path::Path::new("submodule"));

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

    let config_path = repo.git_dir().join("config");
    let mut config =
        gix::config::File::from_path_no_includes(config_path.clone(), gix::config::Source::Local)?;
    set_config_value(
        &mut config,
        but_project_handle::storage_path_config_key(),
        "gitbutler-alt",
    )?;
    write_config(&config_path, &config)?;
    drop(repo);

    let mut reopened: Context = project_id.clone().try_into()?;
    assert!(
        gitbutler_stack::VirtualBranchesHandle::new(reopened.project_data_dir())
            .maybe_get_default_target()?
            .is_none()
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
