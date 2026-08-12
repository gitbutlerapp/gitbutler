use but_core::RefMetadata;
use gix::bstr::ByteSlice;

use crate::support::{
    assert_workspace_ref, checkout_branch_in_linked_worktree, create_empty_branch_above,
    persist_default_target, repo_with_feature_branch, workspace_graph, writable_scenario,
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
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);
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
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), renamed);
    assert!(repo.try_find_reference(tip.as_ref())?.is_none());
    assert_eq!(result.new_ref, renamed);
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
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), tip);

    Ok(())
}

#[test]
fn branch_rename_same_name_rejects_a_missing_source() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let missing = gix::refs::FullName::try_from("refs/heads/missing")?;

    let err = but_api::branch::branch_rename(&mut ctx, missing, "missing".into())
        .expect_err("a same-name rename still requires the source branch to exist");
    assert!(
        err.to_string().contains("does not exist"),
        "unexpected error: {err}"
    );

    Ok(())
}

#[test]
fn branch_rename_refuses_when_checked_out_in_another_worktree() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    // Check `feature` out in a second, linked worktree; the main worktree stays on `main`.
    let _worktree = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let renamed = gix::refs::FullName::try_from("refs/heads/renamed-feature")?;

    let err = but_api::branch::branch_rename(&mut ctx, feature.clone(), "renamed-feature".into())
        .expect_err("cannot rename a branch checked out in another worktree");
    assert!(
        err.to_string().contains("checked out elsewhere"),
        "unexpected error: {err}"
    );

    // The rename must be all-or-nothing: the old ref is untouched and the new ref was never created,
    // so we don't leave a partially-applied rename behind.
    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(feature.as_ref())?.is_some(),
        "the original branch must remain"
    );
    assert!(
        repo.try_find_reference(renamed.as_ref())?.is_none(),
        "the new branch must not have been created"
    );

    Ok(())
}

#[test]
fn branch_rename_rejects_a_destination_that_exists_only_in_metadata() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let other = gix::refs::FullName::try_from("refs/heads/other")?;

    // Give `other` workspace metadata (a branch-order entry), then simulate it being deleted
    // externally by dropping only its git ref. This leaves the name free at the git level but still
    // occupied in metadata — the state where `meta.rename()` would reject *after* refs moved.
    create_empty_branch_above(&mut ctx, &other, &main)?;
    but_api::branch::branch_checkout(&mut ctx, main.clone())?;
    ctx.repo.get()?.find_reference(other.as_ref())?.delete()?;

    // Precondition for the test: the ref is gone but the metadata still knows the name.
    assert!(
        ctx.repo
            .get()?
            .try_find_reference(other.as_ref())?
            .is_none(),
        "the git ref must be gone"
    );
    assert!(
        ctx.meta()?.branch_opt(other.as_ref())?.is_some()
            || ctx.meta()?.branch_stack_order(other.as_ref())?.is_some(),
        "the name must still be occupied in metadata"
    );

    // The rename must bail on the metadata conflict *before* touching any refs.
    let err = but_api::branch::branch_rename(&mut ctx, feature.clone(), "other".into())
        .expect_err("destination is still occupied in metadata");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    // No partial rename: the source is intact and the destination ref was not (re)created.
    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(feature.as_ref())?.is_some(),
        "the source branch must remain"
    );
    assert!(
        repo.try_find_reference(other.as_ref())?.is_none(),
        "no destination ref must be created when the metadata rename can't proceed"
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
    assert_eq!(result.new_ref, expected);

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
    assert_eq!(result.new_ref, renamed);

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
fn branch_rename_then_new_pr_updates_metadata_under_the_new_name() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("checkout-head-info");
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    let renamed = gix::refs::FullName::try_from("refs/heads/renamed-feature")?;

    // Apply `feature` and associate it with PR #42, as if it had been published.
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;
    {
        let mut meta = ctx.meta()?;
        let mut branch = meta.branch(feature.as_ref())?;
        branch.review.pull_request = Some(42);
        meta.set_branch(&branch)?;
    }

    // Rename the published branch. The PR association follows the rename in place (it's stored on the
    // head, keyed by position, not derived from the name), without any push.
    but_api::branch::branch_rename(&mut ctx, feature.clone(), "renamed-feature".into())?;
    assert_eq!(
        ctx.meta()?.branch(renamed.as_ref())?.review.pull_request,
        Some(42),
        "the existing PR number must travel with the rename"
    );

    // Now create a *new* PR for the renamed branch. The writeback keys off the current name, so it
    // finds the renamed head and overwrites the carried-over number instead of losing or duplicating
    // it.
    {
        let mut meta = ctx.meta()?;
        let mut branch = meta.branch(renamed.as_ref())?;
        branch.review.pull_request = Some(99);
        meta.set_branch(&branch)?;
    }

    let meta = ctx.meta()?;
    assert_eq!(
        meta.branch(renamed.as_ref())?.review.pull_request,
        Some(99),
        "creating a new PR after the rename must update the metadata under the new name"
    );
    assert!(
        meta.branch_opt(feature.as_ref())?.is_none(),
        "no branch metadata should linger under the old name"
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

#[test]
fn branch_rename_rejects_non_local_refs() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    // The fixture ships a `refs/remotes/origin/main` remote-tracking ref; renaming it would create a
    // local branch and delete the remote-tracking ref, so it must be rejected outright.
    let remote = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    let err = but_api::branch::branch_rename(&mut ctx, remote.clone(), "hijacked".into())
        .expect_err("must refuse to rename a non-local ref");
    assert!(
        err.to_string().contains("Can only rename local branches"),
        "unexpected error: {err}"
    );

    // Nothing was mutated: the remote-tracking ref is intact and no local branch was created.
    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference(remote.as_ref())?.is_some());
    assert!(repo.try_find_reference("refs/heads/hijacked")?.is_none());

    Ok(())
}

#[test]
fn branch_rename_into_a_directory_prefix() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let foo = gix::refs::FullName::try_from("refs/heads/foo")?;
    let foo_bar = gix::refs::FullName::try_from("refs/heads/foo/bar")?;

    // [foo, main] with HEAD on the empty foo.
    create_empty_branch_above(&mut ctx, &foo, &main)?;

    // `foo` and `foo/bar` can't coexist as refs, so this must not fail with "Could not create
    // branch": the rename is done atomically instead.
    let result = but_api::branch::branch_rename(&mut ctx, foo.clone(), "foo/bar".into())?;
    assert_eq!(result.new_ref, foo_bar);

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(foo.as_ref())?.is_none(),
        "the old ref must be gone"
    );
    assert!(
        repo.try_find_reference(foo_bar.as_ref())?.is_some(),
        "the prefixed ref must exist"
    );
    // HEAD followed the rename onto the new, prefixed name.
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), foo_bar);

    Ok(())
}

#[test]
fn branch_rename_out_of_a_directory_prefix() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let foo_bar = gix::refs::FullName::try_from("refs/heads/foo/bar")?;
    let foo = gix::refs::FullName::try_from("refs/heads/foo")?;

    // [foo/bar, main] with HEAD on the empty foo/bar.
    create_empty_branch_above(&mut ctx, &foo_bar, &main)?;

    // Collapsing `foo/bar` back down to `foo` reuses the now-empty `foo/` directory as a ref file,
    // which the atomic rename must handle.
    let result = but_api::branch::branch_rename(&mut ctx, foo_bar.clone(), "foo".into())?;
    assert_eq!(result.new_ref, foo);

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference(foo_bar.as_ref())?.is_none(),
        "the old ref must be gone"
    );
    assert!(
        repo.try_find_reference(foo.as_ref())?.is_some(),
        "the collapsed ref must exist"
    );
    assert_eq!(repo.head_name()?.expect("HEAD is symbolic"), foo);

    Ok(())
}

#[test]
fn prefix_rename_restores_source_when_destination_is_locked() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main = gix::refs::FullName::try_from("refs/heads/main")?;
    let source = gix::refs::FullName::try_from("refs/heads/foo/bar")?;
    let destination = gix::refs::FullName::try_from("refs/heads/foo")?;

    create_empty_branch_above(&mut ctx, &source, &main)?;

    let source_id = ctx
        .repo
        .get()?
        .find_reference(source.as_ref())?
        .peel_to_id()?
        .detach();

    // Simulate a lock left by a Git process that crashed while attempting to create the
    // destination branch. This doesn't interfere with deleting `foo/bar`, but prevents creating
    // `foo` after its parent directory has been pruned.
    std::fs::write(
        ctx.repo.get()?.path().join("refs/heads/foo.lock"),
        b"stale lock",
    )?;

    but_api::branch::branch_rename(&mut ctx, source.clone(), "foo".into())
        .expect_err("the stale destination lock must make creation fail");

    let repo = ctx.repo.get()?;
    let restored = repo.try_find_reference(source.as_ref())?;
    let destination_exists = repo.try_find_reference(destination.as_ref())?.is_some();
    assert!(!destination_exists, "the destination was never created");
    let mut restored = restored.unwrap_or_else(|| {
        panic!("the source branch must be restored; destination exists: {destination_exists}")
    });
    assert_eq!(
        restored.peel_to_id()?,
        source_id,
        "rollback must restore the source at its original commit"
    );
    assert_eq!(
        repo.head_name()?.expect("HEAD is symbolic"),
        source,
        "HEAD must continue to name the restored source branch"
    );
    let recovery_ref = repo
        .references()?
        .prefixed("refs/gitbutler/rename-backup/")?
        .next()
        .transpose()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    assert!(
        recovery_ref.is_none(),
        "the recovery ref must be removed after restoring the source"
    );

    Ok(())
}
