use bstr::ByteSlice;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_stack::StackId;
use gitbutler_testsupport::stack_details;

use super::Test;

#[test]
fn no_diffs() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);

    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    // Create a commit on the source branch
    gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add a", None).unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    // Create a second commit on the source branch, to be moved
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add b", None).unwrap();

    std::fs::write(repo.path().join("c.txt"), "This is c").unwrap();

    // Create a third commit on the source branch

    gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add c", None).unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    gitbutler_branch_actions::create_commit(ctx, target_stack_entry.id, "Add d", None).unwrap();

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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    // Create a commit on the source branch
    gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add a", None).unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    // Create as second commit on the source branch, to be moved
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add b", None).unwrap();

    // Uncommitted changes on the source branch
    std::fs::write(repo.path().join("c.txt"), "This is c").unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    // State of source branch after the two commits
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 2);

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    gitbutler_branch_actions::create_commit(ctx, target_stack_entry.id, "Add d", None).unwrap();

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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    std::fs::write(repo.path().join("another file.txt"), "another content").unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

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
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    // Uncommitted changes on the source branch
    std::fs::write(repo.path().join("another file.txt"), "another content").unwrap();

    let source = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == source_branch_id)
        .unwrap();

    // State of source branch after the first commit
    assert_eq!(source.1.branch_details[0].clone().commits.len(), 1);

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

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
fn target_commit_locked_to_ancestors() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let depends_on_commit =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add a", None).unwrap();

    std::fs::write(repo.path().join("a.txt"), "This is a \n\n Updated").unwrap();
    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add b and update b", None)
            .unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let result = gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid,
        source_branch_id,
    );

    let illegal_move = result.unwrap().unwrap();

    assert!(matches!(
        illegal_move,
        gitbutler_branch_actions::MoveCommitIllegalAction::DependsOnCommits(commits) if commits == vec![depends_on_commit.to_string()]
    ));
}

#[test]
fn target_commit_locked_to_descendants() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("a.txt"), "This is a").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add a", None).unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b").unwrap();

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Add b and update b", None)
            .unwrap();

    std::fs::write(repo.path().join("b.txt"), "This is b and an update").unwrap();

    let dependent_commit =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "Update b", None).unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let result = gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid,
        source_branch_id,
    );

    let illegal_move = result.unwrap().unwrap();

    assert!(matches!(
        illegal_move,
        gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentChanges(commits) if commits == vec![dependent_commit.to_string()]
    ));
}

#[test]
fn locked_hunks_on_source_branch() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    std::fs::write(repo.path().join("file.txt"), "locked content").unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // This should be OK in the new assignments system because when the assignments are reevaluated, the uncommitted changes will be in the right place
    assert!(gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_entry.id,
        commit_oid,
        source_branch_id
    )
    .is_ok());
}

#[test]
fn no_commit() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    let target_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let commit_id_hex = "a99c95cca7a60f1a2180c2f86fb18af97333c192";
    assert_eq!(
        gitbutler_branch_actions::move_commit(
            ctx,
            target_stack_entry.id,
            git2::Oid::from_str(commit_id_hex).unwrap(),
            source_branch_id,
        )
        .unwrap_err()
        .to_string(),
        format!("commit {commit_id_hex} to be moved could not be found")
    );
}

#[test]
fn no_branch() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();
    let details = stack_details(ctx);
    assert_eq!(details.len(), 1);
    let source_branch_id = details[0].0;

    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, source_branch_id, "commit", None).unwrap();

    let id = StackId::generate();
    assert_eq!(
        gitbutler_branch_actions::move_commit(ctx, id, commit_oid, source_branch_id)
            .unwrap_err()
            .to_string(),
        "Destination branch not found"
    );
}
