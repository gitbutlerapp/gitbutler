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
        .args(["remote", "add", "origin", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(tmp.path())
        .args(["branch", "--set-upstream-to=origin/main", "main"])
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
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
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

/// Uncommitting from a linked worktree's commit rewrites its branch and leaves the extracted
/// change in that worktree's checkout, where it shows up as uncommitted. The main checkout
/// is left alone.
#[test]
fn uncommitting_from_a_linked_worktree_commit_keeps_the_change_in_its_checkout()
-> anyhow::Result<()> {
    use but_api::commit::{json::ChangesSource, types::UncommitChangesSource};

    use crate::support::{checkout_branch_in_linked_worktree, repo_with_feature_branch};

    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let wt = linked.path().join("wt");
    write_file(&wt, "wt.txt", "worktree change\n")?;
    git_at_dir(&wt).args(["add", "wt.txt"]).run();
    git_at_dir(&wt)
        .args(["commit", "-m", "worktree commit"])
        .run();
    let worktree_commit = repo.rev_parse_single("refs/heads/feature")?.detach();
    let main_head = repo.head_id()?.detach();

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    // Adoption archives the worktrees that exist when the flag is first read.
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;

    let outcome = but_api::commit::uncommit::commit_uncommit_changes_from_commits(
        &mut ctx,
        vec![UncommitChangesSource {
            commit_id: worktree_commit,
            changes: vec![but_core::DiffSpec {
                previous_path: None,
                path: "wt.txt".into(),
                hunk_headers: vec![],
            }],
        }],
        None,
        DryRun::No,
    )?;
    let failures: Vec<_> = outcome.failures.iter().map(|f| f.error.clone()).collect();
    assert!(failures.is_empty(), "{failures:?}");

    {
        let repo = ctx.repo.get()?;
        let rewritten = repo.rev_parse_single("refs/heads/feature")?.detach();
        assert_ne!(
            rewritten, worktree_commit,
            "the worktree's branch moves to the rewrite"
        );
        let tree = repo.find_commit(rewritten)?.tree()?;
        assert!(
            tree.lookup_entry_by_path("wt.txt")?.is_none(),
            "the rewritten commit no longer has the file"
        );
        assert_eq!(
            std::fs::read_to_string(wt.join("wt.txt"))?,
            "worktree change\n",
            "the file stays in the worktree's checkout"
        );
        assert_eq!(
            repo.head_id()?.detach(),
            main_head,
            "the main checkout is untouched"
        );
    }

    let guard = ctx.shared_worktree_access();
    let changes = but_api::diff::changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        guard.read_permission(),
    )?;
    let paths: Vec<_> = changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| change.path.clone())
        .collect();
    assert_eq!(paths, ["wt.txt"], "it is uncommitted in the worktree");
    Ok(())
}
