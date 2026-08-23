use but_core::{DryRun, ref_metadata::ProjectMeta};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_testsupport::{CommandExt, git_at_dir, open_repo};
use gitbutler_oplog::OplogExt as _;

use crate::support::write_file;

fn context_with_cherry_pick_history() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
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

    write_file(tmp.path(), "source-one.txt", "one\n")?;
    git_at_dir(tmp.path()).args(["add", "source-one.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "source one"])
        .run();
    write_file(tmp.path(), "source-two.txt", "two\n")?;
    git_at_dir(tmp.path()).args(["add", "source-two.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "source two"])
        .run();

    write_file(tmp.path(), "main.txt", "main\n")?;
    git_at_dir(tmp.path()).args(["add", "main.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "main"]).run();

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

#[test]
fn cherry_pick_materializes_multiple_deduped_commits_and_returns_new_commit_ids()
-> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_cherry_pick_history()?;
    let (source_first, source_tip, main_tip) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("main~2")?.detach(),
            repo.rev_parse_single("main~1")?.detach(),
            repo.rev_parse_single("main")?.detach(),
        )
    };
    let main_ref: gix::refs::FullName = "refs/heads/main".try_into()?;

    let result = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        vec![source_tip, source_first, source_tip],
        RelativeTo::Reference(main_ref.clone()),
        InsertSide::Below,
        DryRun::No,
    )?;

    assert_eq!(result.new_commits.len(), 2);
    let new_first = result.new_commits[0];
    let new_tip = result.new_commits[1];
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.rev_parse_single(main_ref.as_ref())?,
        new_tip,
        "materialization should move the destination reference"
    );
    assert_eq!(
        repo.find_commit(new_first)?
            .parent_ids()
            .next()
            .expect("the copied commit has the old branch tip as parent"),
        main_tip
    );

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .collect::<Result<Vec<_>, _>>()?;
    let cherry_pick = snapshots
        .iter()
        .find_map(|snapshot| {
            snapshot
                .details
                .as_ref()
                .filter(|details| details.operation == but_oplog::legacy::OperationKind::CherryPick)
        })
        .expect("the cherry-pick should record an oplog snapshot");
    assert_eq!(
        cherry_pick.title, "CherryPick (2)",
        "the oplog title should count unique cherry-picked commits"
    );
    Ok(())
}

#[test]
fn cherry_pick_dry_run_does_not_persist_commits_or_move_the_reference() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_cherry_pick_history()?;
    let (source_first, source_tip, main_tip) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("main~2")?.detach(),
            repo.rev_parse_single("main~1")?.detach(),
            repo.rev_parse_single("main")?.detach(),
        )
    };
    let main_ref: gix::refs::FullName = "refs/heads/main".try_into()?;

    let result = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        vec![source_first, source_tip],
        RelativeTo::Reference(main_ref.clone()),
        InsertSide::Below,
        DryRun::Yes,
    )?;

    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.rev_parse_single(main_ref.as_ref())?,
        main_tip,
        "dry-run should not move the destination reference"
    );
    for new_commit in result.new_commits {
        assert!(
            repo.find_object(new_commit).is_err(),
            "dry-run commits should remain in the preview object database"
        );
    }
    Ok(())
}

/// Add a branch holding `outside-one` and `outside-two` that the workspace traversal doesn't
/// reach, the way the branches view offers commits from unapplied branches for cherry-picking.
fn add_branch_outside_the_workspace(tmp: &std::path::Path) -> anyhow::Result<()> {
    git_at_dir(tmp)
        .args(["checkout", "-b", "unapplied", "refs/remotes/origin/main"])
        .run();
    for name in ["outside-one", "outside-two"] {
        write_file(tmp, &format!("{name}.txt"), "outside\n")?;
        git_at_dir(tmp).args(["add", "."]).run();
        git_at_dir(tmp).args(["commit", "-m", name]).run();
    }
    git_at_dir(tmp).args(["checkout", "main"]).run();
    Ok(())
}

#[test]
fn cherry_pick_from_a_branch_outside_the_workspace() -> anyhow::Result<()> {
    let (mut ctx, tmp) = context_with_cherry_pick_history()?;
    add_branch_outside_the_workspace(tmp.path())?;

    let (outside_commit, main_tip) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/heads/unapplied")?.detach(),
            repo.rev_parse_single("main")?.detach(),
        )
    };
    let main_ref: gix::refs::FullName = "refs/heads/main".try_into()?;

    let result = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        vec![outside_commit],
        RelativeTo::Reference(main_ref.clone()),
        InsertSide::Below,
        DryRun::No,
    )?;

    assert_eq!(result.new_commits.len(), 1);
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.rev_parse_single(main_ref.as_ref())?,
        result.new_commits[0],
        "the copy becomes the new branch tip"
    );
    assert_eq!(
        repo.find_commit(result.new_commits[0])?
            .parent_ids()
            .next()
            .expect("the copy is stacked onto the previous tip"),
        main_tip
    );
    assert_ne!(
        result.new_commits[0], outside_commit,
        "the source is copied, not moved"
    );
    Ok(())
}

#[test]
fn cherry_pick_multiple_commits_from_outside_the_workspace() -> anyhow::Result<()> {
    let (mut ctx, tmp) = context_with_cherry_pick_history()?;
    add_branch_outside_the_workspace(tmp.path())?;

    let (outside_first, outside_second) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/heads/unapplied~1")?.detach(),
            repo.rev_parse_single("refs/heads/unapplied")?.detach(),
        )
    };
    let main_ref: gix::refs::FullName = "refs/heads/main".try_into()?;

    let result = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        vec![outside_first, outside_second],
        RelativeTo::Reference(main_ref.clone()),
        InsertSide::Below,
        DryRun::No,
    )?;

    assert_eq!(result.new_commits.len(), 2);
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.rev_parse_single(main_ref.as_ref())?,
        result.new_commits[1],
        "the last source given ends up as the branch tip"
    );
    assert_eq!(
        repo.find_commit(result.new_commits[1])?
            .parent_ids()
            .next()
            .expect("the copies are stacked in the order given"),
        result.new_commits[0]
    );
    let titles = result
        .new_commits
        .iter()
        .map(|id| -> anyhow::Result<String> {
            Ok(repo.find_commit(*id)?.message()?.title.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        titles,
        ["outside-one\n", "outside-two\n"],
        "the copies keep the order the sources were given in"
    );
    Ok(())
}
