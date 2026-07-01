use but_core::ref_metadata::StackId;
use but_ctx::Context;

pub(crate) fn stack_id_by_commit_id(ctx: &Context, oid: gix::ObjectId) -> anyhow::Result<StackId> {
    for stack in crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)? {
        let Some(id) = stack.id else {
            continue;
        };
        if stack
            .branches
            .iter()
            .any(|branch| branch.commits.iter().any(|commit| commit.id == oid))
        {
            return Ok(id);
        }
    }
    anyhow::bail!("No stack found for commit {oid}")
}
