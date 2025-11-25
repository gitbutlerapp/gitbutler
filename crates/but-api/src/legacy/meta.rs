use but_ctx::Context;

/// See docs of [`but_meta::VirtualBranchesTomlMetadata::write_reconciled()`]
pub fn reconcile_in_workspace_state_of_vb_toml(
    ctx: &mut Context,
    exclusive: &but_core::sync::WorktreeWritePermission,
) -> anyhow::Result<()> {
    let meta = ctx.legacy_meta_mut(exclusive)?;
    let repo = ctx.repo.get()?;
    meta.write_reconciled(&repo)
}
