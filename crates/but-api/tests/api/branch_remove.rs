use but_core::RefMetadata;
use but_testsupport::{CommandExt, git_at_dir};
use gitbutler_oplog::{OplogExt, RestoreKind};

use crate::support::{
    assert_workspace_ref, checkout_branch_in_linked_worktree, create_empty_branch_above,
    persist_default_target, repo_with_feature_branch, set_project_target_to_feature, write_file,
};

#[test]
fn branch_remove_rejects_non_local_reference() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let remote = gix::refs::FullName::try_from("refs/remotes/origin/main")?;

    let err = but_api::branch::branch_remove(&mut ctx, remote.clone())
        .expect_err("remote-tracking references cannot be deleted as local branches");
    assert!(
        err.to_string().contains("local branches"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(remote.as_ref())?.is_some(),
        "the rejected remote-tracking reference remains"
    );

    Ok(())
}

#[test]
fn branch_remove_rejects_gitbutler_technical_reference() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let technical = gix::refs::FullName::try_from("refs/heads/gitbutler/target")?;
    let head_id = repo.head_id()?.detach();
    repo.reference(
        technical.as_ref(),
        head_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "create technical branch for test",
    )?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::branch::branch_remove(&mut ctx, technical.clone())
        .expect_err("GitButler technical references cannot be deleted as user branches");
    assert!(
        err.to_string().contains("technical branch"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(technical.as_ref())?.is_some(),
        "the rejected technical reference remains"
    );

    Ok(())
}

#[test]
fn branch_remove_deletes_standalone_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;

    but_api::branch::branch_remove(&mut ctx, feature.clone())?;

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(feature.as_ref())?.is_none(),
        "the standalone branch reference was removed"
    );

    Ok(())
}

#[test]
fn removing_a_branch_outside_the_workspace_can_be_undone_and_redone() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    persist_default_target(&repo)?;
    // `outside` has a commit that nothing in the workspace reaches.
    git_at_dir(tmp.path())
        .args(["switch", "-c", "outside", "feature"])
        .run();
    write_file(tmp.path(), "outside.txt", "outside\n")?;
    git_at_dir(tmp.path()).args(["add", "outside.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "outside"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "main"]).run();
    let outside_tip = repo.rev_parse_single("outside")?.detach();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let outside = gix::refs::FullName::try_from("refs/heads/outside")?;
    let outside_target = |ctx: &but_ctx::Context| -> anyhow::Result<Option<gix::ObjectId>> {
        Ok(ctx
            .repo
            .get()?
            .try_find_reference(outside.as_ref())?
            .map(|reference| reference.id().detach()))
    };

    but_api::branch::branch_remove(&mut ctx, outside.clone())?;
    assert_eq!(outside_target(&ctx)?, None, "the branch was removed");
    let remove_snapshot = ctx
        .oplog_head()?
        .expect("the removal records an undo point");
    // Let Git drop what only the deleted branch reached, as `git gc` does once its grace
    // period is over.
    git_at_dir(tmp.path())
        .args(["reflog", "expire", "--expire=now", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["prune", "--expire=now"]).run();
    assert!(
        !ctx.repo.get()?.has_object(outside_tip),
        "nothing keeps the commit object itself"
    );

    let mut guard = ctx.exclusive_worktree_access();
    let redo_snapshot = ctx.restore_snapshot(
        remove_snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        outside_target(&ctx)?,
        Some(outside_tip),
        "undo brings the branch back"
    );
    {
        let repo = ctx.repo.get()?;
        let outside_tree = repo.find_commit(outside_tip)?.tree()?;
        assert!(
            outside_tree.lookup_entry_by_path("outside.txt")?.is_some(),
            "undo brings back the commit and its files, even after Git pruned them"
        );
    }

    ctx.restore_snapshot(
        redo_snapshot,
        RestoreKind::RestoreFromSnapshotViaRedo,
        guard.write_permission(),
    )?;
    assert_eq!(outside_target(&ctx)?, None, "redo removes it again");
    Ok(())
}

#[test]
fn branch_remove_deletes_middle_empty_branch_and_keeps_head() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // Stack two empty branches above the checked-out `main`: [tip, middle, main].
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;

    but_api::branch::branch_remove(&mut ctx, middle.clone())?;

    let repo = ctx.repo.get()?;
    // Removing a branch that isn't checked out leaves HEAD on the tip.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);
    assert!(repo.try_find_reference(middle.as_ref())?.is_none());
    // The order relinks the tip straight onto the base.
    let order = ctx
        .meta()?
        .branch_stack_order(tip.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(
        order,
        vec![
            tip,
            main,
            gix::refs::FullName::try_from("refs/heads/feature")?
        ]
    );

    Ok(())
}

#[test]
fn branch_remove_checked_out_empty_tip_moves_head_to_ref_below() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // [tip, middle, main] with HEAD on the empty `tip`.
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;

    let result = but_api::branch::branch_remove(&mut ctx, tip.clone())?;

    let repo = ctx.repo.get()?;
    // HEAD lands on the reference that was directly underneath the removed tip.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), middle);
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());
    assert_workspace_ref(&result.workspace, "refs/heads/middle");

    let order = ctx
        .meta()?
        .branch_stack_order(middle.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(
        order,
        vec![
            middle,
            main,
            gix::refs::FullName::try_from("refs/heads/feature")?
        ]
    );

    Ok(())
}

#[test]
fn branch_remove_checked_out_empty_tip_on_the_target_moves_head_to_the_target() -> anyhow::Result<()>
{
    let (repo, _tmp) = repo_with_feature_branch()?;
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // As `git checkout -b tip` from the target: the target is the workspace's base, not a
    // segment under `tip`.
    create_empty_branch_above(&mut ctx, &tip, &main)?;
    assert_eq!(ctx.repo.get()?.head_name()?.expect("HEAD is symbolic"), tip);

    but_api::branch::branch_remove(&mut ctx, tip.clone())?;

    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic"),
        main,
        "HEAD is back on the target it was made from"
    );
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());

    Ok(())
}

#[test]
fn branch_remove_rejects_checked_out_branch_with_commits() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;

    let err = but_api::branch::branch_remove(&mut ctx, main.clone())
        .expect_err("cannot delete the checked-out branch that owns commits");
    assert!(
        err.to_string().contains("contains commits"),
        "unexpected error: {err}"
    );

    // Nothing was removed.
    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(main.as_ref())?.is_some());
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), main);

    Ok(())
}

#[test]
fn branch_remove_refuses_when_checked_out_in_another_worktree() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // Keep `middle` in the primary worktree's projection while checking it out in a linked
    // worktree: [tip, middle, main], with the primary worktree on `tip`.
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;
    let _worktree = checkout_branch_in_linked_worktree(tmp.path(), "middle")?;

    let err = but_api::branch::branch_remove(&mut ctx, middle.clone())
        .expect_err("cannot remove a branch checked out in another worktree");
    assert!(
        err.to_string().contains("checked out"),
        "unexpected error: {err}"
    );

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(middle.as_ref())?.is_some(),
        "the checked-out branch must remain"
    );
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);

    Ok(())
}

#[test]
fn branch_remove_of_the_second_to_last_lane_leaves_the_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let lane = gix::refs::FullName::try_from("refs/heads/lane")?;
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;
    but_api::branch::branch_create(
        &mut ctx,
        Some(lane.clone()),
        but_api::branch::json::BranchCreatePlacement::Independent { order: None },
    )?;
    assert_eq!(
        ctx.repo.get()?.head_name()?.expect("HEAD is symbolic"),
        but_core::WORKSPACE_REF_NAME,
        "a second lane needs the workspace"
    );

    let result = but_api::branch::branch_remove(&mut ctx, lane.clone())?;

    let repo = ctx.repo.get()?;
    // With one lane left the workspace serves no purpose, so `HEAD` moves onto it, as unapply does.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), feature);
    assert!(
        repo.try_find_reference(but_core::WORKSPACE_REF_NAME)?
            .is_none(),
        "the workspace reference is gone"
    );
    assert!(repo.try_find_reference(lane.as_ref())?.is_none());
    assert_workspace_ref(&result.workspace, "refs/heads/feature");

    drop(repo);
    crate::support::undo(&mut ctx)?;
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic"),
        but_core::WORKSPACE_REF_NAME,
        "undo brings the workspace back"
    );
    assert!(repo.try_find_reference(lane.as_ref())?.is_some());
    Ok(())
}

/// A workspace with `feature` and `sibling` applied as lanes, and an empty `top` above `feature`.
fn two_lanes_with_top() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    let (repo, tmp) = crate::support::writable_scenario("checkout-head-info");
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::branch::apply_only(&mut ctx, full("refs/heads/feature").as_ref())?;
    but_api::branch::apply_only(&mut ctx, full("refs/heads/sibling").as_ref())?;
    create_empty_branch_above(
        &mut ctx,
        &full("refs/heads/top"),
        &full("refs/heads/feature"),
    )?;
    Ok((ctx, tmp))
}

fn full(name: &str) -> gix::refs::FullName {
    name.try_into().expect("valid ref name")
}

fn commit_new_file_to(
    ctx: &mut but_ctx::Context,
    branch: &str,
    path: &str,
) -> anyhow::Result<gix::ObjectId> {
    let workdir = ctx.repo.get()?.workdir().expect("non-bare").to_owned();
    write_file(&workdir, path, path)?;
    let mut guard = ctx.exclusive_worktree_access();
    let outcome = but_api::commit::create::commit_create(
        ctx,
        but_rebase::graph_rebase::mutate::RelativeTo::Reference(full(branch)),
        but_rebase::graph_rebase::mutate::InsertSide::Below,
        vec![but_core::DiffSpec {
            previous_path: None,
            path: path.into(),
            hunk_headers: vec![],
        }],
        but_api::commit::json::ChangesSource::Head,
        path.into(),
        but_core::DryRun::No,
        guard.write_permission(),
    )?;
    Ok(outcome.new_commit.expect("the change was committed"))
}

fn ref_id(ctx: &but_ctx::Context, name: &str) -> anyhow::Result<Option<gix::ObjectId>> {
    Ok(ctx
        .repo
        .get()?
        .try_find_reference(name)?
        .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
        .transpose()?)
}

fn workspace_tree_and_parents(
    ctx: &but_ctx::Context,
) -> anyhow::Result<(gix::ObjectId, Vec<gix::ObjectId>)> {
    let repo = ctx.repo.get()?;
    let commit = repo
        .find_reference(but_core::WORKSPACE_REF_NAME)?
        .peel_to_commit()?;
    Ok((
        commit.tree_id()?.detach(),
        commit.parent_ids().map(|id| id.detach()).collect(),
    ))
}

#[test]
fn removing_the_top_branch_with_commits_leaves_them_to_the_branch_below() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = two_lanes_with_top()?;
    let top_tip = commit_new_file_to(&mut ctx, "refs/heads/top", "top.txt")?;
    let (tree, parents) = workspace_tree_and_parents(&ctx)?;

    but_api::branch::branch_remove(&mut ctx, full("refs/heads/top"))?;

    assert_eq!(ref_id(&ctx, "refs/heads/top")?, None, "the branch is gone");
    assert_eq!(
        ref_id(&ctx, "refs/heads/feature")?,
        Some(top_tip),
        "its commit now belongs to the branch below"
    );
    assert_eq!(
        ctx.repo.get()?.head_name()?.expect("HEAD is symbolic"),
        but_core::WORKSPACE_REF_NAME,
    );
    assert_eq!(
        workspace_tree_and_parents(&ctx)?,
        (tree, parents),
        "the workspace keeps its files and lanes"
    );
    Ok(())
}

#[test]
fn removing_the_only_branch_of_a_stack_with_commits_is_refused() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = two_lanes_with_top()?;
    let sibling_tip = ref_id(&ctx, "refs/heads/sibling")?;
    let workspace = workspace_tree_and_parents(&ctx)?;

    let err = but_api::branch::branch_remove(&mut ctx, full("refs/heads/sibling"))
        .expect_err("its commits would be left without a branch");
    assert!(
        err.to_string().contains("last named segment"),
        "unexpected error: {err:#}"
    );

    assert_eq!(ref_id(&ctx, "refs/heads/sibling")?, sibling_tip);
    assert_eq!(workspace_tree_and_parents(&ctx)?, workspace);
    Ok(())
}

#[test]
fn removing_a_branch_removes_its_git_configuration() -> anyhow::Result<()> {
    let (mut ctx, tmp) = two_lanes_with_top()?;
    git_at_dir(tmp.path())
        .args(["config", "branch.top.description", "why it's there"])
        .run();

    but_api::branch::branch_remove(&mut ctx, full("refs/heads/top"))?;

    let config = std::fs::read_to_string(tmp.path().join(".git/config"))?;
    assert!(
        !config.contains("[branch \"top\"]"),
        "a branch made later under the name would take on its description:\n{config}"
    );
    Ok(())
}
