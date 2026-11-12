use but_api::commit::{json::ChangesSource, types::UncommitChangesSource};
use but_core::{DiffSpec, DryRun, ref_metadata::ProjectMeta};
use but_oplog::legacy::{OperationKind, Trailer};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_testsupport::{CommandExt, git_at_dir, open_repo};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gitbutler_oplog::OplogExt as _;

use crate::support::write_file;

/// `feature` is checked out with `first`, `second` and `third` above the target, each adding
/// a file of its name. `other` holds `other one` and `other two` beside it, and `new.txt` is
/// untracked.
fn context_with_three_commits() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir, Commits)> {
    let tmp = tempfile::tempdir()?;
    let git = |args: &[&str]| git_at_dir(tmp.path()).args(args).run();
    let commit_file = |name: &str, message: &str| -> anyhow::Result<()> {
        write_file(tmp.path(), name, &format!("{message}\n"))?;
        git(&["add", name]);
        git(&["commit", "-m", message]);
        Ok(())
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "GitButler"]);
    git(&["config", "user.email", "gitbutler@example.com"]);
    commit_file("base.txt", "base")?;
    git(&["remote", "add", "origin", "../origin"]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["branch", "--set-upstream-to=origin/main", "main"]);

    git(&["checkout", "-b", "other"]);
    commit_file("other-one.txt", "other one")?;
    commit_file("other-two.txt", "other two")?;

    git(&["checkout", "-b", "feature", "main"]);
    commit_file("first.txt", "first")?;
    commit_file("second.txt", "second")?;
    commit_file("third.txt", "third")?;
    write_file(tmp.path(), "new.txt", "new\n")?;

    let repo = open_repo(tmp.path())?;
    let id =
        |rev: &str| -> anyhow::Result<gix::ObjectId> { Ok(repo.rev_parse_single(rev)?.detach()) };
    let commits = Commits {
        first: id("feature~2")?,
        second: id("feature~1")?,
        third: id("feature")?,
        other_one: id("other~1")?,
        other_two: id("other")?,
    };
    ProjectMeta {
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        target_commit_id: Some(id("refs/remotes/origin/main")?),
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;

    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, tmp, commits))
}

struct Commits {
    first: gix::ObjectId,
    second: gix::ObjectId,
    third: gix::ObjectId,
    other_one: gix::ObjectId,
    other_two: gix::ObjectId,
}

fn file(path: &str) -> Vec<DiffSpec> {
    vec![DiffSpec {
        previous_path: None,
        path: path.into(),
        hunk_headers: vec![],
    }]
}

fn feature() -> RelativeTo {
    RelativeTo::Reference("refs/heads/feature".try_into().expect("valid name"))
}

type Operation = fn(&mut but_ctx::Context, &Commits) -> anyhow::Result<()>;

/// An operation's name, how to run it, and the kind, title and commit trailers it records.
type Case = (
    &'static str,
    Operation,
    OperationKind,
    &'static str,
    fn(&Commits) -> Vec<Trailer>,
);

/// The undo history names each operation, counts the commits it took when there are several,
/// and, for the discards and uncommits that take whole commits, lists them.
#[test]
fn each_commit_operation_records_one_undo_step_named_after_it() -> anyhow::Result<()> {
    use OperationKind::*;
    use but_api::commit::*;

    let operations: Vec<Case> = vec![
        (
            "create",
            |ctx, _| {
                let mut guard = ctx.exclusive_worktree_access();
                create::commit_create(
                    ctx,
                    feature(),
                    InsertSide::Below,
                    file("new.txt"),
                    ChangesSource::Head,
                    "new".into(),
                    DryRun::No,
                    guard.write_permission(),
                )?;
                Ok(())
            },
            CreateCommit,
            "CreateCommit",
            |_| vec![],
        ),
        (
            "amend",
            |ctx, c| {
                amend::commit_amend(
                    ctx,
                    c.third,
                    file("new.txt"),
                    ChangesSource::Head,
                    DryRun::No,
                )?;
                Ok(())
            },
            AmendCommit,
            "AmendCommit",
            |_| vec![],
        ),
        (
            "cherry-pick",
            |ctx, c| {
                cherry_pick::commit_cherry_pick(
                    ctx,
                    vec![c.other_one, c.other_two],
                    feature(),
                    InsertSide::Below,
                    DryRun::No,
                )?;
                Ok(())
            },
            CherryPick,
            "CherryPick (2)",
            |_| vec![],
        ),
        (
            "discard",
            |ctx, c| {
                discard_commit::commit_discard(ctx, vec![c.second, c.third], DryRun::No)?;
                Ok(())
            },
            DiscardCommit,
            "DiscardCommit (2)",
            |c| vec![Trailer::Sha(c.second), Trailer::Sha(c.third)],
        ),
        (
            "discard changes",
            |ctx, c| {
                discard_commit::commit_discard_changes(
                    ctx,
                    c.third,
                    file("third.txt"),
                    DryRun::No,
                )?;
                Ok(())
            },
            DiscardChanges,
            "DiscardChanges",
            |c| vec![Trailer::Sha(c.third)],
        ),
        (
            "insert blank",
            |ctx, _| {
                insert_blank::commit_insert_blank(ctx, feature(), InsertSide::Below, DryRun::No)?;
                Ok(())
            },
            InsertBlankCommit,
            "InsertBlankCommit",
            |_| vec![],
        ),
        (
            "move changes",
            |ctx, c| {
                move_changes::commit_move_changes_between(
                    ctx,
                    c.third,
                    c.first,
                    file("third.txt"),
                    DryRun::No,
                )?;
                Ok(())
            },
            MoveCommitFile,
            "MoveCommitFile",
            |_| vec![],
        ),
        (
            "move",
            |ctx, c| {
                move_commit::commit_move(
                    ctx,
                    vec![c.second, c.third],
                    RelativeTo::Commit(c.first),
                    InsertSide::Below,
                    DryRun::No,
                )?;
                Ok(())
            },
            MoveCommit,
            "MoveCommit (2)",
            |_| vec![],
        ),
        (
            "reword",
            |ctx, c| {
                reword::commit_reword(ctx, c.third, "reworded".into(), DryRun::No)?;
                Ok(())
            },
            UpdateCommitMessage,
            "UpdateCommitMessage",
            |_| vec![],
        ),
        (
            "squash",
            |ctx, c| {
                squash::commit_squash(
                    ctx,
                    vec![c.third],
                    c.first,
                    MessageCombinationStrategy::KeepBoth,
                    DryRun::No,
                )?;
                Ok(())
            },
            SquashCommit,
            "SquashCommit",
            |_| vec![],
        ),
        (
            "uncommit",
            |ctx, c| {
                uncommit::commit_uncommit(ctx, vec![c.second, c.third], None, DryRun::No)?;
                Ok(())
            },
            UndoCommit,
            "UndoCommit (2)",
            |c| vec![Trailer::Sha(c.second), Trailer::Sha(c.third)],
        ),
        (
            "uncommit changes",
            |ctx, c| {
                uncommit::commit_uncommit_changes(
                    ctx,
                    c.third,
                    file("third.txt"),
                    None,
                    DryRun::No,
                )?;
                Ok(())
            },
            DiscardChanges,
            "DiscardChanges",
            |_| vec![],
        ),
        (
            "uncommit changes from commits",
            |ctx, c| {
                uncommit::commit_uncommit_changes_from_commits(
                    ctx,
                    vec![UncommitChangesSource {
                        commit_id: c.third,
                        changes: file("third.txt"),
                    }],
                    None,
                    DryRun::No,
                )?;
                Ok(())
            },
            DiscardChanges,
            "DiscardChanges",
            |_| vec![],
        ),
    ];

    for (name, operation, kind, title, trailers) in operations {
        let (mut ctx, _tmp, commits) = context_with_three_commits()?;
        operation(&mut ctx, &commits)?;

        let entries = ctx
            .snapshots_iter(None, Vec::new(), None)?
            .collect::<Result<Vec<_>, _>>()?;
        let [entry] = entries.as_slice() else {
            panic!("{name}: expected one undo step, got {}", entries.len());
        };
        let details = entry.details.as_ref().expect("an entry has details");
        let commit_trailers: Vec<_> = (details.trailers.iter())
            .filter(|trailer| matches!(trailer, Trailer::Sha(_)))
            .cloned()
            .collect();
        assert_eq!(
            (details.operation, details.title.as_str(), commit_trailers),
            (kind, title, trailers(&commits)),
            "{name}"
        );
    }
    Ok(())
}
