//! Committing a linked worktree's changes moves the branch that worktree has checked out,
//! which isn't part of the main checkout. Undo has to move it back all the same.

use but_api::commit::json::ChangesSource;
use but_core::{DiffSpec, DryRun, sync::RepoShared};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_testsupport::{CommandExt, git_at_dir};
use gitbutler_oplog::{OplogExt, RestoreKind};

use crate::support::{
    checkout_branch_in_linked_worktree, persist_default_target, repo_with_feature_branch,
    write_file,
};

/// A context whose project has `wt` as an active linked worktree on `feature`, with
/// `wt.txt` uncommitted in it. `prepare_worktree` runs in the worktree beforehand.
fn ctx_with_linked_change(
    prepare_worktree: impl FnOnce(&std::path::Path),
) -> anyhow::Result<(but_ctx::Context, tempfile::TempDir, tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    persist_default_target(&repo)?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    prepare_worktree(&linked.path().join("wt"));
    write_file(&linked.path().join("wt"), "wt.txt", "worktree change\n")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    // Adoption archives the worktrees that exist when the flag is first read.
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;
    Ok((ctx, tmp, linked))
}

fn wt_txt() -> Vec<DiffSpec> {
    vec![DiffSpec {
        previous_path: None,
        path: "wt.txt".into(),
        hunk_headers: vec![],
    }]
}

fn feature_tip(ctx: &but_ctx::Context) -> anyhow::Result<gix::ObjectId> {
    Ok(ctx.repo.get()?.rev_parse_single("feature")?.detach())
}

fn linked_worktree_change_paths(
    ctx: &but_ctx::Context,
    perm: &RepoShared,
) -> anyhow::Result<Vec<String>> {
    let changes = but_api::diff::changes_in_worktree_with_perm(
        ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        perm,
    )?;
    Ok(changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| change.path.to_string())
        .collect())
}

#[test]
fn committing_from_a_linked_worktree_can_be_undone_and_redone() -> anyhow::Result<()> {
    let (mut ctx, _tmp, _linked) = ctx_with_linked_change(|_| {})?;
    let base = feature_tip(&ctx)?;

    let mut guard = ctx.exclusive_worktree_access();
    let outcome = but_api::commit::create::commit_create(
        &mut ctx,
        RelativeTo::Reference("refs/heads/feature".try_into()?),
        InsertSide::Below,
        wt_txt(),
        ChangesSource::Worktree("wt".into()),
        "worktree commit".into(),
        DryRun::No,
        guard.write_permission(),
    )?;
    let new_commit = outcome.new_commit.expect("the change was committed");
    assert_eq!(feature_tip(&ctx)?, new_commit);
    assert!(
        linked_worktree_change_paths(&ctx, guard.read_permission())?.is_empty(),
        "the committed change left the worktree"
    );
    let commit_snapshot = ctx.oplog_head()?.expect("the commit records an undo point");

    let redo_snapshot = ctx.restore_snapshot(
        commit_snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        feature_tip(&ctx)?,
        base,
        "undo moves the worktree's branch back"
    );
    assert_eq!(
        linked_worktree_change_paths(&ctx, guard.read_permission())?,
        ["wt.txt"],
        "the change is uncommitted in the worktree again"
    );

    ctx.restore_snapshot(
        redo_snapshot,
        RestoreKind::RestoreFromSnapshotViaRedo,
        guard.write_permission(),
    )?;
    assert_eq!(feature_tip(&ctx)?, new_commit, "redo commits it again");
    assert!(
        linked_worktree_change_paths(&ctx, guard.read_permission())?.is_empty(),
        "the worktree is clean after redo"
    );
    Ok(())
}

#[test]
fn amending_from_a_linked_worktree_can_be_undone() -> anyhow::Result<()> {
    // Amend a commit only the worktree's branch has, so the main checkout stays put.
    let (mut ctx, _tmp, _linked) = ctx_with_linked_change(|wt| {
        git_at_dir(wt)
            .args(["commit", "--allow-empty", "-m", "worktree commit"])
            .run();
    })?;
    let worktree_commit = feature_tip(&ctx)?;

    but_api::commit::amend::commit_amend(
        &mut ctx,
        worktree_commit,
        wt_txt(),
        ChangesSource::Worktree("wt".into()),
        DryRun::No,
    )?;
    assert_ne!(feature_tip(&ctx)?, worktree_commit, "the amend rewrote it");
    let amend_snapshot = ctx.oplog_head()?.expect("the amend records an undo point");

    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        amend_snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        feature_tip(&ctx)?,
        worktree_commit,
        "undo moves the worktree's branch back"
    );
    assert_eq!(
        linked_worktree_change_paths(&ctx, guard.read_permission())?,
        ["wt.txt"],
        "the change is uncommitted in the worktree again"
    );
    Ok(())
}
