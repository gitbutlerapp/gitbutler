use anyhow::{Context as _, Result};
use but_ctx::{Context, access::WorktreeWritePermission};
use but_oxidize::{ObjectIdExt, OidExt};
use but_rebase::RebaseStep;
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_stack::{Stack, StackId};
use tracing::instrument;

use crate::VirtualBranchesExt as _;

/// Removes a commit from a branch by rebasing all commits _except_ for it
/// onto it's parent.
///
/// If successful, it will update the branch head to the new head commit.
///
/// It intentionally does **not** update the branch tree. It is a feature
/// of the operation that the branch tree will not be updated as it allows
/// the user to then re-commit the changes if they wish.
///
/// This may create conflicted commits above the commit that is getting
/// undone.
#[instrument(level = tracing::Level::DEBUG, skip(ctx, _perm))]
pub(crate) fn undo_commit(
    ctx: &Context,
    stack_id: StackId,
    commit_to_remove: git2::Oid,
    _perm: &mut WorktreeWritePermission,
) -> Result<Stack> {
    let vb_state = ctx.legacy_project.virtual_branches();

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;

    let merge_base = stack.merge_base(ctx)?;
    let repo = ctx.repo.get()?;
    let steps = stack
        .as_rebase_steps(ctx, &repo)?
        .into_iter()
        .filter(|s| match s {
            RebaseStep::Pick {
                commit_id,
                new_message: _,
            } => commit_id != &commit_to_remove.to_gix(),
            _ => true,
        })
        .collect::<Vec<_>>();

    let mut rebase = but_rebase::Rebase::new(&repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;

    let new_head = output.top_commit.to_git2();
    stack.set_stack_head(&vb_state, &repo, new_head, None)?;

    stack.set_heads_from_rebase_output(ctx, output.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(stack)
}
