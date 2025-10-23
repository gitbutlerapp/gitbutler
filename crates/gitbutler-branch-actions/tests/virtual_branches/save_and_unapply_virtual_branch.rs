use but_hunk_assignment::HunkAssignmentRequest;
use but_workspace::DiffSpec;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn unapply_with_data() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
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
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(
        ctx.project().worktree_dir()?.to_owned(),
    )
    .unwrap()
    .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)
            .unwrap();
    let req = HunkAssignmentRequest {
        hunk_header: assignments[0].hunk_header,
        path_bytes: assignments[0].path_bytes.clone(),
        stack_id: Some(stacks[0].0),
    };
    but_hunk_assignment::assign(ctx, vec![req], None).unwrap();
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)
            .unwrap();
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(stacks[0].0))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );

    gitbutler_branch_actions::unapply_stack(ctx, stacks[0].0, assigned_diffspec).unwrap();

    assert!(!repo.path().join("file.txt").exists());

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);

    Ok(())
}

#[test]
fn delete_if_empty() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);

    gitbutler_branch_actions::unapply_stack(ctx, stacks[0].0, Vec::new()).unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);
}
