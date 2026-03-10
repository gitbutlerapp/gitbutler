use anyhow::Result;
use but_ctx::Context;
use git2::build::CheckoutBuilder;
use gitbutler_edit_mode::commands::{
    abort_and_return_to_workspace, enter_edit_mode, save_and_return_to_workspace,
};
use gitbutler_operating_modes::{EDIT_BRANCH_REF, WORKSPACE_BRANCH_REF};
use gitbutler_stack::VirtualBranchesHandle;
use gitbutler_testsupport::run_git_at_dir;
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

    let mut merge = repo.merge_trees(
        &init.tree()?,
        &left.tree()?,
        &right.tree()?,
        Default::default(),
    )?;

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
    let untracked_path = worktree_dir.join("new-untracked-during-edit-mode.txt");
    std::fs::write(&untracked_path, "temporary file\n")?;

    let result = abort_and_return_to_workspace(&mut ctx, false);
    assert!(result.is_err());
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("forced abort is necessary"),
        "expected force guidance error, got: {err}"
    );
    assert_eq!(ctx.git2_repo.get()?.head()?.name(), Some(EDIT_BRANCH_REF));

    abort_and_return_to_workspace(&mut ctx, true)?;
    assert_eq!(
        ctx.git2_repo.get()?.head()?.name(),
        Some(WORKSPACE_BRANCH_REF)
    );
    assert!(
        !untracked_path.exists(),
        "forced abort should clean untracked files in non-submodule repos"
    );

    Ok(())
}

#[test]
fn save_and_return_to_workspace_preserves_submodule_worktree() -> Result<()> {
    let (mut ctx, _tempdir) =
        command_ctx("save_and_return_to_workspace_preserves_submodule_worktree")?;
    let (foobar, worktree_dir) = {
        let repo = ctx.git2_repo.get()?;
        let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();
        (foobar, repo.path().parent().unwrap().to_path_buf())
    };
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();
    let submodule_probe = worktree_dir.join("submodules/test-module/probe.txt");
    assert!(
        submodule_probe.exists(),
        "fixture should start with populated submodule worktree"
    );

    enter_edit_mode(&mut ctx, foobar, stack.id)?;
    assert!(
        submodule_probe.exists(),
        "submodule file should remain after entering edit mode"
    );
    save_and_return_to_workspace(&mut ctx)?;

    assert!(
        submodule_probe.exists(),
        "submodule file should remain after leaving edit mode"
    );

    Ok(())
}

#[test]
fn abort_preserves_preexisting_dirty_and_diverged_submodule_state() -> Result<()> {
    let (mut ctx, _tempdir) =
        command_ctx("save_and_return_to_workspace_preserves_submodule_worktree")?;
    let (foobar, worktree_dir) = {
        let repo = ctx.git2_repo.get()?;
        let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();
        (foobar, repo.path().parent().unwrap().to_path_buf())
    };
    let submodule_dir = worktree_dir.join("submodules/test-module");

    run_git_at_dir(&submodule_dir, &["config", "user.name", "Submodule Author"])?;
    run_git_at_dir(
        &submodule_dir,
        &["config", "user.email", "submodule@example.com"],
    )?;
    std::fs::write(submodule_dir.join("diverged.txt"), "diverged commit\n")?;
    run_git_at_dir(&submodule_dir, &["add", "diverged.txt"])?;
    run_git_at_dir(&submodule_dir, &["commit", "-m", "local diverged commit"])?;

    std::fs::write(submodule_dir.join("dirty.txt"), "dirty worktree change\n")?;

    let baseline_submodule_head = run_git_at_dir(&submodule_dir, &["rev-parse", "HEAD"])?;
    let baseline_submodule_status = run_git_at_dir(&submodule_dir, &["status", "--porcelain"])?;
    let baseline_superproject_submodule_status = run_git_at_dir(
        &worktree_dir,
        &["status", "--porcelain", "submodules/test-module"],
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();

    enter_edit_mode(&mut ctx, foobar, stack.id)?;
    abort_and_return_to_workspace(&mut ctx, true)?;

    let final_submodule_head = run_git_at_dir(&submodule_dir, &["rev-parse", "HEAD"])?;
    let final_submodule_status = run_git_at_dir(&submodule_dir, &["status", "--porcelain"])?;
    let final_superproject_submodule_status = run_git_at_dir(
        &worktree_dir,
        &["status", "--porcelain", "submodules/test-module"],
    )?;

    assert_eq!(
        final_submodule_head, baseline_submodule_head,
        "abort should preserve pre-existing submodule commit divergence"
    );
    assert_eq!(
        final_submodule_status, baseline_submodule_status,
        "abort should preserve pre-existing dirty submodule working tree state"
    );
    assert_eq!(
        final_superproject_submodule_status, baseline_superproject_submodule_status,
        "abort should restore the same superproject-visible submodule state"
    );

    Ok(())
}

#[test]
fn abort_requires_force_warns_about_submodule_and_gitlink_reversion() -> Result<()> {
    let (mut ctx, _tempdir) =
        command_ctx("save_and_return_to_workspace_preserves_submodule_worktree")?;
    let (foobar, worktree_dir) = {
        let repo = ctx.git2_repo.get()?;
        let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();
        (foobar, repo.path().parent().unwrap().to_path_buf())
    };
    let submodule_dir = worktree_dir.join("submodules/test-module");

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();

    enter_edit_mode(&mut ctx, foobar, stack.id)?;

    run_git_at_dir(&submodule_dir, &["config", "user.name", "Submodule Author"])?;
    run_git_at_dir(
        &submodule_dir,
        &["config", "user.email", "submodule@example.com"],
    )?;
    std::fs::write(
        submodule_dir.join("during-edit-mode-gitlink-change.txt"),
        "changed during edit mode\n",
    )?;
    run_git_at_dir(
        &submodule_dir,
        &["add", "during-edit-mode-gitlink-change.txt"],
    )?;
    run_git_at_dir(
        &submodule_dir,
        &["commit", "-m", "gitlink change during edit mode"],
    )?;

    // Stage the submodule entry so superproject tree diff includes the gitlink change.
    run_git_at_dir(&worktree_dir, &["add", "submodules/test-module"])?;

    let result = abort_and_return_to_workspace(&mut ctx, false);
    assert!(result.is_err());
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("including submodule or gitlink changes"),
        "expected submodule/gitlink warning in abort error, got: {err}"
    );
    assert!(
        err.contains("will revert changes made during edit mode"),
        "expected explicit reversion warning in abort error, got: {err}"
    );

    Ok(())
}
