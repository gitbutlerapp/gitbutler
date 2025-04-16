use anyhow::{bail, Context as _, Result};
use but_rebase::RebaseStep;
use but_workspace::stack_ext::StackExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_diff::Hunk;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{OwnershipClaim, Stack, StackId};
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
    ctx: &CommandContext,
    stack_id: StackId,
    commit_to_remove: git2::Oid,
    _perm: &mut WorktreeWritePermission,
) -> Result<Stack> {
    let vb_state = ctx.project().virtual_branches();

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;

    let merge_base = stack.merge_base(ctx)?;
    let repo = ctx.gix_repo()?;
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

    for ownership in ownership_update(ctx.repo(), commit_to_remove)? {
        stack.ownership.put(ownership);
    }

    let new_head = output.top_commit.to_git2();
    stack.set_stack_head(&vb_state, &repo, new_head, None)?;

    stack.set_heads_from_rebase_output(ctx, output.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(stack)
}

fn ownership_update(
    repo: &git2::Repository,
    commit_to_remove: git2::Oid,
) -> Result<Vec<OwnershipClaim>> {
    let commit_to_remove = repo.find_commit(commit_to_remove)?;

    if commit_to_remove.is_conflicted() {
        bail!("Can not undo a conflicted commit");
    }
    let commit_tree = commit_to_remove
        .tree()
        .context("failed to get commit tree")?;
    let commit_to_remove_parent = commit_to_remove.parent(0)?;
    let commit_parent_tree = commit_to_remove_parent
        .tree()
        .context("failed to get parent tree")?;

    let diff = gitbutler_diff::trees(repo, &commit_parent_tree, &commit_tree, true)?;
    let ownership_update = diff
        .iter()
        .filter_map(|(file_path, file_diff)| {
            let file_path = file_path.clone();
            let hunks = file_diff
                .hunks
                .iter()
                .map(Into::into)
                .filter(|hunk: &Hunk| !hunk.is_null())
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                return None;
            }
            Some(OwnershipClaim { file_path, hunks })
        })
        .collect::<Vec<_>>();
    Ok(ownership_update)
}
