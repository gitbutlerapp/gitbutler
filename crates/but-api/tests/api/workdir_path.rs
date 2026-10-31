use anyhow::Result;
use but_api::commit::json::ChangesSource;

use crate::support::{checkout_branch_in_linked_worktree, repo_with_feature_branch};

#[test]
fn resolves_a_path_in_the_named_checkout() -> Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;

    let main = but_api::open::workdir_path(&ctx, ChangesSource::Head, "dir/file.txt".into())?;
    assert_eq!(
        std::path::Path::new(&main),
        tmp.path().join("dir/file.txt"),
        "the main checkout is the project's own workdir"
    );

    let worktree = but_api::open::workdir_path(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        "file.txt".into(),
    )?;
    assert_eq!(
        gix::path::realpath(std::path::Path::new(&worktree).parent().unwrap())?,
        gix::path::realpath(linked.path().join("wt"))?,
        "a linked worktree's path is in its own folder"
    );
    Ok(())
}

#[test]
fn refuses_paths_leaving_the_checkout() -> Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    for path in ["../outside.txt", "/etc/hosts", "dir/../../outside.txt"] {
        let err = but_api::open::workdir_path(&ctx, ChangesSource::Head, path.into()).unwrap_err();
        assert!(
            err.to_string().contains("must be relative to the checkout"),
            "{path} would resolve outside the checkout: {err}"
        );
    }
    Ok(())
}

#[test]
fn refuses_an_archived_linked_worktree() -> Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let _linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::open::workdir_path(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        "file.txt".into(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Worktree wt is archived",
        "a worktree that predates the first read is adopted as archived"
    );
    Ok(())
}
