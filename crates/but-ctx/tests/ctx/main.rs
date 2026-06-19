use but_core::{RefMetadata as _, WORKSPACE_REF_NAME, ref_metadata::ProjectMeta};
use but_ctx::{Context, ProjectHandle};
use but_meta::VirtualBranchesTomlMetadata;
use but_path::AppChannel;
use but_testsupport::{CommandExt as _, git, gix_testtools::tempfile::TempDir, open_repo};

#[test]
fn new_from_project_handle_uses_repo_gitdir() -> anyhow::Result<()> {
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

    let ctx = Context::from_repo(repo.clone())?;
    assert_eq!(
        ctx.gitdir,
        repo.path(),
        "When creating a context from a repo directly, it will not alter the stored path though."
    );
    Ok(())
}

#[test]
fn new_from_project_handle_keeps_repo_cached() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("unborn-empty")?;
    let handle = ProjectHandle::from_path(repo.git_dir())?;
    let ctx = Context::new_from_project_handle(handle)?;

    assert!(
        ctx.repo.get_opt().is_some(),
        "the repository used during construction should be kept in context"
    );
    assert!(ctx.to_sync().repo.is_some());
    Ok(())
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

    let ctx = Context::from_repo(repo)?;
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
    let ctx = Context::from_repo(repo)?;

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
}

#[test]
fn set_project_meta_updates_git_config_toml_and_database() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo(repo)?;
    let project_meta = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        db: None,
    }
    "#);

    ctx.set_project_meta(project_meta.clone())?;

    insta::assert_debug_snapshot!(storage_state_with_db(&ctx)?, @r#"
    StorageState {
        config_ported: true,
        config: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: Some(
            DbStateView {
                initialized: true,
                default_target_remote_name: Some(
                    "origin",
                ),
                default_target_branch_name: Some(
                    "main",
                ),
                default_target_sha: Some(
                    "[OID]",
                ),
                default_target_push_remote_name: Some(
                    "fork",
                ),
            },
        ),
    }
    "#);
    Ok(())
}

#[test]
fn set_project_meta_fills_missing_target_commit_id_from_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let expected_target_id = {
        let mut target_ref = repo.find_reference("refs/remotes/origin/main")?;
        target_ref.peel_to_commit()?.id
    };
    let ctx = Context::from_repo(repo)?;

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
    let state = storage_state(&ctx)?;
    assert_eq!(state.config.target_commit_id, Some("[OID]"));
    assert_eq!(state.toml.target_commit_id, Some("[OID]"));
    Ok(())
}

#[test]
fn set_project_meta_clears_missing_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/missing".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    assert_eq!(ctx.project_meta()?.target_ref, None);
    let state = storage_state(&ctx)?;
    assert_eq!(state.config.target_ref, None);
    assert_eq!(state.toml.target_ref, None);
    Ok(())
}

#[test]
fn project_meta_defaults_when_config_and_toml_are_unset() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo(repo)?;

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        db: None,
    }
    "#);

    let actual = ctx.project_meta()?;
    insta::assert_snapshot!(project_meta_summary(actual), @"target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>");

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        db: None,
    }
    "#);
    Ok(())
}

#[test]
fn project_meta_observes_changes_made_through_other_repository_handles() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo(repo)?;
    assert_eq!(ctx.project_meta()?.target_ref, None);

    // Write through an independent handle, like another process would.
    let other_ctx = Context::from_repo(open_repo(&ctx.gitdir)?)?;
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
fn project_meta_falls_back_to_toml_and_ports_on_first_write() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-toml")?;
    let ctx = Context::from_repo(repo)?;

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);

    let actual = ctx.project_meta()?;
    insta::assert_snapshot!(
        project_meta_summary(actual.clone()),
        @"target_ref=refs/remotes/origin/main; target_commit_id=[OID]; push_remote=fork"
    );

    // Reading is pure - nothing was ported yet.
    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);

    // The first write ports the metadata to Git config.
    ctx.set_project_meta(actual)?;
    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: true,
        config: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);
    Ok(())
}

#[test]
fn project_meta_reads_git_config_when_ported_even_if_toml_differs() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    let ctx = Context::from_repo(repo)?;

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: true,
        config: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/upstream/trunk",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "origin",
            ),
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);

    let actual = ctx.project_meta()?;
    insta::assert_snapshot!(
        project_meta_summary(actual),
        @"target_ref=refs/remotes/upstream/trunk; target_commit_id=[OID]; push_remote=origin"
    );

    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: true,
        config: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/upstream/trunk",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "origin",
            ),
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);
    Ok(())
}

#[test]
fn resync_project_meta_from_legacy_leaves_unported_repos_alone() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-toml")?;
    let ctx = Context::from_repo(repo)?;

    ctx.resync_project_meta_from_legacy()?;

    // The TOML is still the only source of truth - a snapshot restore must not
    // perform the initial port, as the ported marker is never unset and would hide
    // future TOML-only writes by older binaries.
    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: false,
        config: ProjectMetaView {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);
    Ok(())
}

#[test]
fn resync_project_meta_from_legacy_rewrites_config_from_toml_when_ported() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    let ctx = Context::from_repo(repo)?;

    ctx.resync_project_meta_from_legacy()?;

    // The repository was already ported, so the restored TOML wins over the
    // outdated Git config values, and the repository stays ported.
    insta::assert_debug_snapshot!(storage_state(&ctx)?, @r#"
    StorageState {
        config_ported: true,
        config: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        toml: ProjectMetaView {
            target_ref: Some(
                "refs/remotes/origin/main",
            ),
            target_commit_id: Some(
                "[OID]",
            ),
            push_remote: Some(
                "fork",
            ),
        },
        db: None,
    }
    "#);
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

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "fields are asserted through insta debug snapshots"
)]
struct StorageState {
    config_ported: bool,
    config: ProjectMetaView,
    toml: ProjectMetaView,
    db: Option<DbStateView>,
}

#[derive(Debug)]
struct ProjectMetaView {
    target_ref: Option<String>,
    target_commit_id: Option<&'static str>,
    push_remote: Option<String>,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "fields are asserted through insta debug snapshots"
)]
struct DbStateView {
    initialized: bool,
    default_target_remote_name: Option<String>,
    default_target_branch_name: Option<String>,
    default_target_sha: Option<&'static str>,
    default_target_push_remote_name: Option<String>,
}

/// Read the storage state as it is on disk, like production reads do.
fn storage_state(ctx: &Context) -> anyhow::Result<StorageState> {
    storage_state_with_repo(ctx, open_repo(&ctx.gitdir)?, false)
}

fn storage_state_with_db(ctx: &Context) -> anyhow::Result<StorageState> {
    storage_state_with_repo(ctx, open_repo(&ctx.gitdir)?, true)
}

fn storage_state_with_repo(
    ctx: &Context,
    repo: gix::Repository,
    include_db: bool,
) -> anyhow::Result<StorageState> {
    let toml_meta = VirtualBranchesTomlMetadata::from_path_read_only(
        ctx.project_data_dir().join("virtual_branches.toml"),
    )?;
    let toml = toml_meta.workspace(WORKSPACE_REF_NAME.try_into()?)?;
    let db = if include_db {
        ctx.db
            .get_cache()?
            .virtual_branches()
            .get_snapshot()?
            .map(|snapshot| DbStateView {
                initialized: snapshot.state.initialized,
                default_target_remote_name: snapshot.state.default_target_remote_name,
                default_target_branch_name: snapshot.state.default_target_branch_name,
                default_target_sha: snapshot.state.default_target_sha.as_ref().map(|_| "[OID]"),
                default_target_push_remote_name: snapshot.state.default_target_push_remote_name,
            })
    } else {
        None
    };

    Ok(StorageState {
        config_ported: ProjectMeta::is_ported(&repo.config_snapshot()),
        config: ProjectMeta::try_from_config(&repo.config_snapshot())?.into(),
        toml: toml.project_meta().into(),
        db,
    })
}

fn project_meta_summary(project_meta: ProjectMeta) -> String {
    let view = ProjectMetaView::from(project_meta);
    format!(
        "target_ref={}; target_commit_id={}; push_remote={}",
        view.target_ref.as_deref().unwrap_or("<unset>"),
        view.target_commit_id.unwrap_or("<unset>"),
        view.push_remote.as_deref().unwrap_or("<unset>")
    )
}

impl From<ProjectMeta> for ProjectMetaView {
    fn from(value: ProjectMeta) -> Self {
        ProjectMetaView {
            target_ref: value.target_ref.map(|name| name.to_string()),
            target_commit_id: value.target_commit_id.map(|_| "[OID]"),
            push_remote: value.push_remote,
        }
    }
}
