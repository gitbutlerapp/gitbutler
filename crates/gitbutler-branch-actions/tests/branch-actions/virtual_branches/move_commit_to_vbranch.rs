use bstr::ByteSlice;
use but_oxidize::OidExt;
use gitbutler_branch::BranchCreateRequest;
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

/// Moving the top commit of a two-commit stack (both editing the same file) to an
/// empty branch, where the file has the blank-line pattern that triggers the Myers
/// diff false conflict (GitoxideLabs/gitoxide#2475).
///
/// The 3-way merge base has the exact blank-line structure (`item\n\nitem\nitem\n\n`)
/// that causes Myers to produce an empty insertion hunk colliding with the other
/// side's deletion.
///
/// Once the upstream gitoxide fix lands, this test should be updated to assert that
/// the move succeeds cleanly.
#[test]
fn myers_false_conflict_on_move_to_empty_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Set up initial file content on origin/master so it becomes the merge base.
    std::fs::write(
        repo.path().join("shared-file"),
        "alpha_x\n\nbravo_x\ncharlie_x\n\n",
    )
    .unwrap();
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

    // First commit: delete alpha_x (keep the blank-line structure).
    std::fs::write(
        repo.path().join("shared-file"),
        "\n\nbravo_x\ncharlie_x\n",
    )
    .unwrap();

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

    create_commit(ctx, source_branch_id, "delete alpha_x").unwrap();

    // Second commit: delete bravo_x.
    std::fs::write(repo.path().join("shared-file"), "\n\ncharlie_x\n").unwrap();
    let top_commit_oid = create_commit(ctx, source_branch_id, "delete bravo_x").unwrap();

    // Create empty target branch.
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
        top_commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(
        destination.1.branch_details[0].commits.len(),
        1,
        "The moved commit should be on the destination branch"
    );

    // BUG (gitoxide#2475): The legacy rebase uses merge_options_force_ours, so the
    // Myers false conflict is silently auto-resolved rather than producing an error
    // or a conflicted commit. The commit appears clean but may have incorrect content
    // (the "ours" side wins where the merge should have combined both sides).
    //
    // Verify the commit exists and is not marked conflicted — but note the tree
    // content may be wrong due to the forced resolution.
    assert!(
        !destination.1.branch_details[0].commits[0].has_conflicts,
        "The legacy path force-resolves conflicts, so the commit should appear clean. \
         If this changes, the rebase strategy may have been updated."
    );

    // Check the actual tree content of the moved commit.
    let gix_repo = ctx.repo.get().unwrap();
    let moved_commit_id = destination.1.branch_details[0].commits[0].id;
    let moved_commit = gix_repo.find_commit(moved_commit_id).unwrap();
    let tree = gix_repo
        .find_tree(moved_commit.tree_id().unwrap())
        .unwrap();
    let entry = tree.find_entry("shared-file").expect("shared-file should exist in tree");
    let blob = gix_repo.find_blob(entry.id()).unwrap();
    let content = std::str::from_utf8(blob.data.as_ref()).unwrap();

    // The correct result of cherry-picking "delete bravo_x" onto origin/master
    // (which has `alpha_x\n\nbravo_x\ncharlie_x\n\n`) should be:
    // `alpha_x\n\ncharlie_x\n\n` (bravo_x removed, alpha_x kept).
    //
    // TODO: Once the upstream gitoxide fix lands, assert the correct content.
    // For now, just record what the legacy path actually produces.
    assert!(
        !content.is_empty(),
        "The moved commit should have content in shared-file"
    );
}

/// Moving the top commit of a two-commit stack to an empty branch, where the two
/// commits edit different, non-overlapping parts of the same file. Commit 1 adds
/// "first" at the top, commit 2 adds "last" at the bottom.
///
/// Cherry-picking commit 2 onto origin/master produces a 3-way merge where
/// base→ours removes "first" at top, base→theirs adds "last" at bottom.
/// These are non-overlapping edits that should merge cleanly.
#[test]
fn dependent_changes_move_to_empty_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Set up initial file content on origin/master.
    std::fs::write(
        repo.path().join("shared-file"),
        "alpha\nbravo\ncharlie\n",
    )
    .unwrap();
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

    // First commit: add "first" at the top.
    std::fs::write(
        repo.path().join("shared-file"),
        "first\nalpha\nbravo\ncharlie\n",
    )
    .unwrap();

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

    create_commit(ctx, source_branch_id, "add first at top").unwrap();

    // Second commit: add "last" at the bottom.
    std::fs::write(
        repo.path().join("shared-file"),
        "first\nalpha\nbravo\ncharlie\nlast\n",
    )
    .unwrap();
    let top_commit_oid = create_commit(ctx, source_branch_id, "add last at bottom").unwrap();

    // Create empty target branch.
    let mut guard = ctx.exclusive_worktree_access();
    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Non-overlapping edits (top vs bottom of file) — should merge cleanly.
    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        top_commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(
        destination.1.branch_details[0].commits.len(),
        1,
        "The moved commit should be on the destination branch"
    );

    assert!(
        !destination.1.branch_details[0].commits[0].has_conflicts,
        "Non-overlapping edits (add at top vs add at bottom) should merge cleanly."
    );
}

/// Moving the top commit of a two-commit stack (both editing the SAME line) to an
/// empty branch. This is a genuinely overlapping edit — commit 1 changes `bravo` →
/// `bravo_modified`, commit 2 changes `bravo_modified` → `bravo_replaced`.
///
/// Cherry-picking commit 2 onto origin/master produces a real 3-way conflict:
/// base=`bravo_modified`, ours=`bravo`, theirs=`bravo_replaced`.
#[test]
fn overlapping_changes_move_to_empty_branch() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Set up initial file content on origin/master.
    std::fs::write(
        repo.path().join("shared-file"),
        "alpha\nbravo\ncharlie\n",
    )
    .unwrap();
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

    // First commit: modify bravo → bravo_modified.
    std::fs::write(
        repo.path().join("shared-file"),
        "alpha\nbravo_modified\ncharlie\n",
    )
    .unwrap();

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

    create_commit(ctx, source_branch_id, "modify bravo").unwrap();

    // Second commit: replace bravo_modified → bravo_replaced.
    std::fs::write(
        repo.path().join("shared-file"),
        "alpha\nbravo_replaced\ncharlie\n",
    )
    .unwrap();
    let top_commit_oid = create_commit(ctx, source_branch_id, "replace bravo").unwrap();

    // Create empty target branch.
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
        top_commit_oid.to_gix(),
        source_branch_id,
    )
    .unwrap();

    let destination = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == target_stack_entry.id)
        .unwrap();

    assert_eq!(
        destination.1.branch_details[0].commits.len(),
        1,
        "The moved commit should be on the destination branch"
    );

    // This is a genuine conflict — both sides modify the same line. The legacy
    // path correctly produces a conflicted commit here.
    assert!(
        destination.1.branch_details[0].commits[0].has_conflicts,
        "The moved commit should be conflicted — both commits edit the same line, \
         so cherry-picking onto main creates a real 3-way conflict."
    );
}
