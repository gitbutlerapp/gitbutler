use anyhow::Result;
use but_api_macros::but_api;
use but_cherry_apply::CherryApplyStatus;
use gitbutler_stack::StackId;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply_status(ctx: &mut but_ctx::Context, subject: String) -> Result<CherryApplyStatus> {
    let guard = ctx.exclusive_worktree_access();
    let subject_oid =
        gix::ObjectId::from_hex(subject.as_bytes()).map_err(|e| anyhow::anyhow!("Invalid commit ID: {e}"))?;

    but_cherry_apply::cherry_apply_status(ctx, guard.read_permission(), subject_oid)
}

#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply(ctx: &mut but_ctx::Context, subject: String, target: StackId) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let subject_oid =
        gix::ObjectId::from_hex(subject.as_bytes()).map_err(|e| anyhow::anyhow!("Invalid commit ID: {e}"))?;

    but_cherry_apply::cherry_apply(ctx, guard.write_permission(), subject_oid, target)
}
