use but_core::DryRun;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_oplog::OplogExt as _;

fn loose_object_count(repo: &gix::Repository) -> anyhow::Result<String> {
    let output = but_testsupport::git(repo)
        .args(["count-objects", "-v"])
        .output()?;
    anyhow::ensure!(output.status.success(), "git count-objects failed");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn repeated_move_returns_current_state_without_another_snapshot() -> anyhow::Result<()> {
    let (repo, _tmp) =
        crate::support::writable_scenario("../../../../but/tests/fixtures/scenario/two-stacks");
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let (source, target): (gix::ObjectId, gix::refs::FullName) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/heads/A")?.detach(),
            "refs/heads/B".try_into()?,
        )
    };

    let first = but_api::commit::move_commit::commit_move(
        &mut ctx,
        vec![source],
        RelativeTo::Reference(target.clone()),
        InsertSide::Below,
        DryRun::No,
    )?;
    let moved = first
        .workspace
        .replaced_commits
        .get(&source)
        .copied()
        .unwrap_or(source);
    let refs_before_repeat = but_testsupport::visualize_commit_graph_all(&*ctx.repo.get()?)?;
    let oplog_head = ctx.oplog_head()?;
    let oplog_file = std::fs::read(ctx.project_data_dir().join("operations-log.toml"))?;
    let objects_before_repeat = loose_object_count(&*ctx.repo.get()?)?;

    let repeated = but_api::commit::move_commit::commit_move(
        &mut ctx,
        vec![moved],
        RelativeTo::Reference(target),
        InsertSide::Below,
        DryRun::No,
    )?;

    assert!(
        repeated.workspace.replaced_commits.is_empty(),
        "the repeated result should describe the unchanged current workspace"
    );
    assert_eq!(
        but_testsupport::visualize_commit_graph_all(&*ctx.repo.get()?)?,
        refs_before_repeat,
        "the repeated API call should leave every reference unchanged"
    );
    assert_eq!(
        ctx.oplog_head()?,
        oplog_head,
        "the no-op should leave the oplog head unchanged"
    );
    assert_eq!(
        std::fs::read(ctx.project_data_dir().join("operations-log.toml"))?,
        oplog_file,
        "the no-op should not change oplog state"
    );
    assert_eq!(
        loose_object_count(&*ctx.repo.get()?)?,
        objects_before_repeat,
        "the no-op should not write snapshot objects"
    );
    let moves = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .filter_map(Result::ok)
        .filter(|snapshot| {
            snapshot.details.as_ref().is_some_and(|details| {
                details.operation == but_oplog::legacy::OperationKind::MoveCommit
            })
        })
        .count();
    assert_eq!(moves, 1, "only the first API move should be recorded");
    Ok(())
}

#[test]
fn repeated_move_in_ad_hoc_workspace_is_unchanged() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::repo_with_feature_branch()?;
    crate::support::set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let (source, target): (gix::ObjectId, gix::refs::FullName) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/heads/main")?.detach(),
            "refs/heads/feature".try_into()?,
        )
    };

    let first = but_api::commit::move_commit::commit_move(
        &mut ctx,
        vec![source],
        RelativeTo::Reference(target.clone()),
        InsertSide::Below,
        DryRun::No,
    )?;
    let moved = first
        .workspace
        .replaced_commits
        .get(&source)
        .copied()
        .unwrap_or(source);
    let refs_before_repeat = but_testsupport::visualize_commit_graph_all(&*ctx.repo.get()?)?;
    let oplog_head = ctx.oplog_head()?;
    let oplog_file = std::fs::read(ctx.project_data_dir().join("operations-log.toml"))?;
    let objects_before_repeat = loose_object_count(&*ctx.repo.get()?)?;

    let repeated = but_api::commit::move_commit::commit_move(
        &mut ctx,
        vec![moved],
        RelativeTo::Reference(target),
        InsertSide::Below,
        DryRun::No,
    )?;

    assert!(
        repeated.workspace.replaced_commits.is_empty(),
        "a repeated move must not report replacement commit mappings"
    );
    assert_eq!(
        but_testsupport::visualize_commit_graph_all(&*ctx.repo.get()?)?,
        refs_before_repeat,
        "a repeated move must leave refs and commit graph unchanged"
    );
    assert_eq!(
        ctx.oplog_head()?,
        oplog_head,
        "a repeated move must leave the oplog head unchanged"
    );
    assert_eq!(
        std::fs::read(ctx.project_data_dir().join("operations-log.toml"))?,
        oplog_file,
        "a repeated move must leave the oplog file unchanged"
    );
    assert_eq!(
        loose_object_count(&*ctx.repo.get()?)?,
        objects_before_repeat,
        "a repeated move must not create loose objects"
    );
    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None)?
            .filter_map(Result::ok)
            .filter(|snapshot| snapshot.details.as_ref().is_some_and(|details| {
                details.operation == but_oplog::legacy::OperationKind::MoveCommit
            }))
            .count(),
        1,
        "only the real move should create a MOVE snapshot"
    );
    Ok(())
}
