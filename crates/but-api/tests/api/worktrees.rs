use anyhow::Result;
use but_api::worktrees::{ListedWorktree, worktree_remove, worktree_set_archived, worktrees_list};
use but_testsupport::{CommandExt, git_at_dir};

use crate::support::{repo_with_feature_branch, write_file};

/// A flag-on context around [`repo_with_feature_branch()`], with adoption already run so
/// worktrees added afterwards start out active.
fn flag_on_ctx() -> Result<(but_ctx::Context, tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert!(
        worktrees_list(&mut ctx)?.active.is_empty(),
        "adoption runs on the first read, with nothing to adopt"
    );
    Ok((ctx, tmp))
}

/// Add the linked worktree `name` under `tmp/worktrees` on a new branch of the same name,
/// stamping its reflog entries with `committer_date`.
fn add_worktree(tmp: &std::path::Path, name: &str, committer_date: &str) -> std::path::PathBuf {
    let path = tmp.join("worktrees").join(name);
    git_at_dir(tmp)
        .env("GIT_COMMITTER_DATE", committer_date)
        .args(["worktree", "add", "-b", name])
        .arg(&path)
        .arg("HEAD~1")
        .run();
    path
}

fn names(worktrees: &[ListedWorktree]) -> Vec<&str> {
    worktrees
        .iter()
        .map(|wt| std::str::from_utf8(&wt.name).expect("ascii names"))
        .collect()
}

#[test]
fn listing_is_sorted_by_reflog_recency_then_name() -> Result<()> {
    let (mut ctx, tmp) = flag_on_ctx()?;
    add_worktree(tmp.path(), "older", "2000-01-03 00:00:00 +0000");
    add_worktree(tmp.path(), "newer", "2000-01-04 00:00:00 +0000");
    add_worktree(tmp.path(), "same-day-b", "2000-01-02 00:00:00 +0000");
    add_worktree(tmp.path(), "same-day-a", "2000-01-02 00:00:00 +0000");
    git_at_dir(tmp.path())
        .args([
            "-c",
            "core.logAllRefUpdates=false",
            "worktree",
            "add",
            "-b",
            "nolog",
        ])
        .arg(tmp.path().join("worktrees/nolog"))
        .arg("HEAD~1")
        .run();

    let listing = worktrees_list(&mut ctx)?;
    // Newest first, ties by name, and a worktree without any reflog goes last.
    assert_eq!(
        names(&listing.active),
        ["newer", "older", "same-day-a", "same-day-b", "nolog"]
    );
    assert_eq!(
        listing
            .active
            .iter()
            .map(|wt| wt.updated_at_ms)
            .collect::<Vec<_>>(),
        [
            Some(946_944_000_000),
            Some(946_857_600_000),
            Some(946_771_200_000),
            Some(946_771_200_000),
            None
        ]
    );
    assert!(listing.archived.is_empty());

    worktree_set_archived(&mut ctx, "older".into(), true)?;
    let listing = worktrees_list(&mut ctx)?;
    assert_eq!(
        names(&listing.active),
        ["newer", "same-day-a", "same-day-b", "nolog"]
    );
    assert_eq!(names(&listing.archived), ["older"]);
    Ok(())
}

#[test]
fn remove_needs_force_for_a_dirty_checkout_and_forgets_the_archived_state() -> Result<()> {
    let (mut ctx, tmp) = flag_on_ctx()?;
    let path = add_worktree(tmp.path(), "wt", "2000-01-03 00:00:00 +0000");
    write_file(&path, "file.txt", "dirty\n")?;
    worktree_set_archived(&mut ctx, "wt".into(), true)?;

    let err = worktree_remove(&mut ctx, "wt".into(), false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    assert_eq!(names(&worktrees_list(&mut ctx)?.archived), ["wt"]);

    worktree_remove(&mut ctx, "wt".into(), true)?;
    assert!(!path.exists());
    let listing = worktrees_list(&mut ctx)?;
    assert!(listing.active.is_empty() && listing.archived.is_empty());

    // Re-creating the name yields an active worktree: the archived row went with the checkout,
    // while the branch stayed, as it does with `git worktree remove`.
    git_at_dir(tmp.path())
        .args(["worktree", "add"])
        .arg(&path)
        .arg("wt")
        .run();
    assert_eq!(names(&worktrees_list(&mut ctx)?.active), ["wt"]);

    let err = worktree_remove(&mut ctx, "missing".into(), true).unwrap_err();
    assert_eq!(err.to_string(), "Worktree missing does not exist");
    Ok(())
}

#[test]
fn everything_is_gated_on_the_feature_flag() -> Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    for err in [
        worktrees_list(&mut ctx).map(drop).unwrap_err(),
        worktree_set_archived(&mut ctx, "wt".into(), true).unwrap_err(),
        worktree_remove(&mut ctx, "wt".into(), true).unwrap_err(),
    ] {
        assert_eq!(
            err.to_string(),
            "worktree manipulation is not enabled (featureFlags.worktreeManipulation)"
        );
    }
    Ok(())
}
