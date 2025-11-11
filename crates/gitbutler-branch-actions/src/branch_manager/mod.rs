pub use branch_creation::CreateBranchFromBranchOutcome;
use but_oxidize::OidExt;
use gitbutler_command_context::CommandContext;

mod branch_creation;
mod branch_removal;

/// Note that this checks out the commit and sets the HEAD ref to point to it.
pub(crate) fn checkout_remerged_head(
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<()> {
    let (workspace_tree_id, _, _) = but_workspace::legacy::remerged_workspace_tree_v2(ctx, repo)?;
    but_core::worktree::safe_checkout_from_head(
        workspace_tree_id.to_gix(),
        repo,
        but_core::worktree::checkout::Options::default(),
    )?;
    Ok(())
}

pub struct BranchManager<'l> {
    ctx: &'l CommandContext,
}

pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager<'_>;
}

impl BranchManagerExt for CommandContext {
    fn branch_manager(&self) -> BranchManager<'_> {
        BranchManager { ctx: self }
    }
}
