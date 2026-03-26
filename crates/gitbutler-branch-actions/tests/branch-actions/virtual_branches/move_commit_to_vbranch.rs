use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::OidExt;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_oplog::OplogExt;
use gitbutler_stack::StackId;
use gitbutler_testsupport::stack_details;

use super::{Test, create_commit};

#[test]
fn no_diffs() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);

    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 1);
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 0);
}

#[test]
fn multiple_commits() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    // Create a commit on the source branch
    super::create_commit(ctx, source_branch_id, "Add a").unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    // Create a second commit on the source branch, to be moved
    let commit_oid = super::create_commit(ctx, source_branch_id, "Add b").unwrap();

    std::fs::write(repo.path().join("c.txt"), "This is c").unwrap();

    // Create a third commit on the source branch

    super::create_commit(ctx, source_branch_id, "Add c").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    super::create_commit(ctx, target_stack_entry.id, "Add d").unwrap();

    // Move the top commit from the source branch to the destination branch
    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();
    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 2);
    assert_eq!(
        destination.1.branch_details[0]
            .clone()
            .commits
            .into_iter()
            .map(|c| c.message.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add b", "Add d"]
    );

    assert_eq!(source.1.branch_details[0].clone().commits.len(), 2);
    assert_eq!(
        source.1.branch_details[0]
            .clone()
            .commits
            .into_iter()
            .map(|c| c.message.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add c", "Add a"]
    );
}

#[test]
fn multiple_commits_with_diffs() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    // Create a commit on the source branch
    super::create_commit(ctx, source_branch_id, "Add a").unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    // Create as second commit on the source branch, to be moved
    let commit_oid = super::create_commit(ctx, source_branch_id, "Add b").unwrap();

    // Uncommitted changes on the source branch
    std::fs::write(repo.path().join("c.txt"), "This is c").unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    // State of source branch after the two commits
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 2);

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    super::create_commit(ctx, target_stack_entry.id, "Add d").unwrap();

    // Uncommitted changes on the destination branch
    std::fs::write(repo.path().join("e.txt"), "This is e").unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    // State of destination branch before the commit is moved
    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 1);

    // Move the top commit from the source branch to the destination branch
    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();
    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 2);
    assert_eq!(
        destination.1.branch_details[0]
            .clone()
            .commits
            .into_iter()
            .map(|c| c.message.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add b", "Add d"]
    );
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 1);
    assert_eq!(
        source.1.branch_details[0]
            .clone()
            .commits
            .into_iter()
            .map(|c| c.message.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add a"]
    );
}

#[test]
fn diffs_on_source_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    std::fs::write(repo.path().join("another file.txt"), "another content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();
    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 1);
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 0);
}

#[test]
fn diffs_on_target_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("another file.txt"), "another content").unwrap();

    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();
    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();
    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 1);
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 0);
}

#[test]
fn diffs_on_both_branches() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    // Uncommitted changes on the source branch
    std::fs::write(repo.path().join("another file.txt"), "another content").unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    // State of source branch after the first commit
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 1);

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Uncommitted changes on the destination branch
    std::fs::write(
        repo.path().join("yet another file.txt"),
        "yet another content",
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    // State of the destination branch before the commit is moved
    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 0);

    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();
    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(destination.1.branch_details[0].clone().commits.len(), 1);

    assert_eq!(source.1.branch_details[0].clone().commits.len(), 0);
}

#[test]
fn locked_hunks_on_source_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    std::fs::write(repo.path().join("file.txt"), "locked content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // This should be OK in the new assignments system because when the assignments are reevaluated, the uncommitted changes will be in the right place
    assert!(
        gitbutler_branch_actions::move_commit(
            ctx,
            target_stack_entry.id,
            commit_oid.to_gix(),
            source_branch_id
        )
        .is_ok()
    );
}

#[test]
fn no_commit() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    super::create_commit(ctx, source_branch_id, "commit").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let commit_id_hex = "a99c95cca7a60f1a2180c2f86fb18af97333c192";
    assert_eq!(
        gitbutler_branch_actions::move_commit(
            ctx,
            target_stack_entry.id,
            git2::Oid::from_str(commit_id_hex).unwrap().to_gix(),
            source_branch_id,
        )
        .unwrap_err()
        .to_string(),
        format!("commit {commit_id_hex} to be moved could not be found")
    );
}

#[test]
fn no_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid = super::create_commit(ctx, source_branch_id, "commit").unwrap();

    let id = StackId::generate();
    assert_eq!(
        gitbutler_branch_actions::move_commit(ctx, id, commit_oid.to_gix(), source_branch_id)
            .unwrap_err()
            .to_string(),
        "Destination branch not found"
    );
}

/// Without the `index.write()` fix, `restore_snapshot` read the snapshot's index
/// tree into memory but never flushed it — the on-disk index retained its
/// pre-restore state, which could include files from operations that were undone.
#[test]
fn restore_persists_index_to_disk() {
    let Test { repo, ctx, .. } = &mut Test::default();

    std::fs::write(repo.path().join("base.txt"), "base content\n").unwrap();
    repo.commit_all("M");
    repo.push();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let read_index_paths = |ctx: &Context| -> Vec<String> {
        let git2_repo = ctx.git2_repo.get().unwrap();
        let mut index = git2_repo.index().unwrap();
        index.read(true).unwrap();
        index
            .iter()
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .collect()
    };

    // Stack A: commit that adds a file.
    std::fs::write(repo.path().join("file-a.txt"), "from stack a\n").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let details = stack_details(ctx);
    let stack_a_id = details[0].0;
    let commit_a = create_commit(ctx, stack_a_id, "add file-a").unwrap();

    // Stack B: commit adding a different file.
    let mut guard = ctx.exclusive_worktree_access();
    let stack_b = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    std::fs::write(repo.path().join("file-b.txt"), "from stack b\n").unwrap();
    create_commit(ctx, stack_b.id, "add file-b").unwrap();

    let index_paths_before_move = read_index_paths(ctx);

    // Move a commit between stacks, then restore the snapshot to undo it.
    gitbutler_branch_actions::move_commit(ctx, stack_b.id, commit_a.to_gix(), stack_a_id).unwrap();

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None).unwrap();
    let move_snapshot = snapshots
        .iter()
        .find(|s| {
            s.details
                .as_ref()
                .is_some_and(|d| d.operation == gitbutler_oplog::entry::OperationKind::MoveCommit)
        })
        .expect("MoveCommit snapshot should exist");

    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(move_snapshot.commit_id, guard.write_permission())
        .unwrap();
    drop(guard);

    // Re-read the on-disk index after restore — without `index.write()` in
    // restore_snapshot, this would still reflect the post-move state.
    assert_eq!(
        index_paths_before_move,
        read_index_paths(ctx),
        "On-disk index should match pre-move state after restore"
    );
}
