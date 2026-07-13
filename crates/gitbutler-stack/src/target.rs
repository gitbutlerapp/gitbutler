use anyhow::Result;
use but_ctx::Context;

pub(crate) fn default_target_base_oid(ctx: &Context) -> Result<gix::ObjectId> {
    ctx.project_meta()?.target_commit_id_or_err()
}

pub(crate) fn default_target_push_remote_name(ctx: &Context) -> Result<String> {
    let repo = ctx.repo.get()?;
    ctx.project_meta()?.push_remote_name(&repo)
}
