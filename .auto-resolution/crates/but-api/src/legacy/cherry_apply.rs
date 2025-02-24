use anyhow::Result;
use but_api_macros::but_api;
use but_cherry_apply::CherryApplyStatus;
use but_ctx::Context;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply_status(project_id: ProjectId, subject: String) -> Result<CherryApplyStatus> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let guard = ctx.exclusive_worktree_access();
    let subject_oid = gix::ObjectId::from_hex(subject.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid commit ID: {}", e))?;

    but_cherry_apply::cherry_apply_status(&ctx, guard.read_permission(), subject_oid)
}

#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply(project_id: ProjectId, subject: String, target: StackId) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let mut guard = ctx.exclusive_worktree_access();
    let subject_oid = gix::ObjectId::from_hex(subject.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid commit ID: {}", e))?;

    but_cherry_apply::cherry_apply(&ctx, guard.write_permission(), subject_oid, target)
}
