use std::str::FromStr;

use bstr::ByteSlice;
use but_testsupport::legacy::stack_details;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_stack::StackId;

use super::Test;

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

    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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
    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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
    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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

    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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

    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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

    gitbutler_branch_actions::move_commit(ctx, target_stack_entry.id, commit_oid, source_branch_id)
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
            commit_oid,
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
            gix::ObjectId::from_str(commit_id_hex).unwrap(),
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
        gitbutler_branch_actions::move_commit(ctx, id, commit_oid, source_branch_id)
            .unwrap_err()
            .to_string(),
        "Destination branch not found"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack moves: non-overlapping changes
// ---------------------------------------------------------------------------

/// Move a commit between two stacks that edit different files.
/// No conflicts should arise.
#[test]
fn move_commit_non_overlapping() {
    let Test { repo, ctx, .. } = &mut Test::default();

    std::fs::write(repo.path().join("base.txt"), "base\n").unwrap();
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

    // Source stack: commit edits file-a.
    std::fs::write(repo.path().join("file-a.txt"), "source content\n").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let source_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("source-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let source_id = source_entry.id;
    let commit_oid = super::create_commit(ctx, source_id, "source: add file-a").unwrap();

    // Destination stack: commit edits file-b (non-overlapping).
    std::fs::write(repo.path().join("file-b.txt"), "dest content\n").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let dest_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("dest-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let dest_id = dest_entry.id;
    super::create_commit(ctx, dest_id, "dest: add file-b").unwrap();

    // Move the commit from source to dest.
    gitbutler_branch_actions::move_commit(ctx, dest_id, commit_oid, source_id).unwrap();

    let details = stack_details(ctx);

    let source = details.iter().find(|(id, _)| *id == source_id).unwrap();
    let dest = details.iter().find(|(id, _)| *id == dest_id).unwrap();

    assert_eq!(
        source.1.branch_details[0].commits.len(),
        0,
        "source should have no commits after move"
    );
    assert_eq!(
        dest.1.branch_details[0].commits.len(),
        2,
        "dest should have original + moved commit"
    );

    // No conflicts expected.
    assert!(
        dest.1.branch_details[0]
            .commits
            .iter()
            .all(|c| !c.has_conflicts),
        "no conflicts expected for non-overlapping changes"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack moves: dependent commit is rejected
// ---------------------------------------------------------------------------

/// A commit that modifies a file created by an earlier commit in the source
/// stack cannot be moved: it depends on context that stays in the source, so
/// the cherry-pick onto the destination would conflict.
#[test]
fn move_commit_with_dependency_rejected() {
    let Test { repo, ctx, .. } = &mut Test::default();

    std::fs::write(repo.path().join("base.txt"), "base\n").unwrap();
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

    // Source stack: first commit creates shared.txt, second modifies it.
    std::fs::write(repo.path().join("shared.txt"), "alpha\nbravo\ncharlie\n").unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let source_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("source-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let source_id = source_entry.id;
    let first_commit = super::create_commit(ctx, source_id, "source: create shared.txt").unwrap();

    std::fs::write(
        repo.path().join("shared.txt"),
        "alpha\nbravo_modified\ncharlie\n",
    )
    .unwrap();
    super::create_commit(ctx, source_id, "source: modify shared.txt").unwrap();

    // Destination stack: empty.
    let mut guard = ctx.exclusive_worktree_access();
    let dest_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("dest-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let dest_id = dest_entry.id;

    // Moving the first commit must fail — the second commit depends on it
    // and would conflict when rebased without it.
    assert!(
        gitbutler_branch_actions::move_commit(ctx, dest_id, first_commit, source_id).is_err(),
        "move_commit should be rejected: remaining source commits depend on the moved commit"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack moves: destination conflict is rejected
// ---------------------------------------------------------------------------

/// Moving a commit onto a destination stack that has already modified the same
/// content must be rejected: the commit cannot apply cleanly at the insertion point.
#[test]
fn move_commit_destination_conflict() {
    let Test { repo, ctx, .. } = &mut Test::default();

    std::fs::write(repo.path().join("shared.txt"), "original\n").unwrap();
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

    // Source stack: changes shared.txt to "source".
    std::fs::write(repo.path().join("shared.txt"), "source\n").unwrap();
    let mut guard = ctx.exclusive_worktree_access();
    let source_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("source-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let source_id = source_entry.id;
    let commit_to_move = super::create_commit(ctx, source_id, "source: change shared.txt").unwrap();

    // Destination stack: inject a raw commit that also changes shared.txt to "dest",
    // simulating content that overlaps with the commit being moved.
    let mut guard = ctx.exclusive_worktree_access();
    let dest_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("dest-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let dest_id = dest_entry.id;

    {
        let (_guard, repo, ws, _db) = ctx.workspace_and_db().unwrap();
        let merge_base = ws.lower_bound.unwrap();
        let dest_commit =
            super::make_commit_on_file(&repo, merge_base, "shared.txt", b"dest\n").unwrap();
        let ref_name = ws
            .stacks
            .iter()
            .find(|s| s.id == Some(dest_id))
            .unwrap()
            .ref_name()
            .unwrap()
            .to_owned();
        repo.reference(
            ref_name.as_ref(),
            dest_commit,
            gix::refs::transaction::PreviousValue::Any,
            "test: set dest commit",
        )
        .unwrap();
    }

    // Moving must fail: shared.txt was changed by both stacks, so the commit
    // cannot apply cleanly at the destination insertion point.
    assert!(
        gitbutler_branch_actions::move_commit(ctx, dest_id, commit_to_move, source_id).is_err(),
        "move_commit should be rejected: moved commit conflicts at the destination"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack moves: pre-existing conflict in source does not block
// ---------------------------------------------------------------------------

/// A source stack that already has a conflicted commit (unrelated to the commit
/// being moved) must not block the move. Only *new* conflicts introduced by the
/// move matter.
#[test]
fn move_commit_preexisting_conflict() {
    let Test { repo, ctx, .. } = &mut Test::default();

    std::fs::write(repo.path().join("shared.txt"), "original\n").unwrap();
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

    // Source stack: commit_to_move changes shared.txt to "source".
    std::fs::write(repo.path().join("shared.txt"), "source\n").unwrap();
    let mut guard = ctx.exclusive_worktree_access();
    let source_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("source-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let source_id = source_entry.id;
    let commit_to_move = super::create_commit(ctx, source_id, "source: change shared.txt").unwrap();

    // Create a "competitor" commit from merge_base that also changes shared.txt to "competitor".
    // Cherry-picking it onto commit_to_move will produce a 3-way conflict (both modified
    // shared.txt from "original" in different ways) — giving us a pre-existing conflicted commit.
    {
        let (_guard, repo, ws, _db) = ctx.workspace_and_db().unwrap();
        let merge_base = ws.lower_bound.unwrap();
        let competitor =
            super::make_commit_on_file(&repo, merge_base, "shared.txt", b"competitor\n").unwrap();
        drop(_db);
        drop(ws);
        drop(_guard);
        super::push_conflicted_commit_onto(ctx, source_id, commit_to_move, competitor).unwrap();
    }

    // Destination stack: empty.
    let mut guard = ctx.exclusive_worktree_access();
    let dest_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("dest-stack".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let dest_id = dest_entry.id;

    // Moving commit_to_move must succeed: the conflicted commit above it in the
    // source stack was already conflicted before the move, so it must not block.
    assert!(
        gitbutler_branch_actions::move_commit(ctx, dest_id, commit_to_move, source_id).is_ok(),
        "move_commit should succeed: the conflicted commit in source is pre-existing"
    );
}
