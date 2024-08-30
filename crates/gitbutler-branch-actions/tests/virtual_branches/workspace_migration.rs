use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_branch_actions::{update_workspace_commit, verify_branch};
use gitbutler_operating_modes::{INTEGRATION_BRANCH_REF, WORKSPACE_BRANCH_REF};

/// Tests that "verify branch" won't complain if we are on the old integration
/// branch, and that `update_workspace_commit` will put us back on the a branch
/// with the new name.
#[test]
fn works_on_integration_branch() -> anyhow::Result<()> {
    let ctx = gitbutler_testsupport::read_only::fixture(
        "for-workspace-migration.sh",
        "workspace-migration",
    )?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    // Check that we are on the old `gitbutler/integration` branch.
    assert_eq!(
        ctx.repository().head()?.name(),
        Some(INTEGRATION_BRANCH_REF)
    );

    // Should not throw verification error until migration is complete.
    let result = verify_branch(&ctx, perm);
    assert!(result.is_ok());

    // Updating workspace commit should put us on the workspace branch.
    update_workspace_commit(&VirtualBranchesHandle::new(ctx.project().gb_dir()), &ctx)?;
    assert_eq!(ctx.repository().head()?.name(), Some(WORKSPACE_BRANCH_REF));
    Ok(())
}
