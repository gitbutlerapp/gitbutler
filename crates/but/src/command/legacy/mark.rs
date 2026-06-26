use std::str::FromStr;

use but_core::ref_metadata::StackId;
use but_ctx::Context;
use gitbutler_commit::commit_ext::CommitExt;

pub(crate) fn stack_marked(ctx: &Context, stack_id: StackId) -> anyhow::Result<bool> {
    let db = ctx.db.get_cache()?;
    let rules = but_rules::list_rules(&db)?
        .iter()
        .any(|r| r.target_stack_id() == Some(stack_id.to_string()) && r.session_id().is_none());
    Ok(rules)
}

pub(crate) fn commit_marked(ctx: &Context, commit_id: String) -> anyhow::Result<bool> {
    let change_id = {
        let repo = ctx.repo.get()?;
        let commit = repo.find_commit(gix::ObjectId::from_str(&commit_id)?)?;
        commit.change_id().ok_or_else(|| {
            anyhow::anyhow!("Commit {commit_id} does not have a Change-Id, cannot mark it")
        })?
    };
    let db = ctx.db.get_cache()?;
    let rules = but_rules::list_rules(&db)?
        .iter()
        .any(|r| r.target_change_id() == Some(change_id.clone()) && r.session_id().is_none());
    Ok(rules)
}
