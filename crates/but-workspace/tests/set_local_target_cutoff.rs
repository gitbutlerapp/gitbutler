use but_core::ref_metadata::{LocalRefName, ProjectMeta};

fn main_ref() -> anyhow::Result<LocalRefName> {
    LocalRefName::new("refs/heads/main".try_into()?)
}

#[test]
fn initial_target_uses_upstream_instead_of_checked_out_local_tip() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("set-local-target-cutoff");
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    let head = repo.head_id()?.detach();
    assert_ne!(head, upstream, "main must have local-only work");
    but_workspace::init::set_target_ref_and_init_project(&repo, &main_ref()?, None)?;
    assert_eq!(
        ProjectMeta::resolve(&repo)?.target_commit_id,
        Some(upstream),
        "initialization must leave local-only commits above the cutoff"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        head,
        "initialization must not move HEAD"
    );
    Ok(())
}

#[test]
fn resetting_same_target_preserves_cutoff_not_reachable_from_local() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    ProjectMeta {
        target_ref: Some(main_ref()?),
        target_commit_id: Some(upstream),
        ..Default::default()
    }
    .persist(&repo)?;
    but_workspace::init::set_target_ref_and_init_project(&repo, &main_ref()?, None)?;
    assert_eq!(
        ProjectMeta::resolve(&repo)?.target_commit_id,
        Some(upstream),
        "re-setting main must preserve the stored cutoff even when local main lags it"
    );
    Ok(())
}

#[test]
fn missing_cutoff_is_not_filled_from_local_by_migration_repair() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("set-local-target-cutoff");
    but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
        but_core::git_config::ensure_config_value(
            config,
            "gitbutler.project.targetRef",
            "refs/heads/main",
        )?;
        Ok(())
    })?;
    assert!(
        ProjectMeta::resolve(&repo)?.target_commit_id.is_none(),
        "the fixture must have a target but no stored cutoff"
    );
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    but_workspace::init::set_target_ref_and_init_project(&repo, &main_ref()?, None)?;
    assert_eq!(
        ProjectMeta::resolve(&repo)?.target_commit_id,
        Some(upstream),
        "migration repair must not override unset-cutoff precedence"
    );
    Ok(())
}

#[test]
fn missing_configured_upstream_rejects_initialization_without_metadata_changes()
-> anyhow::Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("set-local-target-cutoff");
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.merge", "refs/heads/missing")?;
    let before = ProjectMeta::resolve(&repo)?;
    let result = but_workspace::init::set_target_ref_and_init_project(&repo, &main_ref()?, None);
    assert!(
        result.is_err(),
        "an unavailable configured upstream must not become local main"
    );
    assert_eq!(
        ProjectMeta::resolve(&repo)?,
        before,
        "failed initialization must leave metadata unchanged"
    );
    Ok(())
}

#[test]
fn local_only_initialization_uses_local_tip() -> anyhow::Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("set-local-target-cutoff");
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.remote", "missing")?;
    let local = repo.rev_parse_single("main")?.detach();
    but_workspace::init::set_target_ref_and_init_project(&repo, &main_ref()?, None)?;
    assert_eq!(
        ProjectMeta::resolve(&repo)?.target_commit_id,
        Some(local),
        "without an upstream the local tip supplies the cutoff"
    );
    Ok(())
}
