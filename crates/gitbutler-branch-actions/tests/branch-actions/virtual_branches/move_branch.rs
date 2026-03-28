use bstr::ByteSlice;
use but_testsupport::legacy::stack_details;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_stack::VirtualBranchesHandle;

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

/// A branch whose commits depend on commits remaining in the source stack
/// cannot be moved: cherry-picking onto the destination would conflict.
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

    {
        let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
        let mut stack = vb_state.get_stack_in_workspace(source_id).unwrap();
        stack
            .add_series_top_of_stack(ctx, "source-top".to_string())
            .unwrap();
    }

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

    // Moving the dependent top branch must fail.
    let result = gitbutler_branch_actions::move_branch(
        ctx,
        dest_id,
        &dest_branch_name,
        source_id,
        "source-top",
    );
    assert!(
        result.is_err(),
        "move_branch should be rejected: top branch depends on bottom branch that stays in source"
    );
}
