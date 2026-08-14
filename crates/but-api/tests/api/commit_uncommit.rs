use but_core::{DryRun, ref_metadata::ProjectMeta};
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::write_file;

/// `feature` carries two commits above the target, and `base.txt` is dirty in the worktree.
///
/// The dirty hunk deliberately has no persisted assignment row: that is the state in which
/// uncommit has to tell "hunk that was already here" apart from "hunk the uncommit surfaced".
fn context_with_two_commits_and_a_dirty_file()
-> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init", "-b", "main"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();

    write_file(tmp.path(), "base.txt", "base\n")?;
    git_at_dir(tmp.path()).args(["add", "base.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "base"]).run();
    git_at_dir(tmp.path())
        .args(["config", "remote.origin.url", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();

    git_at_dir(tmp.path())
        .args(["checkout", "-b", "feature"])
        .run();
    write_file(tmp.path(), "first.txt", "first\n")?;
    git_at_dir(tmp.path()).args(["add", "first.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "first"]).run();
    write_file(tmp.path(), "second.txt", "second\n")?;
    git_at_dir(tmp.path()).args(["add", "second.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "second"])
        .run();

    // Pre-existing uncommitted work, unrelated to the commit about to be uncommitted.
    write_file(tmp.path(), "base.txt", "base edited in the worktree\n")?;

    let repo = open_repo(tmp.path())?;
    let target_commit_id = repo.rev_parse_single("refs/remotes/origin/main")?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;

    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, tmp))
}

/// `assign_to` must claim only the hunks the uncommit surfaced.
///
/// The two hunk-assignment passes either side of the rebase compare freshly minted assignment
/// ids, so the first pass has to be visible to the second. When it is not, every worktree hunk
/// without a persisted row looks new and gets swept into the target stack.
#[test]
fn uncommit_assigns_only_the_surfaced_hunks_to_the_target_stack() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_two_commits_and_a_dirty_file()?;

    let (stack_id, subject_commit_id) = {
        let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
        let stack = ws.stacks.first().expect("`feature` is the only stack");
        let stack_id = stack.id.expect("a workspace stack carries an id");
        let subject_commit_id = repo.rev_parse_single("refs/heads/feature")?.detach();
        (stack_id, subject_commit_id)
    };

    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        vec![subject_commit_id],
        Some(stack_id),
        DryRun::No,
    )?;

    let db = ctx.db.get_cache()?;
    let assigned_paths_by_branch: Vec<(String, bool)> = {
        let mut rows = db.hunk_assignments().list_all()?;
        rows.sort_by(|a, b| a.path.cmp(&b.path));
        rows.into_iter()
            .map(|row| (row.path, row.branch_ref_bytes.is_some()))
            .collect()
    };

    assert_eq!(
        assigned_paths_by_branch,
        vec![
            ("base.txt".to_string(), false),
            ("second.txt".to_string(), true)
        ],
        "only `second.txt`, surfaced by the uncommit, is assigned; the pre-existing `base.txt` \
         edit stays where the user left it"
    );
    Ok(())
}

/// A dry run must leave the assignment table exactly as it found it.
#[test]
fn uncommit_dry_run_persists_no_assignments() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_two_commits_and_a_dirty_file()?;

    let (stack_id, subject_commit_id) = {
        let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
        let stack = ws.stacks.first().expect("`feature` is the only stack");
        let stack_id = stack.id.expect("a workspace stack carries an id");
        let subject_commit_id = repo.rev_parse_single("refs/heads/feature")?.detach();
        (stack_id, subject_commit_id)
    };

    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        vec![subject_commit_id],
        Some(stack_id),
        DryRun::Yes,
    )?;

    let db = ctx.db.get_cache()?;
    assert!(
        db.hunk_assignments().list_all()?.is_empty(),
        "a dry run neither reports nor persists hunk assignments"
    );
    Ok(())
}
