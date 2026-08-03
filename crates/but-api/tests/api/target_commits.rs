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

/// A still-applied stack landed upstream through a merge commit: the workspace
/// lower bound becomes the stack tip, which sits on the merge's *second*
/// parent and is never met by the first-parent walk. The stack's base bounds
/// the walk instead of running to the commit cap (or repository root).
#[test]
fn merge_integrated_stack_bounds_the_walk_at_its_base() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();
    write_file(tmp.path(), "file.txt", "zero\n")?;
    git_at_dir(tmp.path()).args(["add", "file.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "zero"]).run();
    write_file(tmp.path(), "file.txt", "one\n")?;
    git_at_dir(tmp.path()).args(["commit", "-am", "one"]).run();
    git_at_dir(tmp.path()).args(["branch", "feature"]).run();
    write_file(tmp.path(), "file.txt", "two\n")?;
    git_at_dir(tmp.path()).args(["commit", "-am", "two"]).run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    write_file(tmp.path(), "feature.txt", "work\n")?;
    git_at_dir(tmp.path()).args(["add", "feature.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "feature-work"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "main"]).run();
    git_at_dir(tmp.path())
        .args(["merge", "--no-ff", "feature", "-m", "merge feature"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "remote.origin.url", "../origin"])
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

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    let feature_ref = gix::refs::FullName::try_from("refs/heads/feature")?;
    but_api::branch::apply_only(&mut ctx, feature_ref.as_ref())?;

    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    let titles: Vec<_> = complete
        .commits
        .iter()
        .map(|entry| {
            let message = entry.commit.message.to_string();
            message.lines().next().unwrap_or_default().to_owned()
        })
        .collect();
    assert_eq!(
        titles,
        ["merge feature", "two", "one"],
        "the walk stops at the stack's fork point instead of running past it to the root"
    );
    assert!(
        !complete.has_more,
        "stopping at the fork point is a natural bound, not a clip"
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
