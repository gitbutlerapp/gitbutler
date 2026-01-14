use but_ctx::Context;
use gitbutler_repo::hooks::{self, HookResult};

pub fn pre_commit_with_tree(
    ctx: &Context,
    tree_id: git2::Oid,
) -> Result<HookResult, anyhow::Error> {
    hooks::pre_commit_with_tree(ctx, tree_id)
}
