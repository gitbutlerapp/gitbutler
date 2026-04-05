use but_ctx::{Context, ProjectHandle};
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

    let _db = ctx.db.get()?;
    assert!(
        ctx.project_data_dir().join("but.sqlite").exists(),
        "database should be created in configured project-data directory"
    );
    let _cache = ctx.cache.get_cache()?;
    assert!(
        ctx.project_data_dir().join("but_cache.sqlite").exists(),
        "project cache should be created in configured project-data directory"
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
    let _cache = restored.cache.get_cache()?;
    assert!(
        restored
            .project_data_dir()
            .join("but_cache.sqlite")
            .exists(),
        "thread-local restoration should still initialize the project cache in the project data dir"
    );
    Ok(())
}

#[test]
fn memory_cache_does_not_create_project_cache_file() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    gix::init(repo_dir.path())?;
    let repo = open_repo(repo_dir.path())?;
    let ctx = Context::from_repo(repo)?.with_memory_cache();

    let _cache = ctx.cache.get_cache()?;
    assert!(
        !ctx.project_data_dir().join("but_cache.sqlite").exists(),
        "project cache should remain in-memory"
    );
    Ok(())
}
