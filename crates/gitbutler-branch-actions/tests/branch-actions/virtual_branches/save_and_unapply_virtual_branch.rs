use but_core::DiffSpec;
use but_hunk_assignment::HunkAssignmentRequest;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn unapply_with_data() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    std::fs::write(repo.path().join("file.txt"), "content").unwrap();

    let _stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);

    let changes = but_core::diff::ui::worktree_changes(&*ctx.repo.get()?)
        .unwrap()
        .changes;

    let context_lines = ctx.settings.context_lines;
    let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(guard.read_permission())?;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        Some(changes.clone()),
        None,
        context_lines,
    )
    .unwrap();
    let req = HunkAssignmentRequest {
        hunk_header: assignments[0].hunk_header,
        path_bytes: assignments[0].path_bytes.clone(),
        stack_id: Some(stacks[0].0),
    };
    but_hunk_assignment::assign(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        vec![req],
        None,
        context_lines,
    )
    .unwrap();
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        Some(changes.clone()),
        None,
        context_lines,
    )
    .unwrap();
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(stacks[0].0))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );

    drop((repo, ws, db));
    gitbutler_branch_actions::unapply_stack(
        ctx,
        guard.write_permission(),
        stacks[0].0,
        assigned_diffspec,
    )
    .unwrap();

    assert!(!ctx.repo.get()?.path().join("file.txt").exists());

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);

    Ok(())
}

#[test]
fn delete_if_empty() {
    let Test { ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::unapply_stack(ctx, guard.write_permission(), stacks[0].0, Vec::new())
        .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 0);
}
