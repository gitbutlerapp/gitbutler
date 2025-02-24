use but_ctx::Context;
use gitbutler_repo::{
    hooks::{self, HookResult},
    staging,
};
use gitbutler_stack::BranchOwnershipClaims;

pub fn pre_commit(
    ctx: &Context,
    ownership: &BranchOwnershipClaims,
) -> Result<HookResult, anyhow::Error> {
    let repo = &*ctx.git2_repo.get()?;
    let diffs =
        gitbutler_diff::workdir(&*ctx.git2_repo.get()?, repo.head()?.peel_to_commit()?.id())?;
    let selected_files = staging::filter_diff_by_hunk_ids(
        diffs,
        ownership
            .claims
            .iter()
            .map(|claim| (&claim.file_path, &claim.hunks))
            .collect(),
    )?;
    hooks::pre_commit(ctx, &selected_files)
}

pub fn pre_commit_with_tree(
    ctx: &Context,
    tree_id: git2::Oid,
) -> Result<HookResult, anyhow::Error> {
    hooks::pre_commit_with_tree(ctx, tree_id)
}
