use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_operating_modes::{
    assure_open_workspace_mode, INTEGRATION_BRANCH_REF, WORKSPACE_BRANCH_REF,
};
use gitbutler_stack::VirtualBranchesHandle;

/// Tests that "verify branch" won't complain if we are on the old integration
/// branch, and that `update_workspace_commit` will put us back on the a branch
/// with the new name.
#[test]
fn works_on_integration_branch() -> anyhow::Result<()> {
    let (ctx, _temp_dir) = gitbutler_testsupport::writable::fixture(
        "for-workspace-migration.sh",
        "workspace-migration",
    )?;

    // Check that we are on the old `gitbutler/integration` branch.
    assert_eq!(
        ctx.repository().head()?.name(),
        Some(INTEGRATION_BRANCH_REF)
    );

    // Should not throw verification error until migration is complete.
    let result = assure_open_workspace_mode(&ctx);
    assert!(result.is_ok());

    // Updating workspace commit should put us on the workspace branch.
    update_workspace_commit(&VirtualBranchesHandle::new(ctx.project().gb_dir()), &ctx)?;
    assert_eq!(ctx.repository().head()?.name(), Some(WORKSPACE_BRANCH_REF));
    Ok(())
}
