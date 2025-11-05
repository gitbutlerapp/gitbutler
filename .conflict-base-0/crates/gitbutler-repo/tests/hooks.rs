use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use gitbutler_repo::hooks::{HookResult, pre_push};
use gitbutler_testsupport::TestProject;

#[test]
fn pre_push_hook_not_configured() -> anyhow::Result<()> {
    let test_project = TestProject::default();

    let result = pre_push(
        &test_project.local_repo,
        "origin",
        "https://github.com/test/repo.git",
        git2::Oid::zero(),
        &gitbutler_reference::RemoteRefname::new("origin", "does-not-matter"),
    );
    assert!(result.is_ok());
    assert_eq!(result?, HookResult::NotConfigured);
    Ok(())
}

#[test]
fn pre_push_hook_success() -> anyhow::Result<()> {
    let test_project = TestProject::default();

    let repo = &test_project.local_repo;
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(&hook_path, "#!/bin/sh\ncat >hook.input\n")?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    let result = pre_push(
        repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head()?.target().expect("not detached"),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
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

    let repo = &test_project.local_repo;
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-push");

    fs::write(
        &hook_path,
        "#!/bin/sh\necho Hook failed with args: $@\nexit 1\n",
    )?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    let result = pre_push(
        repo,
        "origin",
        "https://github.com/test/repo.git",
        repo.head()?.target().expect("not detached"),
        &gitbutler_reference::RemoteRefname::new("origin", "master"),
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
