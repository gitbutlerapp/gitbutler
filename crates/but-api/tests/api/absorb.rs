use but_hunk_assignment::AbsorptionTarget;
use but_testsupport::{CommandExt, git_at_dir, open_repo};
use gitbutler_oplog::{OplogExt as _, entry::OperationKind};

use crate::support::{undo, write_file};

/// `feature` is checked out with `first` and `second` above the target `main`, and the
/// worktree changes a line each of them added.
fn context_with_changes_to_two_commits()
-> anyhow::Result<(but_ctx::Context, tempfile::TempDir, gix::ObjectId)> {
    let tmp = tempfile::tempdir()?;
    let git = |args: &[&str]| git_at_dir(tmp.path()).args(args).run();
    let commit_file = |name: &str| -> anyhow::Result<()> {
        write_file(tmp.path(), name, "one\ntwo\nthree\n")?;
        git(&["add", name]);
        git(&["commit", "-m", name]);
        Ok(())
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "GitButler"]);
    git(&["config", "user.email", "gitbutler@example.com"]);
    commit_file("base.txt")?;
    git(&["remote", "add", "origin", "../origin"]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["checkout", "-b", "feature"]);
    commit_file("first.txt")?;
    commit_file("second.txt")?;
    write_file(tmp.path(), "first.txt", "one\nTWO\nthree\n")?;
    write_file(tmp.path(), "second.txt", "one\ntwo\nTHREE\n")?;

    let repo = open_repo(tmp.path())?;
    let tip = repo.rev_parse_single("feature")?.detach();
    but_core::ref_metadata::ProjectMeta {
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, tmp, tip))
}

fn blob(ctx: &but_ctx::Context, rev: &str) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    Ok(repo
        .rev_parse_single(rev)?
        .object()?
        .data
        .to_vec()
        .try_into()?)
}

#[test]
fn absorbing_into_two_commits_is_one_undo_step() -> anyhow::Result<()> {
    let (mut ctx, tmp, tip) = context_with_changes_to_two_commits()?;

    let plan = but_api::legacy::absorb::absorption_plan(&mut ctx, AbsorptionTarget::default())?;
    let rejected = but_api::legacy::absorb::absorb(&mut ctx, plan)?;

    assert_eq!(rejected, 0);
    assert_eq!(blob(&ctx, "feature~1:first.txt")?, "one\nTWO\nthree\n");
    assert_eq!(blob(&ctx, "feature:second.txt")?, "one\ntwo\nTHREE\n");
    let kinds: Vec<_> = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .map(|entry| entry.map(|entry| entry.details.map(|details| details.operation)))
        .collect::<Result<_, _>>()?;
    assert_eq!(kinds, [Some(OperationKind::Absorb)]);

    undo(&mut ctx)?;

    let repo = ctx.repo.get()?;
    assert_eq!(repo.rev_parse_single("feature")?.detach(), tip);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("first.txt"))?,
        "one\nTWO\nthree\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("second.txt"))?,
        "one\ntwo\nTHREE\n"
    );
    Ok(())
}
