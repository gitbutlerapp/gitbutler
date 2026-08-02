use but_api::json::HexHash;
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::{repo_with_feature_branch, write_file};

fn incoming_target() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    write_file(tmp.path(), "file.txt", "three\n")?;
    git_at_dir(tmp.path())
        .args(["commit", "-am", "three"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    git_at_dir(tmp.path())
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(tmp.path())
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    drop(repo);

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    Ok((ctx, tmp))
}

#[test]
fn reports_clipping_and_accepts_a_zero_limit() -> anyhow::Result<()> {
    let (ctx, _tmp) = incoming_target()?;

    let zero = but_api::target_commits::workspace_target_commits(&ctx, None, Some(0))?;
    assert!(
        zero.commits.is_empty(),
        "a zero-sized page contains no commits"
    );
    assert!(
        zero.has_more,
        "the empty page is clipped before the workspace bound"
    );

    let first = but_api::target_commits::workspace_target_commits(&ctx, None, Some(1))?;
    assert_eq!(
        first.commits.len(),
        1,
        "the requested page size is respected"
    );
    assert!(
        first.has_more,
        "another relative commit remains after the first page"
    );

    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    assert_eq!(
        complete.commits.len(),
        3,
        "the complete page includes two incoming commits and the lower bound"
    );
    assert!(
        !complete.has_more,
        "the complete page reached its workspace bound"
    );
    Ok(())
}

#[test]
fn continuation_excludes_its_cursor_and_reports_more_history() -> anyhow::Result<()> {
    let (ctx, _tmp) = incoming_target()?;
    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    let cursor = complete
        .commits
        .first()
        .expect("the relative page has a tip");

    let page = but_api::target_commits::workspace_target_commits(
        &ctx,
        Some(HexHash(cursor.commit.id)),
        Some(1),
    )?;
    assert_eq!(page.commits.len(), 1, "one older commit is returned");
    assert_ne!(
        page.commits[0].commit.id, cursor.commit.id,
        "continuation starts below its cursor"
    );
    assert!(
        page.has_more,
        "the repository has more history below this page"
    );
    Ok(())
}
