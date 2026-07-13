use std::fs;

use but_core::ref_metadata::ProjectMeta;
use but_ctx::{Context, ProjectHandle};
use but_path::AppChannel;
use but_testsupport::{CommandExt as _, git, gix_testtools::tempfile::TempDir, open_repo};

#[test]
fn new_from_project_handle_uses_repo_gitdir() -> anyhow::Result<()> {
    but_testsupport::isolated_app_data_dir(|| {
        let repo = but_testsupport::read_only_in_memory_scenario("unborn-empty")?;
        let worktree = repo.workdir().expect("fixture is non-bare").to_owned();

        assert!(repo.path().is_relative());
        for input in [
            repo.git_dir().to_owned(),
            repo.workdir().expect("non-bare").to_owned(),
        ] {
            let handle = ProjectHandle::from_path(&input)?;
            let ctx = Context::new_from_project_handle(handle)?;

            let expected_gitdir = gix::path::realpath(ctx.repo.get()?.path())?;
            let expected_worktree = gix::path::realpath(&worktree)?;
            assert_eq!(
                ctx.gitdir, expected_gitdir,
                "the Git dir is the realpath, so ProjectHandles can be worktrees or git directories"
            );
            assert_ne!(ctx.gitdir, repo.path(), "even though we didn't pass it");
            assert_eq!(
                ctx.workdir()?.as_deref(),
                Some(expected_worktree.as_path()),
                "real-pathiness translates to the worktree"
            );
        }

        let ctx = Context::from_repo_for_testing(repo.clone())?;
        assert_eq!(
            ctx.gitdir,
            repo.path(),
            "When creating a context from a repo directly, it will not alter the stored path though."
        );
        Ok(())
    })
}

#[test]
fn new_from_project_handle_keeps_repo_cached() -> anyhow::Result<()> {
    but_testsupport::isolated_app_data_dir(|| {
        let repo = but_testsupport::read_only_in_memory_scenario("unborn-empty")?;
        let handle = ProjectHandle::from_path(repo.git_dir())?;
        let ctx = Context::new_from_project_handle(handle)?;

        assert!(
            ctx.repo.get_opt().is_some(),
            "the repository used during construction should be kept in context"
        );
        assert!(ctx.to_sync().repo.is_some());
        Ok(())
    })
}

#[test]
fn project_data_dir_comes_from_git_config() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    let repo = gix::init(repo_dir.path())?;
    let key = but_project_handle::storage_path_config_key().to_owned();
    git(&repo)
        .args(["config", "--local", key.as_str(), "gitbutler-custom"])
        .run();
    let repo = open_repo(repo_dir.path())?;

    let ctx = Context::from_repo_for_testing(repo)?;
    assert_eq!(ctx.project_data_dir(), ctx.gitdir.join("gitbutler-custom"));

    let db = ctx.db.get_cache()?;
    assert!(
        ctx.project_data_dir().join("but.sqlite").exists(),
        "database should be created in configured project-data directory"
    );

    let project_cache_path = ctx.project_data_dir().join("but_cache.sqlite");
    assert!(
        !project_cache_path.exists(),
        "cache database isn't present initially"
    );

    let _cache = db.cache.get()?;
    assert!(
        project_cache_path.exists(),
        "cache database should be created after first access alongside the main database in configured project-data directory"
    );
    Ok(())
}

#[test]
fn sync_context_preserves_project_data_dir() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    gix::init(repo_dir.path())?;
    let repo = open_repo(repo_dir.path())?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let sync = ctx.to_sync();
    let restored = sync.into_thread_local();
    assert_eq!(ctx.project_data_dir(), restored.project_data_dir());
    Ok(())
}

#[test]
fn discover_with_app_channel_uses_requested_project_data_dir() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    let repo = gix::init(repo_dir.path())?;
    let nightly_key =
        but_project_handle::storage_path_config_key_for_app_channel(AppChannel::Nightly);
    let dev_key = but_project_handle::storage_path_config_key_for_app_channel(AppChannel::Dev);
    git(&repo)
        .args(["config", "--local", nightly_key, "gitbutler-nightly"])
        .run();
    git(&repo)
        .args(["config", "--local", dev_key, "gitbutler-dev"])
        .run();

    but_testsupport::isolated_app_data_dir(|| {
        let nightly_ctx = Context::discover_with_app_channel(repo_dir.path(), AppChannel::Nightly)?;
        assert_eq!(
            nightly_ctx.project_data_dir(),
            nightly_ctx.gitdir.join("gitbutler-nightly")
        );

        let dev_ctx = Context::discover_with_app_channel(repo_dir.path(), AppChannel::Dev)?;
        assert_eq!(
            dev_ctx.project_data_dir(),
            dev_ctx.gitdir.join("gitbutler-dev")
        );
        Ok(())
    })
}

#[test]
fn set_project_meta_persists_git_config() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    let project_meta = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    assert_eq!(ctx.project_meta()?, ProjectMeta::default());

    ctx.set_project_meta(project_meta.clone())?;
    assert_eq!(ctx.project_meta()?, project_meta);

    let changed = ProjectMeta {
        push_remote: Some("another-fork".into()),
        ..project_meta
    };
    ctx.set_project_meta(changed.clone())?;
    assert_eq!(ctx.project_meta()?, changed);
    Ok(())
}

#[test]
fn set_project_meta_fills_missing_target_commit_id_from_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let expected_target_id = {
        let mut target_ref = repo.find_reference("refs/remotes/origin/main")?;
        target_ref.peel_to_commit()?.id
    };
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(expected_target_id),
        "migration should fill a missing target commit from the target ref tip"
    );
    Ok(())
}

#[test]
fn set_project_meta_preserves_existing_target_commit_id() -> anyhow::Result<()> {
    let (_tmp, repo, target_ref_tip) = run_fixture("project-meta-base")?;
    let stable_target = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")?;
    assert_ne!(
        stable_target, target_ref_tip,
        "the fixture must detect repair"
    );
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(stable_target),
        push_remote: None,
    })?;

    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(stable_target),
        "an existing stable target must not move to the current ref tip"
    );
    Ok(())
}

#[test]
fn set_project_meta_clears_missing_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/missing".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    assert_eq!(ctx.project_meta()?.target_ref, None);
    Ok(())
}

#[test]
fn project_meta_defaults_when_config_and_toml_are_unset() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let actual = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(actual),
        snapbox::str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>"]
    );
    Ok(())
}

#[test]
fn project_meta_observes_changes_made_through_other_repository_handles() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    assert_eq!(ctx.project_meta()?.target_ref, None);

    // Write through an independent handle, like another process would.
    let other_ctx = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    other_ctx.set_project_meta(project_meta(
        target_commit_id,
        "refs/remotes/origin/main",
        "fork",
    )?)?;

    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/main".to_string()),
        "a long-lived context observes target changes made elsewhere"
    );
    Ok(())
}

#[test]
fn context_creation_ports_legacy_toml_before_cleanup() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-toml")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    let expected = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    assert_eq!(ctx.project_meta()?, expected);

    fs::write(
        ctx.project_data_dir().join("virtual_branches.toml"),
        "[branches]\n",
    )?;
    let reopened = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    assert_eq!(reopened.project_meta()?, expected);
    Ok(())
}

#[test]
fn context_creation_preserves_unmarked_project_config() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
        but_core::git_config::remove_config_value(config, "gitbutler.project.portedMeta")
    })?;

    let ctx = Context::from_repo_for_testing(repo)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=[OID]; push_remote=origin"
        ]
    );
    Ok(())
}

#[test]
fn project_meta_reads_git_config_and_ignores_stale_toml() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let actual = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(actual),
        snapbox::str![
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=[OID]; push_remote=origin"
        ]
    );
    Ok(())
}

fn run_fixture(name: &str) -> anyhow::Result<(TempDir, gix::Repository, gix::ObjectId)> {
    let (repo, repo_dir) = but_testsupport::writable_scenario(name);
    let target_commit_id = repo.rev_parse_single("HEAD")?.detach();
    Ok((repo_dir, repo, target_commit_id))
}

fn project_meta(
    target_commit_id: gix::ObjectId,
    target_ref: &str,
    push_remote: &str,
) -> anyhow::Result<ProjectMeta> {
    Ok(ProjectMeta {
        target_ref: Some(target_ref.try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some(push_remote.to_owned()),
    })
}

fn project_meta_summary(project_meta: ProjectMeta) -> String {
    format!(
        "target_ref={}; target_commit_id={}; push_remote={}",
        project_meta
            .target_ref
            .as_ref()
            .map_or("<unset>".into(), ToString::to_string),
        project_meta.target_commit_id.map_or("<unset>", |_| "[OID]"),
        project_meta.push_remote.as_deref().unwrap_or("<unset>")
    )
}
