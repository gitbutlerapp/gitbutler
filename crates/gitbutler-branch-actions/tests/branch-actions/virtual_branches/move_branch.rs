use bstr::ByteSlice;
use but_testsupport::legacy::stack_details;
use gitbutler_branch::BranchCreateRequest;

use super::{Test, create_commit};

/// Move a branch that has no dependencies on commits remaining in the source
/// stack. The source stack is left empty and deleted; the destination gains
/// the branch. No conflicts should arise.
#[test]
fn move_branch_non_overlapping() {
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

    // Source stack: one branch, edits file-a.
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
    create_commit(ctx, source_id, "source: add file-a").unwrap();

    let details = stack_details(ctx);
    let (_, source_sd) = details.iter().find(|(id, _)| *id == source_id).unwrap();
    let source_branch_name = source_sd.branch_details[0].name.to_str_lossy().to_string();

    // Destination stack: one branch, edits file-b.
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
    create_commit(ctx, dest_id, "dest: add file-b").unwrap();

    let details = stack_details(ctx);
    let (_, dest_sd) = details.iter().find(|(id, _)| *id == dest_id).unwrap();
    let dest_branch_name = dest_sd.branch_details[0].name.to_str_lossy().to_string();

    let result = gitbutler_branch_actions::move_branch(
        ctx,
        dest_id,
        &dest_branch_name,
        source_id,
        &source_branch_name,
    );
    assert!(result.is_ok(), "move_branch should succeed: {result:?}");

    let details = stack_details(ctx);

    // Source stack was the only branch — it should be deleted.
    let result = result.unwrap();
    assert!(
        result.deleted_stacks.contains(&source_id),
        "source stack should be deleted after its only branch was moved"
    );

    // Destination should now have 2 branches, no conflicts.
    let (_, dest_sd) = details
        .iter()
        .find(|(id, _)| *id == dest_id)
        .expect("dest stack should exist");
    assert_eq!(
        dest_sd.branch_details.len(),
        2,
        "dest should have 2 branches"
    );
    for bd in &dest_sd.branch_details {
        assert!(
            bd.commits.iter().all(|c| !c.has_conflicts),
            "no conflicts expected"
        );
    }
}

/// Moving a branch whose commits are depended upon by remaining branches
/// in the source stack must be rejected: the remaining branches would conflict
/// when rebased without the moved branch.
#[test]
fn move_branch_with_dependency_rejected() {
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

    // Source stack: bottom branch creates shared.txt; top branch modifies it.
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
    create_commit(ctx, source_id, "bottom: create shared.txt").unwrap();

    let details = stack_details(ctx);
    let (_, source_sd) = details.iter().find(|(id, _)| *id == source_id).unwrap();
    let bottom_branch_name = source_sd.branch_details[0].name.to_str_lossy().to_string();

    gitbutler_branch_actions::stack::create_branch(
        ctx,
        source_id,
        gitbutler_branch_actions::stack::CreateSeriesRequest {
            name: "source-top".to_string(),
            target_patch: None,
            preceding_head: None,
        },
    )
    .unwrap();

    std::fs::write(
        repo.path().join("shared.txt"),
        "alpha\nbravo_modified\ncharlie\n",
    )
    .unwrap();
    let mut guard = ctx.exclusive_worktree_access();
    but_workspace::legacy::commit_engine::create_commit_simple(
        ctx,
        source_id,
        None,
        vec![but_core::DiffSpec {
            previous_path: None,
            path: "shared.txt".into(),
            hunk_headers: vec![],
        }],
        "top: modify shared.txt".to_string(),
        "source-top".to_string(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

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

    let details = stack_details(ctx);
    let (_, dest_sd) = details.iter().find(|(id, _)| *id == dest_id).unwrap();
    let dest_branch_name = dest_sd.branch_details[0].name.to_str_lossy().to_string();

    // Moving the bottom branch must fail — the top branch depends on it
    // and would conflict when rebased without it.
    let result = gitbutler_branch_actions::move_branch(
        ctx,
        dest_id,
        &dest_branch_name,
        source_id,
        &bottom_branch_name,
    );
    assert!(
        result.is_err(),
        "move_branch should be rejected: remaining top branch depends on the moved bottom branch"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack branch moves: destination conflict is rejected
// ---------------------------------------------------------------------------

/// Moving a branch onto a destination that has already modified the same content
/// must be rejected: the branch commits cannot apply cleanly at the insertion point.
#[test]
fn move_branch_destination_conflict() {
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

    // Source stack: one branch that changes shared.txt to "source".
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
    create_commit(ctx, source_id, "source: change shared.txt").unwrap();

    let details = stack_details(ctx);
    let (_, source_sd) = details.iter().find(|(id, _)| *id == source_id).unwrap();
    let source_branch_name = source_sd.branch_details[0].name.to_str_lossy().to_string();

    // Destination stack: inject a raw commit that also changes shared.txt to "dest".
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

    let dest_branch_name = {
        let details = stack_details(ctx);
        let (_, dest_sd) = details.iter().find(|(id, _)| *id == dest_id).unwrap();
        dest_sd.branch_details[0].name.to_str_lossy().to_string()
    };
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

    // Moving must fail: the branch's commit conflicts at the destination insertion point.
    let result = gitbutler_branch_actions::move_branch(
        ctx,
        dest_id,
        &dest_branch_name,
        source_id,
        &source_branch_name,
    );
    assert!(
        result.is_err(),
        "move_branch should be rejected: branch commits conflict at the destination"
    );
}

// ---------------------------------------------------------------------------
// Cross-stack branch moves: pre-existing conflict in source does not block
// ---------------------------------------------------------------------------

/// A source stack whose top branch already has a conflicted commit must not block
/// moving the bottom branch out. Only *new* conflicts introduced by the move matter.
#[test]
fn move_branch_preexisting_conflict() {
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

    // Source stack: bottom branch changes shared.txt to "source" (this is what we'll move).
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
    let commit_to_move = create_commit(ctx, source_id, "bottom: change shared.txt").unwrap();

    let details = stack_details(ctx);
    let (_, source_sd) = details.iter().find(|(id, _)| *id == source_id).unwrap();
    let bottom_branch_name = source_sd.branch_details[0].name.to_str_lossy().to_string();

    // Add a top branch to the source stack, then push a pre-existing conflicted commit onto it.
    gitbutler_branch_actions::stack::create_branch(
        ctx,
        source_id,
        gitbutler_branch_actions::stack::CreateSeriesRequest {
            name: "source-top".to_string(),
            target_patch: None,
            preceding_head: None,
        },
    )
    .unwrap();
    {
        let (_guard, repo, ws, _db) = ctx.workspace_and_db().unwrap();
        let merge_base = ws.lower_bound.unwrap();
        // Competitor: from merge_base, also changes shared.txt differently.
        // Cherry-picking it onto commit_to_move produces a 3-way conflict.
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

    let details = stack_details(ctx);
    let (_, dest_sd) = details.iter().find(|(id, _)| *id == dest_id).unwrap();
    let dest_branch_name = dest_sd.branch_details[0].name.to_str_lossy().to_string();

    // Moving the bottom branch must succeed: the top branch's conflict was pre-existing
    // and must not be counted as a new conflict caused by this move.
    let result = gitbutler_branch_actions::move_branch(
        ctx,
        dest_id,
        &dest_branch_name,
        source_id,
        &bottom_branch_name,
    );
    assert!(
        result.is_ok(),
        "move_branch should succeed: top branch's conflict is pre-existing, not caused by this move: {result:?}"
    );
}
