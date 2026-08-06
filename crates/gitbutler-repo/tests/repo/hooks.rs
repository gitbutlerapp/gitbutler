use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::support::RepoWithOrigin;
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use but_testsupport::open_repo;
use gitbutler_repo::hooks::{HookResult, pre_commit_with_tree, pre_push};

fn context_for_repo(workdir: &Path) -> Context {
    let project = gitbutler_project::Project::from_path(workdir).expect("valid test project");
    Context::new_from_legacy_project_and_settings_with_repo_open_mode(
        &project,
        AppSettings::default(),
        RepoOpenMode::Isolated,
    )
    .expect("can create context")
    .with_memory_app_cache()
}

#[test]
fn pre_commit_refuses_to_overwrite_stale_index_backup() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let workdir = test_project.local_repo.workdir().expect("non-bare");
    let ctx = context_for_repo(workdir);
    let index_path = ctx.gitdir.join("index");
    let backup_path = index_path.with_extension("gitbutler-hook-backup");
    let original_index = fs::read(&index_path)?;
    fs::write(&backup_path, b"stale backup")?;
    let tree_id = ctx.repo.get()?.head_tree_id_or_empty()?.detach();

    let err = pre_commit_with_tree(&ctx, tree_id).expect_err("stale backup must stop the hook");

    assert!(
        err.to_string().contains("stale pre-commit index backup"),
        "unexpected error: {err:#}"
    );
    assert_eq!(
        fs::read(&backup_path)?,
        b"stale backup",
        "stale backup must be preserved"
    );
    assert_eq!(
        fs::read(&index_path)?,
        original_index,
        "index must remain untouched"
    );
    Ok(())
}

#[test]
fn pre_commit_refuses_a_concurrent_index_swap() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let workdir = test_project.local_repo.workdir().expect("non-bare");
    let ctx = context_for_repo(workdir);
    let mut lock = but_core::sync::LockFile::open(
        ctx.gitdir
            .join("index")
            .with_extension("gitbutler-hook-lock"),
    )?;
    lock.lock()?;
    let tree_id = ctx.repo.get()?.head_tree_id_or_empty()?.detach();

    let err = pre_commit_with_tree(&ctx, tree_id).expect_err("concurrent swap must be rejected");

    assert!(
        err.to_string()
            .contains("another pre-commit hook is already using the repository index"),
        "unexpected error: {err:#}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn pre_commit_propagates_index_restore_failure() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let workdir = test_project.local_repo.workdir().expect("non-bare");
    let ctx = context_for_repo(workdir);
    let hook_path = ctx.gitdir.join("hooks/pre-commit");
    fs::write(
        &hook_path,
        "#!/bin/sh\nrm -f \"$(git rev-parse --git-path index).gitbutler-hook-backup\"\n",
    )?;
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    let tree_id = ctx.repo.get()?.head_tree_id_or_empty()?.detach();

    let err = pre_commit_with_tree(&ctx, tree_id).expect_err("restore failure must be returned");

    assert!(
        err.to_string()
            .contains("failed to restore pre-commit index"),
        "unexpected error: {err:#}"
    );
    Ok(())
}

#[test]
fn pre_push_hook_not_configured() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let repo = open_repo(test_project.local_repo.path())?;

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.object_hash().null(),
        &gitbutler_reference::RemoteRefname::new("origin", "does-not-matter"),
        true,
    );
    assert!(result.is_ok());
    assert_eq!(result?, HookResult::NotConfigured);
    Ok(())
}

#[test]
fn pre_push_hook_success() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();

    let repo = open_repo(test_project.local_repo.path())?;
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(&hook_path, "#!/bin/sh\ncat >hook.input\n")?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        true,
    )?;
    assert_eq!(result, HookResult::Success);

    let input = std::fs::read_to_string(repo.workdir().expect("non-bare").join("hook.input"))
        .expect("test-hook to pipe its output");
    let expected_pattern = "refs/heads/master ???????????????????????????????????????? refs/remotes/origin/master ????????????????????????????????????????\n";
    let is_required_format = gix::glob::wildmatch(
        expected_pattern.into(),
        input.as_str().into(),
        Default::default(),
    );
    assert!(is_required_format, "must match: {expected_pattern}");
    Ok(())
}

#[test]
fn pre_push_hook_failure() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();

    let repo = open_repo(test_project.local_repo.path())?;
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(
        &hook_path,
        "#!/bin/sh\nsleep 1\necho Hook failed with args: $@\nexit 1\n",
    )?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        true,
    );
    match result.expect("success") {
        HookResult::Failure(error_data) => {
            assert_eq!(
                error_data.error,
                "Hook failed with args: origin https://github.com/test/repo.git\n"
            );
        }
        _ => panic!("Expected hook failure"),
    }
    Ok(())
}

#[test]
fn pre_push_ignores_husky_core_hooks_path_when_disabled() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();

    let mut repo = open_repo(test_project.local_repo.path())?;
    let workdir = repo.workdir().expect("non-bare").to_path_buf();
    let hooks_dir = workdir.join(".husky").join("_");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(&hook_path, "#!/bin/sh\necho ran > husky-pre-push-ran\n")?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    repo.config_snapshot_mut()
        .set_raw_value("core.hooksPath", gix::path::into_bstr(&hooks_dir).as_ref())?;

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        false,
    )?;
    assert_eq!(result, HookResult::NotConfigured);
    assert!(!workdir.join("husky-pre-push-ran").exists());

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        true,
    )?;
    assert_eq!(result, HookResult::Success);
    assert!(workdir.join("husky-pre-push-ran").exists());
    Ok(())
}

#[test]
fn pre_push_resolves_relative_core_hooks_path_against_workdir() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();

    let mut repo = open_repo(test_project.local_repo.path())?;
    let workdir = repo.workdir().expect("non-bare").to_path_buf();
    let relative_hooks = format!(
        "relative-hooks-{}",
        workdir
            .file_name()
            .expect("temp dir name")
            .to_string_lossy()
    );
    let hooks_dir = workdir.join(&relative_hooks);
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(&hook_path, "#!/bin/sh\necho ran > relative-pre-push-ran\n")?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    repo.config_snapshot_mut()
        .set_raw_value("core.hooksPath", relative_hooks.as_str())?;

    let result = pre_push(
        &repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        true,
    )?;
    assert_eq!(result, HookResult::Success);
    assert!(workdir.join("relative-pre-push-ran").exists());
    Ok(())
}
