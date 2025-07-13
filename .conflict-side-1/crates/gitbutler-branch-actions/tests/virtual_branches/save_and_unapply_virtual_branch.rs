use but_hunk_assignment::HunkAssignmentRequest;
use but_workspace::DiffSpec;

use super::*;

#[test]
fn unapply_with_data() {
    let Test { repo, ctx, .. } = &mut Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;

    assert_eq!(branches.len(), 1);

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())
        .unwrap()
        .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)
            .unwrap();
    let req = HunkAssignmentRequest {
        hunk_header: assignments[0].hunk_header,
        path_bytes: assignments[0].path_bytes.clone(),
        stack_id: Some(branches[0].id),
    };
    but_hunk_assignment::assign(ctx, vec![req], None).unwrap();
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)
            .unwrap();
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(branches[0].id))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );

    gitbutler_branch_actions::unapply_stack(ctx, branches[0].id, assigned_diffspec).unwrap();

    assert!(!repo.path().join("file.txt").exists());

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 0);
}

#[test]
fn delete_if_empty() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::unapply_stack(ctx, branches[0].id, Vec::new()).unwrap();

    let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
    let branches = list_result.branches;
    assert_eq!(branches.len(), 0);
}
