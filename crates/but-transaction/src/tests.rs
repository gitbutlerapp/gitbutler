use but_api::WorkspaceState;
use but_core::DryRun;
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_testsupport::Sandbox;
use but_workspace::{
    branch::create_reference::{Anchor, Position},
    commit::squash_commits::MessageCombinationStrategy,
};
use gix::{ObjectId, refs::FullName};

use crate::{DynamicOutcome, with_transaction};

#[track_caller]
fn ref_target(env: &Sandbox, ref_name: &gix::refs::FullNameRef) -> Option<ObjectId> {
    env.open_repo()
        .try_find_reference(ref_name)
        .unwrap()
        .map(|mut reference| reference.peel_to_id().unwrap().detach())
}

// TODO(david): shared.sh is copy-pasted from but tests

#[track_caller]
fn find_commits<const N: usize>(env: &Sandbox, commits: [&str; N]) -> [ObjectId; N] {
    let repo = env.open_repo();
    commits.map(|commit| repo.rev_parse_single(commit).unwrap().detach())
}

#[track_caller]
fn assert_num_snapshots(ctx: &Context, expected: usize) {
    assert_eq!(
        expected,
        but_api::legacy::oplog::snapshots_iter(ctx, None, None, None)
            .unwrap()
            .count(),
    );
}

#[test]
fn squashing_three_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three, two, one] = find_commits(&env, ["1e25c58", "9b3b3d5", "dbdbcea"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);

    let _must_return_workspace: WorkspaceState = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let new_two = tx.squash_commits(
                Vec::from([three]),
                two,
                MessageCombinationStrategy::KeepBoth,
            )?;
            let new_one = tx.squash_commits(
                Vec::from([new_two]),
                one,
                MessageCombinationStrategy::KeepBoth,
            )?;
            tx.reword_commit(new_one, "squashed".into())?;

            Ok(())
        },
    )
    .unwrap();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ec6f55f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* a2fc6dc (branch) squashed
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}

#[test]
fn rollback() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three, two, one] = find_commits(&env, ["1e25c58", "9b3b3d5", "dbdbcea"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);

    let _must_return_unit: () = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.squash_commits([three], two, MessageCombinationStrategy::KeepBoth)?;
            tx.squash_commits([two], one, MessageCombinationStrategy::KeepBoth)?;

            Ok(tx.rollback(()))
        },
    )
    .unwrap();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ebaef69 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 1e25c58 (branch) add file-three
* 9b3b3d5 add file-two
* dbdbcea add file-one
* 6674d4f (origin/main, origin/HEAD, main) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 0);
}

#[test]
fn create_reference_without_creating_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three] = find_commits(&env, ["1e25c58"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
    let refname = FullName::try_from("refs/heads/created-without-commits").unwrap();

    let _workspace: WorkspaceState = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                refname.as_ref(),
                Anchor::at_id(three, Position::Above),
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;

            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        Some(three),
        ref_target(&env, refname.as_ref()),
        "created reference should be persisted even if no commits are created"
    );
    assert_num_snapshots(&ctx, 1);
}

#[test]
fn create_reference_relative_to_various_anchors() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three, two, base] = find_commits(&env, ["1e25c58", "9b3b3d5", "6674d4f"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
    let branch = FullName::try_from("refs/heads/branch").unwrap();
    let at_commit_above = FullName::try_from("refs/heads/at-commit-above").unwrap();
    let at_commit_below = FullName::try_from("refs/heads/at-commit-below").unwrap();
    let at_segment_above = FullName::try_from("refs/heads/at-segment-above").unwrap();
    let at_segment_below = FullName::try_from("refs/heads/at-segment-below").unwrap();
    let independent = FullName::try_from("refs/heads/independent").unwrap();

    let _workspace: WorkspaceState = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            for (refname, anchor) in [
                (
                    at_commit_above.as_ref(),
                    Some(Anchor::at_id(three, Position::Above)),
                ),
                (
                    at_commit_below.as_ref(),
                    Some(Anchor::at_id(three, Position::Below)),
                ),
                (
                    at_segment_above.as_ref(),
                    Some(Anchor::at_segment(branch.as_ref(), Position::Above)),
                ),
                (
                    at_segment_below.as_ref(),
                    Some(Anchor::at_segment(branch.as_ref(), Position::Below)),
                ),
                (independent.as_ref(), None),
            ] {
                tx.create_reference(
                    refname,
                    anchor,
                    |_| but_core::ref_metadata::StackId::generate(),
                    None,
                )?;
            }

            Ok(())
        },
    )
    .unwrap();

    assert_eq!(Some(three), ref_target(&env, at_commit_above.as_ref()));
    assert_eq!(Some(two), ref_target(&env, at_commit_below.as_ref()));
    assert_eq!(Some(three), ref_target(&env, at_segment_above.as_ref()));
    assert_eq!(Some(two), ref_target(&env, at_segment_below.as_ref()));
    assert_eq!(Some(base), ref_target(&env, independent.as_ref()));
    assert_num_snapshots(&ctx, 1);
}

#[test]
fn create_reference_then_remove_it_in_same_transaction() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three] = find_commits(&env, ["1e25c58"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
    let refname = FullName::try_from("refs/heads/create-then-remove").unwrap();

    let _workspace: WorkspaceState = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                refname.as_ref(),
                Anchor::at_id(three, Position::Above),
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            tx.remove_reference(refname.as_ref())?;

            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        None,
        ref_target(&env, refname.as_ref()),
        "reference created and removed in one transaction should not persist"
    );
    assert_num_snapshots(&ctx, 1);
}

#[test]
fn create_reference_then_commit_below_anchor_keeps_commit_in_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three, base] = find_commits(&env, ["1e25c58", "6674d4f"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
    let branch = FullName::try_from("refs/heads/branch").unwrap();
    let refname = FullName::try_from("refs/heads/new-lower-branch").unwrap();

    let outcome = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                refname.as_ref(),
                Anchor::at_segment(branch.as_ref(), Position::Below),
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            let new_commit =
                tx.insert_blank_commit(RelativeTo::Reference(refname.clone()), InsertSide::Below)?;

            Ok(DynamicOutcome::<_, ()>::Commit(new_commit))
        },
    )
    .unwrap();

    let DynamicOutcome::Commit((new_commit, _workspace)) = outcome else {
        panic!("transaction should commit");
    };

    assert_eq!(Some(new_commit), ref_target(&env, refname.as_ref()));

    let repo = env.open_repo();
    let mut branch_commit = ref_target(&env, branch.as_ref()).unwrap();
    let mut commits_above_new_branch = 0;
    while branch_commit != new_commit {
        commits_above_new_branch += 1;
        let commit = repo.find_commit(branch_commit).unwrap();
        let commit = commit.decode().unwrap();
        branch_commit = commit.parents().next().unwrap();
    }
    assert_eq!(
        3, commits_above_new_branch,
        "new lower branch commit should be below oldest commit in anchored segment"
    );
    let lower_branch_tip = repo.find_commit(new_commit).unwrap();
    let lower_branch_tip = lower_branch_tip.decode().unwrap();
    assert_eq!(
        Some(base),
        lower_branch_tip.parents().next(),
        "new lower branch commit should be inserted above segment base"
    );
    assert_ne!(
        Some(three),
        ref_target(&env, branch.as_ref()),
        "upper branch should be rebased onto inserted lower branch commit"
    );
    assert_num_snapshots(&ctx, 1);
}

#[test]
fn create_reference_then_commit_relative_to_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three] = find_commits(&env, ["1e25c58"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
    let refname = FullName::try_from("refs/heads/new-branch").unwrap();

    let new_commit = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                refname.as_ref(),
                Anchor::at_id(three, Position::Above),
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            let new_commit =
                tx.insert_blank_commit(RelativeTo::Reference(refname.clone()), InsertSide::Below)?;

            Ok(DynamicOutcome::<_, ()>::Commit(new_commit))
        },
    )
    .unwrap();

    let DynamicOutcome::Commit((new_commit, _workspace)) = new_commit else {
        panic!("transaction should commit");
    };
    assert_eq!(
        Some(new_commit),
        ref_target(&env, refname.as_ref()),
        "created reference should point to the commit inserted relative to it"
    );
    assert_num_snapshots(&ctx, 1);
}

#[test]
fn create_reference_is_removed_on_rollback() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three] = find_commits(&env, ["1e25c58"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
    let refname = FullName::try_from("refs/heads/rolled-back").unwrap();

    let outcome = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                refname.as_ref(),
                Anchor::at_id(three, Position::Above),
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;

            Ok(DynamicOutcome::<(), _>::Rollback("nope"))
        },
    )
    .unwrap();

    assert!(matches!(outcome, DynamicOutcome::Rollback("nope")));
    assert_eq!(
        None,
        ref_target(&env, refname.as_ref()),
        "created reference should be removed when the transaction rolls back"
    );
    assert_num_snapshots(&ctx, 0);
}

#[test]
fn dynamic_rollback() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let [three, two, one] = find_commits(&env, ["1e25c58", "9b3b3d5", "dbdbcea"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);

    let outcome = with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.squash_commits([three], two, MessageCombinationStrategy::KeepBoth)?;
            tx.squash_commits([two], one, MessageCombinationStrategy::KeepBoth)?;

            if 2 == 4 {
                Ok(DynamicOutcome::Commit(1))
            } else {
                Ok(DynamicOutcome::Rollback(2))
            }
        },
    )
    .unwrap();

    assert!(matches!(outcome, DynamicOutcome::Rollback(2)));

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ebaef69 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 1e25c58 (branch) add file-three
* 9b3b3d5 add file-two
* dbdbcea add file-one
* 6674d4f (origin/main, origin/HEAD, main) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 0);
}

#[test]
fn discarding_three_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ebaef69 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 1e25c58 (branch) add file-three
* 9b3b3d5 add file-two
* dbdbcea add file-one
* 6674d4f (origin/main, origin/HEAD, main) add random-file

"#]]
    );

    let [three, two, one] = find_commits(&env, ["1e25c58", "9b3b3d5", "dbdbcea"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);

    with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.discard_commits([one])?;
            tx.discard_commits([two])?;
            tx.discard_commits([three])?;

            Ok(())
        },
    )
    .unwrap();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 8413d71 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target, branch) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}

#[test]
fn remove_references() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ebaef69 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 1e25c58 (branch) add file-three
* 9b3b3d5 add file-two
* dbdbcea add file-one
* 6674d4f (origin/main, origin/HEAD, main) add random-file

"#]]
    );

    let [three, two, one] = find_commits(&env, ["1e25c58", "9b3b3d5", "dbdbcea"]);

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);

    let refname = FullName::try_from("refs/heads/branch").unwrap();

    with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            tx.remove_reference(refname.as_ref())?;

            tx.discard_commits([one, two, three])?;

            Ok(())
        },
    )
    .unwrap();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 8413d71 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}
