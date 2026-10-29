use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::support::RepoWithOrigin;
use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use but_testsupport::open_repo;
use gitbutler_repo::hooks::{
    HookResult, MessageData, MessageHookResult, commit_msg, post_commit, pre_commit_with_tree,
    pre_push,
};

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

#[cfg(unix)]
#[test]
fn commit_hooks_use_common_hooks_in_linked_worktrees_without_git()
-> but_testsupport::gix_testtools::TestResult {
    use but_testsupport::gix_testtools;
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = RepoWithOrigin::default();
    let main = fixture
        .local_repo
        .workdir()
        .expect("fixture has a worktree");
    gix_testtools::git(main, "worktree add --detach ../linked")?;
    let linked = main.parent().expect("fixture has a parent").join("linked");
    let hooks = fixture.local_repo.common_dir().join("hooks");
    fs::create_dir_all(&hooks)?;
    for (name, script) in [
        (
            "commit-msg",
            "#!/bin/sh\nprintf 'rewritten message\\n' > \"$1\"\nprintf '%s' \"$GIT_DIR\" > hook-git-dir\n",
        ),
        ("post-commit", "#!/bin/sh\nprintf 'ran' > post-commit-ran\n"),
    ] {
        let path = hooks.join(name);
        fs::write(&path, script)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    let empty_path = tempfile::tempdir()?;
    let _environment =
        gix_testtools::Env::new().set("PATH", empty_path.path().display().to_string());
    for workdir in [main, linked.as_path()] {
        let ctx = context_for_repo(workdir);
        assert_eq!(
            commit_msg(&ctx, "original message".into())?,
            MessageHookResult::Message(MessageData {
                message: "rewritten message\n".into(),
            }),
            "both worktrees execute their common commit-message hook without Git"
        );
        assert_eq!(
            fs::read(workdir.join("hook-git-dir"))?,
            gix::path::into_bstr(ctx.repo.get()?.git_dir())?.as_ref(),
            "hooks receive their worktree's repository context"
        );
        assert_eq!(post_commit(&ctx)?, HookResult::Success);
        assert_eq!(
            fs::read(workdir.join("post-commit-ran"))?,
            b"ran",
            "post-commit runs in the caller's worktree"
        );
    }
    Ok(())
}

#[test]
fn commit_hooks_preserve_husky_fallback_and_explicit_directory_precedence() -> anyhow::Result<()> {
    let fixture = RepoWithOrigin::default();
    let workdir = fixture
        .local_repo
        .workdir()
        .expect("fixture has a worktree");
    let mut ctx = context_for_repo(workdir);
    let husky = workdir.join(".husky");
    fs::create_dir_all(&husky)?;
    let hook = husky.join("post-commit");
    fs::write(&hook, "#!/bin/sh\nprintf 'husky hook'\nexit 19\n")?;
    #[cfg(unix)]
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    assert_eq!(
        post_commit(&ctx)?,
        HookResult::NotConfigured,
        "the optional Husky directory is disabled by default"
    );
    ctx.legacy_project.husky_hooks_enabled = true;
    assert!(
        matches!(post_commit(&ctx)?, HookResult::Failure(error) if error.error == "husky hook"),
        "enabled Husky hooks run when the default hook is absent"
    );
    #[cfg(unix)]
    {
        let inactive = ctx.repo.get()?.hooks_directory()?.join("post-commit");
        fs::create_dir_all(inactive.parent().expect("hook has a directory"))?;
        fs::write(&inactive, "#!/bin/sh\nexit 0\n")?;
        fs::set_permissions(&inactive, fs::Permissions::from_mode(0o644))?;
        assert_eq!(
            post_commit(&ctx)?,
            HookResult::NotConfigured,
            "an inactive default hook does not fall through to Husky"
        );
        fs::remove_file(inactive)?;
    }
    ctx.repo
        .get_mut()?
        .config_snapshot_mut()
        .set_raw_value("core.hooksPath", "absent-hooks")?;
    assert_eq!(
        post_commit(&ctx)?,
        HookResult::NotConfigured,
        "an explicit hook directory suppresses fallback even when it is absent"
    );
    Ok(())
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

#[test]
fn pre_commit_keeps_the_index_modified_time() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let workdir = test_project.local_repo.workdir().expect("non-bare");
    let ctx = context_for_repo(workdir);
    let index_path = ctx.gitdir.join("index");
    let modified =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    fs::File::options()
        .write(true)
        .open(&index_path)?
        .set_modified(modified)?;
    let tree_id = ctx.repo.get()?.head_tree_id_or_empty()?.detach();

    pre_commit_with_tree(&ctx, tree_id)?;

    assert_eq!(
        fs::metadata(&index_path)?.modified()?,
        modified,
        "Git takes a same-size edit made in the same clock tick as its index entry for unchanged \
         once the index file is newer than the entry"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn pre_commit_restores_conflicts_or_absent_index_after_rejection()
-> but_testsupport::gix_testtools::TestResult {
    use but_testsupport::gix_testtools;
    use gix::index::entry::{Flags, Mode, Stage};
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    for had_index in [true, false] {
        let fixture = RepoWithOrigin::default();
        let repo = &fixture.local_repo;
        let workdir = repo.workdir().expect("fixture has a worktree");
        let index_path = repo.index_path();
        if had_index {
            let mut index = repo.open_index()?;
            for (stage, contents) in [
                (Stage::Base, "base"),
                (Stage::Ours, "ours"),
                (Stage::Theirs, "theirs"),
            ] {
                index.dangerously_push_entry(
                    Default::default(),
                    repo.write_blob(contents)?.detach(),
                    Flags::from_stage(stage),
                    Mode::FILE,
                    "conflicted".into(),
                );
            }
            index.sort_entries();
            index.write(Default::default())?;
            fs::set_permissions(&index_path, fs::Permissions::from_mode(0o600))?;
        } else {
            fs::remove_file(&index_path)?;
        }
        let original = fs::read(&index_path).ok();
        let tree_id = repo.head_tree_id()?.detach();
        let expected = repo.index_from_tree(&tree_id)?;
        let hook = repo.hooks_directory()?.join("pre-commit");
        fs::create_dir_all(hook.parent().expect("hook has a directory"))?;
        fs::write(
            &hook,
            "#!/bin/sh\n/bin/cp \"${GIT_INDEX_FILE:-.git/index}\" observed-index\nprintf 'rejected'\nexit 19\n",
        )?;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
        let ctx = context_for_repo(workdir);
        #[expect(
            deprecated,
            reason = "verify remaining legacy callers observe the restored index"
        )]
        let cached = ctx.git2_repo.get()?.index()?;
        let empty_path = tempfile::tempdir()?;
        let result = {
            let _environment =
                gix_testtools::Env::new().set("PATH", empty_path.path().display().to_string());
            pre_commit_with_tree(&ctx, tree_id)?
        };
        assert!(
            matches!(result, HookResult::Failure(error) if error.error == "rejected"),
            "a failed hook still restores the user's index"
        );
        assert_eq!(
            fs::read(&index_path).ok(),
            original,
            "the original bytes or absence of the index are preserved"
        );
        let observed = gix::index::File::at(
            workdir.join("observed-index"),
            repo.object_hash(),
            false,
            Default::default(),
        )?;
        let content = |index: &gix::index::State| {
            index
                .entries()
                .iter()
                .map(|entry| {
                    (
                        entry.path(index).to_owned(),
                        entry.id,
                        entry.mode,
                        entry.stage(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            content(&observed),
            content(&expected),
            "the hook sees exactly the requested tree"
        );
        assert_eq!(
            cached.has_conflicts(),
            had_index,
            "cached legacy handles see the original conflicts"
        );
        if had_index {
            assert_eq!(
                fs::metadata(&index_path)?.permissions().mode() & 0o777,
                0o600,
                "index permissions survive the swap"
            );
        } else {
            assert!(
                cached.is_empty(),
                "a previously absent index remains empty in legacy handles"
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn pre_commit_propagates_index_restore_failure() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();
    let workdir = test_project.local_repo.workdir().expect("non-bare");
    let ctx = context_for_repo(workdir);
    let hook_path = ctx.gitdir.join("hooks/pre-commit");
    fs::create_dir_all(hook_path.parent().expect("hook has a directory"))?;
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
fn pre_push_preserves_literal_hook_paths_and_arguments() -> anyhow::Result<()> {
    let fixture = RepoWithOrigin::default();
    let mut repo = fixture.local_repo;
    let workdir = repo.workdir().expect("fixture has a worktree");
    let hooks = workdir.join("hooks $@ with spaces");
    fs::create_dir_all(&hooks)?;
    let hook = hooks.join("pre-push");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" \"$2\" >&2\nexit 23\n",
    )?;
    #[cfg(unix)]
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    repo.config_snapshot_mut()
        .set_raw_value("core.hooksPath", "hooks $@ with spaces")?;
    let result = pre_push(
        &repo,
        "origin $@",
        "ssh://host/path with spaces",
        repo.head_id()?.detach(),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
        false,
    )?;
    assert!(
        matches!(result, HookResult::Failure(error) if error.error == "origin $@\nssh://host/path with spaces\n"),
        "configured paths and remote arguments are never interpreted as shell fragments"
    );
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
fn pre_push_hook_failure_with_stderr_only() -> anyhow::Result<()> {
    let test_project = RepoWithOrigin::default();

    let repo = open_repo(test_project.local_repo.path())?;
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(
        &hook_path,
        "#!/bin/sh\necho Run the tests before pushing >&2\nexit 1\n",
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
                error_data.error, "Run the tests before pushing\n",
                "a hook that only writes to stderr has its message shown"
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
        .set_raw_value("core.hooksPath", gix::path::into_bstr(&hooks_dir)?.as_ref())?;

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
