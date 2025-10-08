use anyhow::Result;
use git2::build::CheckoutBuilder;
use gitbutler_command_context::CommandContext;
use gitbutler_edit_mode::commands::{enter_edit_mode, save_and_return_to_workspace};
use gitbutler_stack::VirtualBranchesHandle;
use tempfile::TempDir;

fn command_ctx(folder: &str) -> Result<(CommandContext, TempDir)> {
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
    let (ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repository = ctx.repo();

    let foobar = repository.head()?.peel_to_commit()?.parent(0)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack = stacks.first().unwrap();
    enter_edit_mode(&ctx, foobar.id(), stack.id)?;

    let init = repository
        .find_reference("refs/heads/main")?
        .peel_to_commit()?;
    let left = repository
        .find_reference("refs/heads/left")?
        .peel_to_commit()?;
    let right = repository
        .find_reference("refs/heads/right")?
        .peel_to_commit()?;

    let mut merge = repository.merge_trees(
        &init.tree()?,
        &left.tree()?,
        &right.tree()?,
        Default::default(),
    )?;

    repository.checkout_index(
        Some(&mut merge),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    save_and_return_to_workspace(&ctx)?;

    assert_eq!(
        std::fs::read_to_string(repository.path().parent().unwrap().join("conflict"))?,
        "<<<<<<< ours\nleft\n|||||||\n=======\nright\n>>>>>>> theirs\n".to_string()
    );

    Ok(())
}
