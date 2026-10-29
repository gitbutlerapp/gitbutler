use but_testsupport::{CommandExt, git_at_dir};

use crate::support::{repo_with_feature_branch, set_project_target_to_feature, write_file};

fn assert_target_rejected_without_changes(
    root: &std::path::Path,
    expected_error: &str,
) -> anyhow::Result<()> {
    let repo = but_testsupport::open_repo(root)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    write_file(root, "file.txt", "dirty work\n")?;
    let metadata_before = ctx.project_meta()?;
    let config_before = std::fs::read(root.join(".git/config"))?;
    let refs_before = git_at_dir(root).args(["show-ref"]).output()?.stdout;
    let head_before = git_at_dir(root)
        .args(["symbolic-ref", "HEAD"])
        .output()?
        .stdout;
    let diff_before = git_at_dir(root).args(["diff", "HEAD"]).output()?.stdout;
    let error = but_api::land::branch_land(&mut ctx, "main".into(), false, false, false, None)
        .expect_err("unsupported targets must be rejected before delivery");
    assert_eq!(
        error.to_string(),
        expected_error,
        "reject the target configuration before attempting delivery"
    );
    assert_eq!(
        ctx.project_meta()?,
        metadata_before,
        "project metadata is unchanged"
    );
    assert_eq!(
        std::fs::read(root.join(".git/config"))?,
        config_before,
        "config bytes are unchanged"
    );
    assert_eq!(
        git_at_dir(root).args(["show-ref"]).output()?.stdout,
        refs_before,
        "no local or tracking refs move"
    );
    assert_eq!(
        git_at_dir(root)
            .args(["symbolic-ref", "HEAD"])
            .output()?
            .stdout,
        head_before,
        "HEAD is unchanged"
    );
    assert_eq!(
        git_at_dir(root).args(["diff", "HEAD"]).output()?.stdout,
        diff_before,
        "worktree changes are preserved"
    );
    Ok(())
}

#[test]
fn land_refuses_differently_named_target_upstream() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/trunk", "origin/main"])
        .run();
    git_at_dir(tmp.path())
        .args(["branch", "--set-upstream-to=origin/trunk", "main"])
        .run();
    assert_target_rejected_without_changes(
        tmp.path(),
        "`but merge` requires matching local and upstream branch names (local `main`, upstream `trunk`). Merge via a pull request instead.",
    )
}

#[test]
fn land_refuses_triangular_remotes() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    git_at_dir(tmp.path())
        .args(["remote", "add", "fork", "../fork"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "gitbutler.project.pushRemote", "fork"])
        .run();
    assert_target_rejected_without_changes(
        tmp.path(),
        "`but merge` does not yet support triangular remotes (fetch `origin`, push `fork`). Merge via a pull request instead, or configure a single remote for the target branch.",
    )
}

/// A detached HEAD cannot be used as a landing checkout.
#[test]
fn land_refuses_detached_head() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    git_at_dir(tmp.path())
        .args(["checkout", "--detach", "-q"])
        .run();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::land::branch_land(&mut ctx, "main".into(), false, false, false, None)
        .expect_err("HEAD is detached, outside the managed workspace");

    assert_eq!(
        err.to_string(),
        "`but merge` requires the GitButler workspace or a checked-out branch."
    );
    Ok(())
}

/// Landing validates the branch's stack, so a branch in no lane is refused before anything is
/// fetched or published, rather than published without those checks.
#[test]
fn land_refuses_branch_outside_the_workspace() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let git = || git_at_dir(tmp.path());
    git().args(["checkout", "-q", "-b", "topic"]).run();
    write_file(tmp.path(), "topic.txt", "topic\n")?;
    git().args(["add", "topic.txt"]).run();
    git().args(["commit", "-q", "-m", "topic"]).run();
    git().args(["checkout", "-q", "main"]).run();
    let refs_before = git().args(["show-ref"]).output()?.stdout;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::land::branch_land(&mut ctx, "topic".into(), false, false, false, None)
        .expect_err("topic is not part of the checked-out stack");

    assert_eq!(
        err.to_string(),
        "Refusing to land `topic`: it is not in the workspace. Apply or check out the branch first."
    );
    assert_eq!(
        git().args(["show-ref"]).output()?.stdout,
        refs_before,
        "a refused land moves no refs"
    );
    Ok(())
}
