use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use but_testsupport::{legacy::TestProject, open_repo};
use gitbutler_repo::hooks::{HookResult, pre_push};

#[test]
fn pre_push_hook_not_configured() -> anyhow::Result<()> {
    let test_project = TestProject::default();
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
    let test_project = TestProject::default();

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
    let test_project = TestProject::default();

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
    let test_project = TestProject::default();

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
    let test_project = TestProject::default();

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
