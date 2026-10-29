#[cfg(unix)]
#[test]
fn post_checkout_runs_without_git() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use but_testsupport::{gix_testtools, writable_scenario};

    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (repo, _tmp) = writable_scenario("single-unsigned");
    let workdir = repo.workdir().expect("fixture has a worktree");
    let hook = repo.common_dir().join("hooks/post-checkout");
    std::fs::create_dir_all(hook.parent().expect("hook has a directory"))?;
    std::fs::write(
        &hook,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" \"$GIT_INDEX_FILE\" > checkout-arguments\n",
    )?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    let _environment = gix_testtools::Env::new()
        .set("PATH", "")
        .set("GIT_INDEX_FILE", "unrelated-index");
    let old_head = repo.object_hash().null();
    let new_head = repo.head_id()?.detach();
    but_core::hooks::post_checkout(&repo, old_head, new_head)?;
    assert_eq!(
        std::fs::read_to_string(workdir.join("checkout-arguments"))?,
        format!(
            "{old_head}\n{new_head}\n1\n{}\n",
            repo.index_path().display()
        ),
        "post-checkout uses this repository's index and arguments without a Git executable"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn post_checkout_runs_all_hooks_and_preserves_their_failure_diagnostics() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let (mut repo, _tmp) = but_testsupport::writable_scenario("single-unsigned");
    let old_head = repo.object_hash().null();
    let new_head = repo.head_id()?.detach();
    but_core::hooks::post_checkout(&repo, old_head, new_head)?;

    let hook = repo.common_dir().join("hooks/post-checkout");
    std::fs::create_dir_all(hook.parent().expect("hook has a directory"))?;
    std::fs::write(&hook, "#!/bin/sh\nprintf 'traditional\\n' >> hook-events\n")?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("hook.first.event", "post-checkout")?;
        config.set_raw_value(
            "hook.first.command",
            "f () { printf 'first\\n' >> hook-events; printf stdout-diagnostic; printf stderr-diagnostic >&2; exit 2; }; f",
        )?;
        config.set_raw_value("hook.second.event", "post-checkout")?;
        config.set_raw_value(
            "hook.second.command",
            "f () { printf 'second\\n' >> hook-events; }; f",
        )?;
    }
    let error = but_core::hooks::post_checkout(&repo, old_head, new_head)
        .expect_err("a failed configured hook must be reported");
    let message = error.to_string();
    assert!(
        message.contains("exit status: 2")
            && message.contains("stdout-diagnostic")
            && message.contains("stderr-diagnostic"),
        "hook failure retains both output streams: {message}"
    );
    let events = repo
        .workdir()
        .expect("fixture has a worktree")
        .join("hook-events");
    assert_eq!(
        std::fs::read_to_string(&events)?,
        "first\nsecond\ntraditional\n",
        "failure does not prevent remaining configured or traditional hooks from running"
    );
    repo.config_snapshot_mut()
        .set_raw_value("hook.post-checkout.enabled", "false")?;
    std::fs::remove_file(&events)?;
    but_core::hooks::post_checkout(&repo, old_head, new_head)?;
    assert!(
        !events.exists(),
        "disabling the event suppresses every hook"
    );
    Ok(())
}
