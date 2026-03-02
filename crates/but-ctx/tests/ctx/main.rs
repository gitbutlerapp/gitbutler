use but_ctx::{Context, ProjectHandle};

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
        "When creatinga a context from a repo directly, it will not alter the stored path though."
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
