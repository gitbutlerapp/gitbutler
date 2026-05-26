use but_api::WorkspaceState;
use but_core::{DiffSpec, DryRun};
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_testsupport::Sandbox;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gix::{ObjectId, refs::FullName};

use crate::{DynamicOutcome, with_transaction};

// TODO(david): shared.sh is copy-pasted from but tests

#[track_caller]
fn find_commits<const N: usize>(env: &Sandbox, commits: [&str; N]) -> [ObjectId; N] {
    let repo = env.open_repo().unwrap();
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

#[track_caller]
fn worktree_changes_as_specs(repo: &gix::Repository) -> Vec<DiffSpec> {
    let changes = but_core::diff::worktree_changes(repo).unwrap();
    let specs = changes
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect::<Vec<_>>();
    assert!(
        !specs.is_empty(),
        "fixture should contain worktree changes to discard"
    );
    specs
}

#[test]
fn squashing_three_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

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
        env.git_log().unwrap(),
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

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
        env.git_log().unwrap(),
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
fn dynamic_rollback() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

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
        env.git_log().unwrap(),
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    snapbox::assert_data_eq!(
        env.git_log().unwrap(),
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
        env.git_log().unwrap(),
        snapbox::str![[r#"
* 8413d71 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target, branch) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}

#[test]
fn remove_references() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    snapbox::assert_data_eq!(
        env.git_log().unwrap(),
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
        env.git_log().unwrap(),
        snapbox::str![[r#"
* 8413d71 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}

#[test]
fn discard_workspace_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.append_file("file-one", "changed\n");
    env.file("new-file", "new\n");

    snapbox::assert_data_eq!(
        env.git_status().unwrap(),
        snapbox::str![[r#"
 M file-one
?? new-file

"#]]
    );

    let repo = but_testsupport::open_repo(env.projects_root()).unwrap();
    let changes = worktree_changes_as_specs(&repo);
    let mut ctx = Context::from_repo(repo)
        .map(Context::with_memory_app_cache)
        .unwrap();

    assert_num_snapshots(&ctx, 0);

    let mut meta = ctx.meta().unwrap();
    let snapshot_details = SnapshotDetails::new(OperationKind::DiscardChanges);

    with_transaction(
        &mut ctx,
        &mut meta,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let dropped_changes = tx.discard_workspace_changes(changes)?;
            assert!(
                dropped_changes.is_empty(),
                "all matching worktree changes should be discarded"
            );

            Ok(())
        },
    )
    .unwrap();

    snapbox::assert_data_eq!(env.git_status().unwrap(), snapbox::str![[r#""#]]);

    snapbox::assert_data_eq!(
        env.git_log().unwrap(),
        snapbox::str![[r#"
* ebaef69 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 1e25c58 (branch) add file-three
* 9b3b3d5 add file-two
* dbdbcea add file-one
* 6674d4f (origin/main, origin/HEAD, main, gitbutler/target) add random-file

"#]]
    );

    assert_num_snapshots(&ctx, 1);
}
