use anyhow::Result;
use but_ctx::Context;
use git2::build::CheckoutBuilder;
use gitbutler_edit_mode::commands::{abort_and_return_to_workspace, enter_edit_mode, save_and_return_to_workspace};
use gitbutler_operating_modes::{EDIT_BRANCH_REF, WORKSPACE_BRANCH_REF};
use gitbutler_stack::VirtualBranchesHandle;
use tempfile::TempDir;

fn command_ctx(folder: &str) -> Result<(Context, TempDir)> {
    gitbutler_testsupport::writable::fixture("edit_mode.sh", folder)
}

// Fixture:
// * xxx (HEAD -> gitbutler/workspace) GitButler Workspace Commit
// * xxx foobar
// | * 1e2a3a8 (right) right
// |/
// | * f3d2634 (left) left
// |/
// * 7950f06 (origin/main, origin/HEAD, main) init
// Where "left" and "right" contain changes which conflict with each other
#[test]
fn conficted_entries_get_written_when_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.git2_repo.get()?;

    let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();
    drop(repo);
    enter_edit_mode(&mut ctx, foobar, stack.id)?;

    let repo = ctx.git2_repo.get()?;
    let init = repo.find_reference("refs/heads/main")?.peel_to_commit()?;
    let left = repo.find_reference("refs/heads/left")?.peel_to_commit()?;
    let right = repo.find_reference("refs/heads/right")?.peel_to_commit()?;

    let mut merge = repo.merge_trees(&init.tree()?, &left.tree()?, &right.tree()?, Default::default())?;

    repo.checkout_index(
        Some(&mut merge),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    drop((init, left, right));
    drop(repo);
    save_and_return_to_workspace(&mut ctx)?;

    let repo = ctx.git2_repo.get()?;
    assert_eq!(
        std::fs::read_to_string(repo.path().parent().unwrap().join("conflict"))?,
        "<<<<<<< ours\nleft\n|||||||\n=======\nright\n>>>>>>> theirs\n".to_string()
    );

    Ok(())
}

#[test]
fn abort_requires_force_when_changes_were_made() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.git2_repo.get()?;
    let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();
    drop(repo);

    enter_edit_mode(&mut ctx, foobar, stack.id)?;

    let repo = ctx.git2_repo.get()?;
    assert_eq!(repo.head()?.name(), Some(EDIT_BRANCH_REF));
    let worktree_dir = repo.path().parent().unwrap().to_path_buf();
    drop(repo);

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    let result = abort_and_return_to_workspace(&mut ctx, false);
    assert!(result.is_err());
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("forced abort is necessary"),
        "expected force guidance error, got: {err}"
    );
    assert_eq!(ctx.git2_repo.get()?.head()?.name(), Some(EDIT_BRANCH_REF));

    abort_and_return_to_workspace(&mut ctx, true)?;
    assert_eq!(ctx.git2_repo.get()?.head()?.name(), Some(WORKSPACE_BRANCH_REF));

    Ok(())
}
