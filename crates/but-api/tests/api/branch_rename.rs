use but_core::RefMetadata;
use gix::bstr::ByteSlice;

use crate::support::{
    assert_workspace_ref, create_empty_branch_above, persist_default_target,
    repo_with_feature_branch, workspace_graph, writable_scenario,
};

#[test]
fn branch_rename_middle_branch_keeps_head_and_order() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let middle = gix::refs::FullName::try_from("refs/heads/middle")?;
    let renamed = gix::refs::FullName::try_from("refs/heads/renamed")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    // [tip, middle, main] with HEAD on the empty tip.
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;

    but_api::branch::branch_rename(&mut ctx, middle.clone(), "renamed".into())?;

    let repo = ctx.repo.get()?;
    // Renaming a branch that isn't checked out leaves HEAD on the tip.
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic").as_ref(),
        tip.as_ref()
    );
    assert!(repo.try_find_reference(middle.as_ref())?.is_none());
    assert!(repo.try_find_reference(renamed.as_ref())?.is_some());
    // The order keeps the branch in place under the new name.
    let order = ctx
        .meta()?
        .branch_stack_order(tip.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(order, vec![tip, renamed, main]);

    Ok(())
}

#[test]
fn branch_rename_checked_out_branch_moves_head_to_new_name() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;
    let renamed = gix::refs::FullName::try_from("refs/heads/renamed-tip")?;

    // [tip, main] with HEAD on the empty tip.
    create_empty_branch_above(&mut ctx, &tip, &main)?;

    let result = but_api::branch::branch_rename(&mut ctx, tip.clone(), "renamed-tip".into())?;

    let repo = ctx.repo.get()?;
    // Renaming the checked-out branch carries HEAD over to the new name.
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic").as_ref(),
        renamed.as_ref()
    );
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());
    assert_eq!(result.new_ref.as_ref(), renamed.as_ref());
    assert_workspace_ref(&result.workspace, "refs/heads/renamed-tip");

    let order = ctx
        .meta()?
        .branch_stack_order(renamed.as_ref())?
        .expect("branch order still persisted");
    assert_eq!(order, vec![renamed, main]);

    Ok(())
}

#[test]
fn branch_rename_rejects_a_name_that_already_exists() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    create_empty_branch_above(&mut ctx, &tip, &main)?;

    // `feature` already exists in the fixture.
    let err = but_api::branch::branch_rename(&mut ctx, tip.clone(), "feature".into())
        .expect_err("cannot rename onto an existing branch");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    // Nothing changed: the original branch is intact and still checked out.
    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(tip.as_ref())?.is_some());
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic").as_ref(),
        tip.as_ref()
    );

    Ok(())
}

#[test]
fn branch_rename_normalizes_the_requested_name() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let tip = gix::refs::FullName::try_from("refs/heads/tip")?;

    create_empty_branch_above(&mut ctx, &tip, &main)?;

    // A raw name with spaces is not a valid ref on its own, so the rename must normalize it.
    assert!(
        gix::refs::FullName::try_from("refs/heads/My Fancy Branch").is_err(),
        "the raw name is not a valid ref, so normalization is required"
    );
    let result = but_api::branch::branch_rename(&mut ctx, tip.clone(), "My Fancy Branch".into())?;

    // The resulting ref matches what the non-legacy normalizer produces.
    let expected_short = but_core::branch::normalize_short_name("My Fancy Branch")?;
    let expected = gix::refs::Category::LocalBranch.to_full_name(expected_short.as_bstr())?;
    assert_eq!(result.new_ref.as_ref(), expected.as_ref());

    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());
    assert!(repo.try_find_reference(expected.as_ref())?.is_some());

    Ok(())
}

#[test]
fn branch_rename_keeps_a_managed_stack_branch_applied() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("checkout-head-info");
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;

    // Enter a managed workspace with `feature` applied as a stack.
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;
    let before = workspace_graph(&ctx)?;
    assert!(
        before.contains("feature"),
        "feature should be applied before the rename:\n{before}"
    );

    let result =
        but_api::branch::branch_rename(&mut ctx, feature.clone(), "renamed-feature".into())?;
    let renamed = gix::refs::FullName::try_from("refs/heads/renamed-feature")?;
    assert_eq!(result.new_ref.as_ref(), renamed.as_ref());

    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(feature.as_ref())?.is_none());
    assert!(repo.try_find_reference(renamed.as_ref())?.is_some());

    // The renamed branch stays part of the managed workspace: it was renamed in place within its
    // stack rather than torn out into a fresh, unapplied standalone stack.
    let after = workspace_graph(&ctx)?;
    assert!(
        after.contains("renamed-feature"),
        "renamed branch should still be applied in the managed workspace:\n{after}"
    );

    Ok(())
}

#[test]
fn branch_rename_leaves_the_remote_tracking_ref_untouched() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    // Simulate `feature` having been pushed by creating its remote-tracking ref.
    let feature_tip = repo
        .find_reference("refs/heads/feature")?
        .peel_to_id()?
        .detach();
    repo.reference(
        "refs/remotes/origin/feature",
        feature_tip,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "simulate pushed branch",
    )?;

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;

    but_api::branch::branch_rename(&mut ctx, feature.clone(), "renamed-feature".into())?;

    let repo = ctx.repo.get()?;
    // The local branch is renamed, but the remote-tracking ref is left in place (same as legacy).
    assert!(repo.try_find_reference(feature.as_ref())?.is_none());
    assert!(
        repo.try_find_reference("refs/heads/renamed-feature")?
            .is_some()
    );
    assert!(
        repo.try_find_reference("refs/remotes/origin/feature")?
            .is_some(),
        "the remote-tracking ref must be left untouched by a local rename"
    );

    Ok(())
}
