use but_core::RefMetadata;
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

    let order = ctx
        .meta()?
        .branch_stack_order(anchor_ref.as_ref())?
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

    let order = ctx
        .meta()?
        .branch_stack_order(anchor_ref.as_ref())?
        .expect("ad-hoc branch creation below a local ref persists branch order");
    assert_eq!(order, vec![anchor_ref, new_ref]);

    Ok(())
}
