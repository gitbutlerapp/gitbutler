use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::{repo_with_feature_branch, write_file};

/// `one` writes `file.txt`, `two` rewrites it and `three` adds `other.txt`, so each range differs.
fn three_commits() -> anyhow::Result<(but_ctx::Context, [gix::ObjectId; 3], tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    write_file(tmp.path(), "other.txt", "other\n")?;
    git_at_dir(tmp.path()).args(["add", "other.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "three"]).run();
    drop(repo);

    let repo = open_repo(tmp.path())?;
    let id =
        |spec: &str| -> anyhow::Result<gix::ObjectId> { Ok(repo.rev_parse_single(spec)?.detach()) };
    let commits = [id("HEAD~2")?, id("HEAD~1")?, id("HEAD")?];
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, commits, tmp))
}

fn paths(changes: &but_core::ui::TreeChanges) -> Vec<String> {
    changes
        .changes
        .iter()
        .map(|change| change.path_bytes.to_string())
        .collect()
}

#[test]
fn one_commit_is_its_own_changes() -> anyhow::Result<()> {
    let (ctx, [_, _, three], _tmp) = three_commits()?;

    let diff = but_api::diff::commit_range_diff(&ctx, three, three)?;
    assert_eq!(
        paths(&diff),
        ["other.txt"],
        "only what the commit itself changed"
    );
    assert_eq!(diff.stats.lines_added, 1, "other.txt has one line");
    assert_eq!(diff.stats.lines_removed, 0, "three only adds a file");
    Ok(())
}

#[test]
fn a_range_combines_its_commits() -> anyhow::Result<()> {
    let (ctx, [_, two, three], _tmp) = three_commits()?;

    let diff = but_api::diff::commit_range_diff(&ctx, two, three)?;
    assert_eq!(
        paths(&diff),
        ["file.txt", "other.txt"],
        "the oldest commit's parent is the base, so the oldest commit's own change is included"
    );
    assert_eq!(
        diff.stats.lines_added, 2,
        "two -> file.txt, three -> other.txt"
    );
    assert_eq!(
        diff.stats.lines_removed, 1,
        "two replaced file.txt's only line"
    );
    Ok(())
}

#[test]
fn a_range_from_the_root_commit_diffs_against_nothing() -> anyhow::Result<()> {
    let (ctx, [one, _, three], _tmp) = three_commits()?;

    let diff = but_api::diff::commit_range_diff(&ctx, one, three)?;
    assert_eq!(
        paths(&diff),
        ["file.txt", "other.txt"],
        "with no parent, every file in the newest tree is an addition"
    );
    assert_eq!(
        diff.stats.lines_removed, 0,
        "nothing existed before the root commit"
    );
    Ok(())
}
