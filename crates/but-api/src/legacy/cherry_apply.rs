use anyhow::Result;
use but_api_macros::but_api;
use but_cherry_apply::CherryApplyStatus;
use gitbutler_stack::StackId;
use tracing::instrument;

/// Return the cherry-pick applicability of `subject` in the current workspace state.
#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply_status(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
) -> Result<CherryApplyStatus> {
    let guard = ctx.exclusive_worktree_access();
    but_cherry_apply::cherry_apply_status(ctx, guard.read_permission(), subject)
}

/// Cherry-pick `subject` into the stack identified by `target`.
#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
    target: StackId,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();

    but_cherry_apply::cherry_apply(ctx, guard.write_permission(), subject, target)
}
