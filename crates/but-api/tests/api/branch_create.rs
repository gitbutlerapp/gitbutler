use but_rebase::graph_rebase::mutate::InsertSide;

use crate::support::{assert_workspace_ref, repo_with_feature_branch};

#[test]
fn branch_create_independent_respects_insertion_order() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;

    for (name, order, expected_index) in [
        ("first", Some(0), 0),
        ("second", Some(0), 0),
        ("middle", Some(1), 1),
        ("last", None, 4),
        ("clamped", Some(100), 5),
    ] {
        let new_ref = gix::refs::FullName::try_from(format!("refs/heads/{name}"))?;
        but_api::branch::branch_create(
            &mut ctx,
            Some(new_ref.clone()),
            but_api::branch::json::BranchCreatePlacement::Independent { order },
        )?;
        let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
        assert_eq!(
            ws.find_segment_owner_indexes_by_refname(new_ref.as_ref())
                .map(|(stack, _)| stack),
            Some(expected_index),
            "new independent branches must respect the requested insertion order"
        );
    }
    Ok(())
}

#[test]
fn branch_create_above_checked_out_ref_checks_out_new_ref_in_ad_hoc_workspace() -> anyhow::Result<()>
{
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let new_ref = gix::refs::FullName::try_from("refs/heads/top")?;
    let anchor_ref = gix::refs::FullName::try_from("refs/heads/main")?;

    let result = but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor_ref.clone()),
            side: InsertSide::Above,
        },
    )?;

    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("creating above checked-out branch checks out the new ref");
    assert_eq!(head_name, new_ref);
    assert_workspace_ref(&result.workspace, "refs/heads/top");

    let order_metadata = ctx.db.get_cache()?.meta()?;
    let order = order_metadata
        .branch_stack_order(anchor_ref.as_ref())
        .expect("ad-hoc branch creation above a local ref persists branch order");
    assert_eq!(order, vec![new_ref, anchor_ref]);

    Ok(())
}

#[test]
fn branch_create_below_checked_out_ref_keeps_head_in_ad_hoc_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let new_ref = gix::refs::FullName::try_from("refs/heads/bottom")?;
    let anchor_ref = gix::refs::FullName::try_from("refs/heads/main")?;

    let result = but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor_ref.clone()),
            side: InsertSide::Below,
        },
    )?;

    // Creating below the checked-out branch checks nothing out: HEAD stays on the anchor.
    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("HEAD remains symbolic after create-below");
    assert_eq!(head_name, anchor_ref);
    assert_workspace_ref(&result.workspace, "refs/heads/main");
    assert!(repo.try_find_reference(new_ref.as_ref())?.is_some());

    let order_metadata = ctx.db.get_cache()?.meta()?;
    let order = order_metadata
        .branch_stack_order(anchor_ref.as_ref())
        .expect("ad-hoc branch creation below a local ref persists branch order");
    assert_eq!(order, vec![anchor_ref, new_ref]);

    Ok(())
}

#[test]
fn failed_branch_creation_rolls_back_earlier_metadata_writes() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let new_ref: gix::refs::FullName = "refs/heads/bottom".try_into()?;
    let anchor_ref: gix::refs::FullName = "refs/heads/main".try_into()?;
    crate::support::workspace_graph(&ctx)?;
    let observer = but_db::DbHandle::new_in_directory(&ctx.project_data_dir)?;
    let metadata_before = observer.meta()?;
    let order_before = observer.branch_order().get_snapshot()?;
    let refresh = ctx.project_data_dir.join("REFRESH");
    if refresh.exists() {
        std::fs::remove_file(&refresh)?;
    }
    let sql = rusqlite::Connection::open(but_db::DbHandle::db_file_path(&ctx.project_data_dir))?;
    sql.execute_batch(
        "CREATE TRIGGER reject_new_branch BEFORE INSERT ON branch_metadata
         WHEN NEW.ref_name = CAST('refs/heads/bottom' AS BLOB)
         BEGIN SELECT RAISE(ABORT, 'reject branch metadata after saving order'); END;",
    )?;

    let error = but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor_ref),
            side: InsertSide::Below,
        },
    )
    .err()
    .expect("the final metadata write is rejected after branch order was saved");
    assert!(
        format!("{error:#}").contains("reject branch metadata after saving order"),
        "the regression reaches the late metadata write"
    );
    assert_eq!(
        observer.branch_order().get_snapshot()?,
        order_before,
        "an earlier successful metadata write rolls back with the failed operation"
    );
    assert_eq!(
        observer.meta()?,
        metadata_before,
        "failed creation leaves all reference metadata unchanged"
    );
    assert!(
        !refresh.exists(),
        "rolled-back metadata does not notify observers"
    );
    Ok(())
}
