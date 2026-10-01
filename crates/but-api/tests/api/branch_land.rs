use but_testsupport::{CommandExt, git_at_dir};

use crate::support::{repo_with_feature_branch, set_project_target_to_feature, write_file};

/// Outside the managed workspace, landing is single-branch-mode behavior, so it stays refused
/// while that feature is off.
#[test]
fn land_outside_workspace_requires_single_branch_mode() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.single_branch = false;

    let err = but_api::land::branch_land(&mut ctx, "main".into(), false, false)
        .expect_err("HEAD is on main, outside the managed workspace");

    assert_eq!(
        err.to_string(),
        "`but merge` requires the GitButler workspace or, in single-branch mode, a checked-out \
         branch."
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
    ctx.settings.feature_flags.single_branch = true;

    let err = but_api::land::branch_land(&mut ctx, "topic".into(), false, false)
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
